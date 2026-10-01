//! Monitoring engine that manages the background monitoring loop.
//!
//! This module extracts all monitoring orchestration logic out of the TUI,
//! implementing a soft/hard state model inspired by Nagios, Zabbix, and
//! Uptime Kuma to reduce false positives:
//!
//! - **Online**: Node is responding. A single check failure transitions to Degraded.
//! - **Degraded** (soft state): Node failed a check but hasn't yet been confirmed down.
//!   Retries happen at a shorter `retry_interval`.
//! - **Offline** (hard state): Node has failed `max_check_attempts` consecutive checks.
//!
//! Recovery from either Degraded or Offline is immediate on the first successful check.
//! Every transition between the three states is persisted as a `StatusChange`, stamped
//! with the time of the check that caused it and with the time of the last check that
//! still succeeded, along with the monitoring result itself. Results for checks that
//! leave the status unchanged are not stored. A node's first check ever sets its status
//! without recording a transition: the status it was created with is a placeholder,
//! not something it was observed in.
//!
//! The node's runtime state, the transition and the result are written in one
//! transaction. If that write fails the engine keeps the previous state in memory
//! and records the transition on the next check instead, so a transient database
//! error delays a transition rather than losing it.
//!
//! Each run of the engine is recorded in `engine_runs` and kept alive with a
//! heartbeat, so the history can show when nothing was being monitored at all.
//!
//! One background thread owns the loop and all per-node state. Checks themselves do
//! not run on that thread: each due node is handed to a tokio task, so a slow or
//! timing-out check never delays the nodes behind it, and the loop keeps applying
//! config updates and finished results while checks are in flight. A node is never
//! checked twice at once.

use crate::database::Database;
use crate::models::{MonitoringResult, Node, NodeStatus, StatusChange};
use crate::monitor::check_node;
use chrono::{DateTime, Utc};
use std::collections::{HashMap, HashSet};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use tracing::{error, info};

/// How often the loop wakes up to launch due checks and apply finished ones.
const TICK: Duration = Duration::from_millis(250);

/// How often the engine records that it is still running. A crash loses at
/// most this much of the monitored timeline to the following gap.
const HEARTBEAT: Duration = Duration::from_secs(5);

/// Commands sent to the monitoring thread to update its node configuration.
#[derive(Clone)]
pub enum NodeConfigUpdate {
    Add(Node),
    Update(Node),
    Delete(i64),
}

/// Handle returned when monitoring starts, used to control the background thread.
pub struct MonitoringHandle {
    pub stop_tx: mpsc::Sender<()>,
    pub config_tx: mpsc::Sender<NodeConfigUpdate>,
}

/// What the engine remembers about a node between checks, beyond what is on
/// the `Node` itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeState {
    /// The status the node was last known to be in.
    pub status: NodeStatus,
    /// When that status began, if a transition into it was ever recorded.
    pub last_change_at: Option<DateTime<Utc>>,
    /// When the node's most recent successful check ran.
    pub last_success_at: Option<DateTime<Utc>>,
}

/// Starts the monitoring engine in a background thread.
///
/// Returns a `MonitoringHandle` for sending stop/config signals, and uses the
/// provided `update_tx` channel to send updated nodes back to the caller (TUI).
pub fn start_monitoring(
    db: Database,
    initial_nodes: Vec<Node>,
    update_tx: mpsc::Sender<Node>,
) -> MonitoringHandle {
    info!("Starting monitoring engine");
    let (stop_tx, stop_rx) = mpsc::channel();
    let (config_tx, config_rx) = mpsc::channel();

    thread::spawn(move || {
        run_monitoring_loop(db, initial_nodes, update_tx, stop_rx, config_rx);
    });

    MonitoringHandle { stop_tx, config_tx }
}

/// The main monitoring loop that runs in a background thread.
///
/// Every tick it drains config updates, applies the results of checks that
/// have finished, and launches a check for every node that is due. Checks run
/// concurrently as tokio tasks and report back over a channel; only this thread
/// touches the node list, the per-node state, and the database.
fn run_monitoring_loop(
    db: Database,
    initial_nodes: Vec<Node>,
    update_tx: mpsc::Sender<Node>,
    stop_rx: mpsc::Receiver<()>,
    config_rx: mpsc::Receiver<NodeConfigUpdate>,
) {
    let mut last_check_times: HashMap<i64, Instant> = HashMap::new();

    // Seed per-node state from the database so a restart neither records
    // duplicate transitions nor loses track of how long the current state
    // has lasted.
    let mut node_states: HashMap<i64, NodeState> = HashMap::new();
    for node in &initial_nodes {
        if let Some(node_id) = node.id {
            node_states.insert(node_id, load_node_state(&db, node));
        }
    }

    let run_id = match db.start_engine_run(Utc::now()) {
        Ok(id) => Some(id),
        Err(e) => {
            error!("Failed to record engine start: {}", e);
            None
        }
    };
    let mut last_heartbeat = Instant::now();

    let mut current_nodes = initial_nodes;
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let (result_tx, result_rx) = mpsc::channel::<(i64, anyhow::Result<MonitoringResult>)>();
    let mut in_flight: HashSet<i64> = HashSet::new();

    loop {
        process_config_updates(
            &config_rx,
            &db,
            &mut current_nodes,
            &mut last_check_times,
            &mut node_states,
        );

        // Apply every check that finished since the last tick.
        while let Ok((node_id, result)) = result_rx.try_recv() {
            in_flight.remove(&node_id);
            let Ok(check_result) = result else { continue };
            // The node may have been deleted while its check was in flight.
            let Some(node) = current_nodes.iter_mut().find(|n| n.id == Some(node_id)) else {
                continue;
            };
            apply_check_result(&db, node, check_result, &mut node_states);
            if update_tx.send(node.clone()).is_err() {
                heartbeat(&db, run_id);
                runtime.shutdown_background();
                return;
            }
        }

        // Launch a check for every node that is due and not already being checked.
        for node in &current_nodes {
            let Some(node_id) = node.id else { continue };
            if in_flight.contains(&node_id) || !should_check_node(node, node_id, &last_check_times)
            {
                continue;
            }

            last_check_times.insert(node_id, Instant::now());
            in_flight.insert(node_id);
            let node = node.clone();
            let result_tx = result_tx.clone();
            runtime.spawn(async move {
                let result = check_node(&node).await;
                let _ = result_tx.send((node_id, result));
            });
        }

        if last_heartbeat.elapsed() >= HEARTBEAT {
            heartbeat(&db, run_id);
            last_heartbeat = Instant::now();
        }

        match stop_rx.recv_timeout(TICK) {
            Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }

    heartbeat(&db, run_id);
    // Don't wait for in-flight checks (a ping can take count × timeout seconds).
    runtime.shutdown_background();
}

fn heartbeat(db: &Database, run_id: Option<i64>) {
    if let Some(run_id) = run_id {
        if let Err(e) = db.touch_engine_run(run_id, Utc::now()) {
            error!("Failed to record engine heartbeat: {}", e);
        }
    }
}

/// Folds a finished check into the node: runs the soft/hard state machine,
/// records the transition and its result when the status changed, and
/// persists the node's runtime state, all in one database transaction.
///
/// The transition is stamped with the check's own timestamp, not the time it
/// was applied, and carries the time of the last successful check so the
/// history can bound when the node really went down.
pub fn apply_check_result(
    db: &Database,
    node: &mut Node,
    mut check_result: MonitoringResult,
    node_states: &mut HashMap<i64, NodeState>,
) {
    let node_id = node.id.unwrap_or(0);
    let previous = node_states.get(&node_id).copied();
    let check_succeeded = check_result.status == NodeStatus::Online;
    let checked_at = check_result.timestamp;

    // A node that has never been checked holds a placeholder status, not one
    // it was observed in: its first check sets the status without a transition.
    let first_check = node.last_check.is_none();

    let new_status = evaluate_node_status(node, check_succeeded);
    check_result.status = new_status;
    check_result.node_id = node_id;

    let last_success_at = if check_succeeded {
        Some(checked_at)
    } else {
        previous.and_then(|state| state.last_success_at)
    };

    let transition = match previous {
        Some(state) if !first_check && state.status != new_status => Some(StatusChange {
            id: None,
            node_id,
            from_status: state.status,
            to_status: new_status,
            changed_at: checked_at,
            duration_ms: state
                .last_change_at
                .map(|began| StatusChange::calculate_duration(began, checked_at)),
            last_success_at,
        }),
        _ => None,
    };

    node.status = new_status;
    node.last_check = Some(checked_at);
    node.response_time = check_result.response_time;

    // Keep the result that caused a transition, and the first check ever.
    let keep_result = transition.is_some() || first_check;

    // Persist only runtime state: the user may have edited this node's
    // configuration while the check was in flight, and writing our copy
    // back would clobber that edit in the database.
    match db.record_check(
        node,
        transition.as_ref(),
        keep_result.then_some(&check_result),
    ) {
        Ok(()) => {
            node_states.insert(
                node_id,
                NodeState {
                    status: new_status,
                    last_change_at: transition
                        .as_ref()
                        .map(|change| change.changed_at)
                        .or(previous.and_then(|state| state.last_change_at)),
                    last_success_at,
                },
            );
        }
        Err(e) => {
            error!(
                "Failed to record check for node {} ({}); will retry on the next check: {}",
                node_id, node.name, e
            );
            // Remember the success so the eventual transition still carries
            // it, but keep the old status so the transition is attempted again.
            match node_states.get_mut(&node_id) {
                Some(state) => state.last_success_at = last_success_at,
                None => {
                    node_states.insert(
                        node_id,
                        NodeState {
                            status: new_status,
                            last_change_at: None,
                            last_success_at,
                        },
                    );
                }
            }
        }
    }
}

/// Loads the persisted state the engine needs for a node.
///
/// The status and the time it began come from the latest recorded status
/// change; a node without one keeps the status on its record. The last
/// successful check is the node's last check when it is Online (an Online
/// node's last check succeeded by definition), otherwise whatever the latest
/// change recorded.
pub fn load_node_state(db: &Database, node: &Node) -> NodeState {
    let latest_change = node
        .id
        .and_then(|node_id| db.get_latest_status_change(node_id).ok().flatten());

    let status = latest_change
        .as_ref()
        .map(|change| change.to_status)
        .unwrap_or(node.status);

    let last_success_at = if status == NodeStatus::Online {
        node.last_check
    } else {
        latest_change
            .as_ref()
            .and_then(|change| change.last_success_at)
    };

    NodeState {
        status,
        last_change_at: latest_change.map(|change| change.changed_at),
        last_success_at,
    }
}

/// Determines if a node should be checked based on its interval and current state.
///
/// When degraded (soft failure), uses the shorter `retry_interval` for faster confirmation.
/// Otherwise uses the normal `monitoring_interval`.
fn should_check_node(node: &Node, node_id: i64, last_check_times: &HashMap<i64, Instant>) -> bool {
    let now = Instant::now();
    let interval = if node.status == NodeStatus::Degraded {
        node.retry_interval
    } else {
        node.monitoring_interval
    };

    last_check_times
        .get(&node_id)
        .is_none_or(|last_check| now.duration_since(*last_check).as_secs() >= interval)
}

/// Evaluates the new status of a node based on check result and soft/hard state logic.
///
/// State machine:
/// - Online + success → Online (reset failures)
/// - Online + failure → Degraded (start counting)
/// - Degraded + success → Online (recovery, reset failures)
/// - Degraded + failure (< max_attempts) → Degraded (keep counting)
/// - Degraded + failure (>= max_attempts) → Offline (confirmed down)
/// - Offline + success → Online (immediate recovery)
/// - Offline + failure → Offline (stay down, reset counter to max)
pub fn evaluate_node_status(node: &mut Node, check_succeeded: bool) -> NodeStatus {
    if check_succeeded {
        // Any success immediately recovers the node
        node.consecutive_failures = 0;
        NodeStatus::Online
    } else {
        // Failure path
        node.consecutive_failures += 1;

        if node.consecutive_failures >= node.max_check_attempts {
            // Enough failures to confirm offline (hard state)
            NodeStatus::Offline
        } else {
            // Not enough failures yet — soft state
            NodeStatus::Degraded
        }
    }
}

/// Process incoming configuration updates from the TUI.
fn process_config_updates(
    config_rx: &mpsc::Receiver<NodeConfigUpdate>,
    db: &Database,
    current_nodes: &mut Vec<Node>,
    last_check_times: &mut HashMap<i64, Instant>,
    node_states: &mut HashMap<i64, NodeState>,
) {
    while let Ok(config_update) = config_rx.try_recv() {
        match config_update {
            NodeConfigUpdate::Add(node) => {
                if !current_nodes.iter().any(|n| n.id == node.id) {
                    if let Some(node_id) = node.id {
                        node_states.insert(node_id, load_node_state(db, &node));
                    }
                    current_nodes.push(node);
                }
            }
            NodeConfigUpdate::Update(updated_node) => {
                if let Some(node) = current_nodes.iter_mut().find(|n| n.id == updated_node.id) {
                    let status = node.status;
                    let last_check = node.last_check;
                    let response_time = node.response_time;
                    let consecutive_failures = node.consecutive_failures;

                    *node = updated_node;
                    // Preserve runtime state
                    node.status = status;
                    node.last_check = last_check;
                    node.response_time = response_time;
                    node.consecutive_failures = consecutive_failures;

                    if let Some(node_id) = node.id {
                        last_check_times.remove(&node_id);
                    }
                }
            }
            NodeConfigUpdate::Delete(node_id) => {
                current_nodes.retain(|n| n.id != Some(node_id));
                last_check_times.remove(&node_id);
                node_states.remove(&node_id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{MonitorDetail, MonitoringResult, DEFAULT_MAX_CHECK_ATTEMPTS};

    fn temp_db() -> (tempfile::TempDir, Database) {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::new(dir.path().join("engine.db")).unwrap();
        (dir, db)
    }

    fn checked_node(status: NodeStatus, failures: u32, max_attempts: u32) -> Node {
        let mut node = make_node(status, failures, max_attempts);
        node.last_check = Some(Utc::now() - chrono::Duration::seconds(60));
        node
    }

    #[test]
    fn test_load_node_state_without_history_uses_node_record() {
        let (_dir, db) = temp_db();
        let mut node = checked_node(NodeStatus::Online, 0, 3);
        node.id = Some(db.add_node(&node).unwrap());

        let state = load_node_state(&db, &node);
        assert_eq!(state.status, NodeStatus::Online);
        assert_eq!(state.last_change_at, None);
        assert_eq!(
            state.last_success_at, node.last_check,
            "an Online node's last check was a success"
        );
    }

    #[test]
    fn test_load_node_state_restores_status_change_time_and_last_success() {
        let (_dir, db) = temp_db();
        let mut node = checked_node(NodeStatus::Online, 0, 3);
        let node_id = db.add_node(&node).unwrap();
        node.id = Some(node_id);

        let changed_at = Utc::now() - chrono::Duration::minutes(42);
        let last_success_at = changed_at - chrono::Duration::minutes(1);
        db.add_status_change(&StatusChange {
            id: None,
            node_id,
            from_status: NodeStatus::Online,
            to_status: NodeStatus::Offline,
            changed_at,
            duration_ms: Some(1000),
            last_success_at: Some(last_success_at),
        })
        .unwrap();

        let state = load_node_state(&db, &node);
        assert_eq!(
            state.status,
            NodeStatus::Offline,
            "the history wins over the node record"
        );
        // Compare at millisecond precision: RFC 3339 storage may drop sub-ms digits
        assert_eq!(
            state.last_change_at.map(|t| t.timestamp_millis()),
            Some(changed_at.timestamp_millis())
        );
        assert_eq!(
            state.last_success_at.map(|t| t.timestamp_millis()),
            Some(last_success_at.timestamp_millis())
        );
    }

    fn make_result(status: NodeStatus) -> MonitoringResult {
        MonitoringResult {
            id: None,
            node_id: 0,
            timestamp: Utc::now(),
            status,
            response_time: (status == NodeStatus::Online).then_some(12),
            details: Some("test".to_string()),
        }
    }

    fn state(
        status: NodeStatus,
        last_change_at: Option<DateTime<Utc>>,
        last_success_at: Option<DateTime<Utc>>,
    ) -> NodeState {
        NodeState {
            status,
            last_change_at,
            last_success_at,
        }
    }

    // -- apply_check_result tests --

    #[test]
    fn test_apply_first_check_records_result_but_no_transition() {
        let (_dir, db) = temp_db();
        // A new node is created Offline as a placeholder and has never been checked.
        let mut node = make_node(NodeStatus::Offline, 0, 3);
        let node_id = db.add_node(&node).unwrap();
        node.id = Some(node_id);
        let mut states = HashMap::from([(node_id, load_node_state(&db, &node))]);

        apply_check_result(&db, &mut node, make_result(NodeStatus::Online), &mut states);

        assert_eq!(node.status, NodeStatus::Online);
        assert_eq!(node.response_time, Some(12));
        assert!(node.last_check.is_some());
        assert_eq!(states[&node_id].status, NodeStatus::Online);
        assert_eq!(states[&node_id].last_success_at, node.last_check);
        let stored = db.get_latest_monitoring_result(node_id).unwrap().unwrap();
        assert_eq!(stored.node_id, node_id);
        assert!(
            db.get_latest_status_change(node_id).unwrap().is_none(),
            "the placeholder status is not a state the node was observed in"
        );
        assert_eq!(
            db.get_all_nodes().unwrap()[0].status,
            NodeStatus::Online,
            "runtime state is persisted"
        );
    }

    #[test]
    fn test_apply_transition_is_stamped_with_the_check_time_and_last_success() {
        let (_dir, db) = temp_db();
        let mut node = checked_node(NodeStatus::Online, 0, 3);
        let node_id = db.add_node(&node).unwrap();
        node.id = Some(node_id);
        let began = Utc::now() - chrono::Duration::seconds(30);
        let last_success = node.last_check;
        let mut states = HashMap::from([(
            node_id,
            state(NodeStatus::Online, Some(began), last_success),
        )]);

        let mut result = make_result(NodeStatus::Offline);
        // The check finished a while before the engine got to apply it.
        result.timestamp = Utc::now() - chrono::Duration::seconds(10);
        let checked_at = result.timestamp;

        apply_check_result(&db, &mut node, result, &mut states);

        assert_eq!(node.status, NodeStatus::Degraded);
        assert_eq!(node.consecutive_failures, 1);
        assert_eq!(node.response_time, None);
        let change = db.get_latest_status_change(node_id).unwrap().unwrap();
        assert_eq!(change.from_status, NodeStatus::Online);
        assert_eq!(change.to_status, NodeStatus::Degraded);
        assert_eq!(
            change.changed_at.timestamp_millis(),
            checked_at.timestamp_millis(),
            "the transition happened when the check did, not when it was applied"
        );
        assert_eq!(
            change.duration_ms,
            Some((checked_at - began).num_milliseconds())
        );
        assert_eq!(
            change.last_success_at.map(|t| t.timestamp_millis()),
            last_success.map(|t| t.timestamp_millis()),
            "the transition out of Online carries the last good check"
        );
        assert_eq!(
            db.get_latest_monitoring_result(node_id)
                .unwrap()
                .unwrap()
                .status,
            NodeStatus::Degraded,
            "the stored result carries the evaluated status, not the raw check"
        );
        assert_eq!(states[&node_id].status, NodeStatus::Degraded);
        assert_eq!(states[&node_id].last_change_at, Some(checked_at));
        assert_eq!(states[&node_id].last_success_at, last_success);
    }

    #[test]
    fn test_apply_recovery_carries_the_recovering_check_as_last_success() {
        let (_dir, db) = temp_db();
        let mut node = checked_node(NodeStatus::Offline, 3, 3);
        let node_id = db.add_node(&node).unwrap();
        node.id = Some(node_id);
        let old_success = Some(Utc::now() - chrono::Duration::hours(1));
        let mut states = HashMap::from([(node_id, state(NodeStatus::Offline, None, old_success))]);

        let result = make_result(NodeStatus::Online);
        let checked_at = result.timestamp;
        apply_check_result(&db, &mut node, result, &mut states);

        let change = db.get_latest_status_change(node_id).unwrap().unwrap();
        assert_eq!(change.to_status, NodeStatus::Online);
        assert_eq!(
            change.last_success_at.map(|t| t.timestamp_millis()),
            Some(checked_at.timestamp_millis())
        );
        assert_eq!(states[&node_id].last_success_at, Some(checked_at));
    }

    #[test]
    fn test_apply_unchanged_status_stores_nothing() {
        let (_dir, db) = temp_db();
        let mut node = checked_node(NodeStatus::Online, 0, 3);
        let node_id = db.add_node(&node).unwrap();
        node.id = Some(node_id);
        let mut states = HashMap::from([(node_id, state(NodeStatus::Online, None, None))]);

        apply_check_result(&db, &mut node, make_result(NodeStatus::Online), &mut states);

        assert!(db.get_latest_monitoring_result(node_id).unwrap().is_none());
        assert!(db.get_latest_status_change(node_id).unwrap().is_none());
        assert!(node.last_check.is_some(), "runtime state still updates");
        assert_eq!(
            states[&node_id].last_success_at, node.last_check,
            "every successful check moves the last success forward"
        );
    }

    #[test]
    fn test_apply_keeps_old_state_when_the_write_fails() {
        let (dir, db) = temp_db();
        let mut node = checked_node(NodeStatus::Online, 0, 3);
        let node_id = db.add_node(&node).unwrap();
        node.id = Some(node_id);
        let began = Utc::now() - chrono::Duration::seconds(30);
        let mut states = HashMap::from([(
            node_id,
            state(NodeStatus::Online, Some(began), node.last_check),
        )]);

        // Make the database unwritable: the handle only holds the path, so a
        // directory in place of the file makes every connection fail.
        std::fs::remove_file(dir.path().join("engine.db")).unwrap();
        std::fs::create_dir(dir.path().join("engine.db")).unwrap();

        apply_check_result(
            &db,
            &mut node,
            make_result(NodeStatus::Offline),
            &mut states,
        );

        assert_eq!(
            node.status,
            NodeStatus::Degraded,
            "the screen still shows the new status"
        );
        assert_eq!(
            states[&node_id].status,
            NodeStatus::Online,
            "the transition is still owed and will be recorded by the next check"
        );
        assert_eq!(states[&node_id].last_change_at, Some(began));
    }

    // -- end-to-end: checks run concurrently --

    /// A local HTTP server that holds every request for `delay` before answering 200.
    fn slow_http_server(delay: Duration) -> u16 {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                thread::spawn(move || {
                    let mut stream = stream;
                    let mut buf = [0u8; 1024];
                    let _ = stream.read(&mut buf);
                    thread::sleep(delay);
                    let _ = stream.write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    );
                });
            }
        });
        port
    }

    #[test]
    fn test_engine_checks_nodes_concurrently() {
        let (_dir, db) = temp_db();
        let delay = Duration::from_millis(1500);
        let port = slow_http_server(delay);

        let node_count = 4;
        let mut nodes = Vec::new();
        for i in 0..node_count {
            let mut node = make_node(NodeStatus::Offline, 0, 3);
            node.name = format!("slow {}", i);
            node.detail = MonitorDetail::Http {
                url: format!("http://127.0.0.1:{}/", port),
                expected_status: 200,
            };
            node.id = Some(db.add_node(&node).unwrap());
            nodes.push(node);
        }

        let (update_tx, update_rx) = mpsc::channel();
        let started = Instant::now();
        let handle = start_monitoring(db, nodes, update_tx);

        // Sequential checks would need node_count × delay; concurrent ones
        // finish in about one delay plus a tick. Leave generous slack for CI.
        let deadline = delay * 2 + Duration::from_secs(1);
        let mut updated = Vec::new();
        while updated.len() < node_count {
            let remaining = deadline.saturating_sub(started.elapsed());
            let node = update_rx.recv_timeout(remaining).unwrap_or_else(|_| {
                panic!(
                    "only {} of {} nodes were checked within {:?}: the checks did not overlap",
                    updated.len(),
                    node_count,
                    deadline
                )
            });
            assert_eq!(node.status, NodeStatus::Online, "{}", node.name);
            updated.push(node);
        }

        let _ = handle.stop_tx.send(());
    }

    #[test]
    fn test_engine_stops_on_request() {
        let (_dir, db) = temp_db();
        let (update_tx, update_rx) = mpsc::channel();
        let handle = start_monitoring(db, Vec::new(), update_tx);
        handle.stop_tx.send(()).unwrap();
        // The engine drops its update sender when it exits.
        assert_eq!(
            update_rx.recv_timeout(Duration::from_secs(5)),
            Err(mpsc::RecvTimeoutError::Disconnected)
        );
    }

    fn make_node(status: NodeStatus, failures: u32, max_attempts: u32) -> Node {
        Node {
            id: Some(1),
            name: "Test".to_string(),
            detail: MonitorDetail::Http {
                url: "https://example.com".to_string(),
                expected_status: 200,
            },
            status,
            last_check: None,
            response_time: None,
            monitoring_interval: 60,
            consecutive_failures: failures,
            max_check_attempts: max_attempts,
            retry_interval: 15,
        }
    }

    // -- evaluate_node_status tests --

    #[test]
    fn test_online_success_stays_online() {
        let mut node = make_node(NodeStatus::Online, 0, 3);
        let status = evaluate_node_status(&mut node, true);
        assert_eq!(status, NodeStatus::Online);
        assert_eq!(node.consecutive_failures, 0);
    }

    #[test]
    fn test_online_failure_becomes_degraded() {
        let mut node = make_node(NodeStatus::Online, 0, 3);
        let status = evaluate_node_status(&mut node, false);
        assert_eq!(status, NodeStatus::Degraded);
        assert_eq!(node.consecutive_failures, 1);
    }

    #[test]
    fn test_degraded_success_recovers_to_online() {
        let mut node = make_node(NodeStatus::Degraded, 1, 3);
        let status = evaluate_node_status(&mut node, true);
        assert_eq!(status, NodeStatus::Online);
        assert_eq!(node.consecutive_failures, 0);
    }

    #[test]
    fn test_degraded_failure_stays_degraded() {
        let mut node = make_node(NodeStatus::Degraded, 1, 3);
        let status = evaluate_node_status(&mut node, false);
        assert_eq!(status, NodeStatus::Degraded);
        assert_eq!(node.consecutive_failures, 2);
    }

    #[test]
    fn test_degraded_reaches_max_becomes_offline() {
        let mut node = make_node(NodeStatus::Degraded, 2, 3);
        let status = evaluate_node_status(&mut node, false);
        assert_eq!(status, NodeStatus::Offline);
        assert_eq!(node.consecutive_failures, 3);
    }

    #[test]
    fn test_offline_success_recovers_to_online() {
        let mut node = make_node(NodeStatus::Offline, 3, 3);
        let status = evaluate_node_status(&mut node, true);
        assert_eq!(status, NodeStatus::Online);
        assert_eq!(node.consecutive_failures, 0);
    }

    #[test]
    fn test_offline_failure_stays_offline() {
        let mut node = make_node(NodeStatus::Offline, 3, 3);
        let status = evaluate_node_status(&mut node, false);
        assert_eq!(status, NodeStatus::Offline);
        assert_eq!(node.consecutive_failures, 4);
    }

    #[test]
    fn test_max_attempts_of_one_skips_degraded() {
        let mut node = make_node(NodeStatus::Online, 0, 1);
        let status = evaluate_node_status(&mut node, false);
        assert_eq!(status, NodeStatus::Offline);
        assert_eq!(node.consecutive_failures, 1);
    }

    #[test]
    fn test_default_max_check_attempts() {
        assert_eq!(DEFAULT_MAX_CHECK_ATTEMPTS, 3);
    }

    // -- should_check_node tests --

    #[test]
    fn test_should_check_node_first_time() {
        let node = make_node(NodeStatus::Online, 0, 3);
        let last_check_times = HashMap::new();
        assert!(should_check_node(&node, 1, &last_check_times));
    }

    #[test]
    fn test_should_check_degraded_uses_retry_interval() {
        let mut node = make_node(NodeStatus::Degraded, 1, 3);
        node.retry_interval = 10;
        node.monitoring_interval = 60;

        let mut last_check_times = HashMap::new();
        // Checked 11 seconds ago - should check (retry_interval=10)
        last_check_times.insert(1, Instant::now() - Duration::from_secs(11));
        assert!(should_check_node(&node, 1, &last_check_times));

        // Checked 5 seconds ago - should not check (retry_interval=10)
        last_check_times.insert(1, Instant::now() - Duration::from_secs(5));
        assert!(!should_check_node(&node, 1, &last_check_times));
    }

    #[test]
    fn test_should_check_online_uses_monitoring_interval() {
        let mut node = make_node(NodeStatus::Online, 0, 3);
        node.retry_interval = 10;
        node.monitoring_interval = 60;

        let mut last_check_times = HashMap::new();
        // Checked 11 seconds ago - should NOT check (monitoring_interval=60)
        last_check_times.insert(1, Instant::now() - Duration::from_secs(11));
        assert!(!should_check_node(&node, 1, &last_check_times));

        // Checked 61 seconds ago - should check
        last_check_times.insert(1, Instant::now() - Duration::from_secs(61));
        assert!(should_check_node(&node, 1, &last_check_times));
    }

    // -- Full state machine walkthrough --

    #[test]
    fn test_full_state_machine_cycle() {
        let mut node = make_node(NodeStatus::Online, 0, 3);

        // Online → first failure → Degraded
        let s = evaluate_node_status(&mut node, false);
        assert_eq!(s, NodeStatus::Degraded);
        assert_eq!(node.consecutive_failures, 1);

        // Degraded → second failure → still Degraded
        let s = evaluate_node_status(&mut node, false);
        assert_eq!(s, NodeStatus::Degraded);
        assert_eq!(node.consecutive_failures, 2);

        // Degraded → third failure → Offline (confirmed)
        let s = evaluate_node_status(&mut node, false);
        assert_eq!(s, NodeStatus::Offline);
        assert_eq!(node.consecutive_failures, 3);

        // Offline → continued failure → stays Offline
        let s = evaluate_node_status(&mut node, false);
        assert_eq!(s, NodeStatus::Offline);
        assert_eq!(node.consecutive_failures, 4);

        // Offline → success → immediate recovery to Online
        let s = evaluate_node_status(&mut node, true);
        assert_eq!(s, NodeStatus::Online);
        assert_eq!(node.consecutive_failures, 0);
    }

    #[test]
    fn test_degraded_recovery_before_max() {
        let mut node = make_node(NodeStatus::Online, 0, 5);

        // Fail twice
        evaluate_node_status(&mut node, false);
        evaluate_node_status(&mut node, false);
        assert_eq!(node.consecutive_failures, 2);

        // Recover
        let s = evaluate_node_status(&mut node, true);
        assert_eq!(s, NodeStatus::Online);
        assert_eq!(node.consecutive_failures, 0);
    }
}
