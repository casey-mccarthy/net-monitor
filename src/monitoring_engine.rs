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
//! Every transition between the three states is persisted as a `StatusChange`, along
//! with the monitoring result that caused it; results for checks that leave the status
//! unchanged are not stored.
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
use tracing::info;

/// How often the loop wakes up to launch due checks and apply finished ones.
const TICK: Duration = Duration::from_millis(250);

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
    let mut previous_statuses: HashMap<i64, NodeStatus> = HashMap::new();
    let mut last_status_change_times: HashMap<i64, DateTime<Utc>> = HashMap::new();
    for node in &initial_nodes {
        if let Some(node_id) = node.id {
            let (status, last_change) = load_node_state(&db, node_id, node.status);
            previous_statuses.insert(node_id, status);
            if let Some(changed_at) = last_change {
                last_status_change_times.insert(node_id, changed_at);
            }
        }
    }

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
            &mut previous_statuses,
            &mut last_status_change_times,
        );

        // Apply every check that finished since the last tick.
        while let Ok((node_id, result)) = result_rx.try_recv() {
            in_flight.remove(&node_id);
            let Ok(check_result) = result else { continue };
            // The node may have been deleted while its check was in flight.
            let Some(node) = current_nodes.iter_mut().find(|n| n.id == Some(node_id)) else {
                continue;
            };
            apply_check_result(
                &db,
                node,
                check_result,
                &mut previous_statuses,
                &mut last_status_change_times,
            );
            if update_tx.send(node.clone()).is_err() {
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

        match stop_rx.recv_timeout(TICK) {
            Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }

    // Don't wait for in-flight checks (a ping can take count × timeout seconds).
    runtime.shutdown_background();
}

/// Folds a finished check into the node: runs the soft/hard state machine,
/// records the transition and its result when the status changed (or on the
/// node's first check ever), and persists the node's runtime state.
fn apply_check_result(
    db: &Database,
    node: &mut Node,
    mut check_result: MonitoringResult,
    previous_statuses: &mut HashMap<i64, NodeStatus>,
    last_status_change_times: &mut HashMap<i64, DateTime<Utc>>,
) {
    let node_id = node.id.unwrap_or(0);
    let previous_status = previous_statuses.get(&node_id).copied();
    let check_succeeded = check_result.status == NodeStatus::Online;

    let new_status = evaluate_node_status(node, check_succeeded);
    check_result.status = new_status;
    check_result.node_id = node_id;

    let status_changed =
        previous_status.is_some_and(|prev| should_record_status_change(prev, new_status));

    if let (Some(prev_status), true) = (previous_status, status_changed) {
        let current_time = Utc::now();
        let duration_ms = last_status_change_times
            .get(&node_id)
            .map(|last_change| StatusChange::calculate_duration(*last_change, current_time));

        let status_change = StatusChange {
            id: None,
            node_id,
            from_status: prev_status,
            to_status: new_status,
            changed_at: current_time,
            duration_ms,
        };

        let _ = db.add_status_change(&status_change);
        last_status_change_times.insert(node_id, current_time);
    }

    previous_statuses.insert(node_id, new_status);
    node.status = new_status;
    node.last_check = Some(check_result.timestamp);
    node.response_time = check_result.response_time;

    // Persist only runtime state: the user may have edited this node's
    // configuration while the check was in flight, and writing our copy
    // back would clobber that edit in the database.
    let _ = db.update_node_runtime_state(node);

    // Record the monitoring result on confirmed status changes or the first check ever.
    if status_changed || previous_status.is_none() {
        let _ = db.add_monitoring_result(&check_result);
    }
}

/// Loads the persisted state the engine needs for a node: the status it was
/// last known to be in and when its most recent status change happened.
///
/// The status comes from the latest monitoring result when one exists,
/// otherwise from the node record itself (`fallback_status`). The timestamp
/// comes from the latest recorded status change, or `None` if there has
/// never been one.
fn load_node_state(
    db: &Database,
    node_id: i64,
    fallback_status: NodeStatus,
) -> (NodeStatus, Option<DateTime<Utc>>) {
    let status = db
        .get_latest_monitoring_result(node_id)
        .ok()
        .flatten()
        .map(|result| result.status)
        .unwrap_or(fallback_status);

    let last_change = db
        .get_latest_status_change(node_id)
        .ok()
        .flatten()
        .map(|change| change.changed_at);

    (status, last_change)
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

/// Determines whether a status change should be recorded as an event.
///
/// We only record transitions between the three confirmed display states
/// (Online, Degraded, Offline) when they actually change. Degraded→Degraded
/// is not a transition.
fn should_record_status_change(prev: NodeStatus, new: NodeStatus) -> bool {
    prev != new
}

/// Process incoming configuration updates from the TUI.
fn process_config_updates(
    config_rx: &mpsc::Receiver<NodeConfigUpdate>,
    db: &Database,
    current_nodes: &mut Vec<Node>,
    last_check_times: &mut HashMap<i64, Instant>,
    previous_statuses: &mut HashMap<i64, NodeStatus>,
    last_status_change_times: &mut HashMap<i64, DateTime<Utc>>,
) {
    while let Ok(config_update) = config_rx.try_recv() {
        match config_update {
            NodeConfigUpdate::Add(node) => {
                if !current_nodes.iter().any(|n| n.id == node.id) {
                    if let Some(node_id) = node.id {
                        let (status, last_change) = load_node_state(db, node_id, node.status);
                        previous_statuses.insert(node_id, status);
                        if let Some(changed_at) = last_change {
                            last_status_change_times.insert(node_id, changed_at);
                        }
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
                previous_statuses.remove(&node_id);
                last_status_change_times.remove(&node_id);
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

    #[test]
    fn test_load_node_state_without_history_uses_fallback() {
        let (_dir, db) = temp_db();
        let node_id = db.add_node(&make_node(NodeStatus::Offline, 0, 3)).unwrap();

        let (status, last_change) = load_node_state(&db, node_id, NodeStatus::Degraded);
        assert_eq!(status, NodeStatus::Degraded);
        assert_eq!(last_change, None);
    }

    #[test]
    fn test_load_node_state_restores_status_and_change_time() {
        let (_dir, db) = temp_db();
        let node_id = db.add_node(&make_node(NodeStatus::Online, 0, 3)).unwrap();

        let changed_at = Utc::now() - chrono::Duration::minutes(42);
        db.add_status_change(&StatusChange {
            id: None,
            node_id,
            from_status: NodeStatus::Online,
            to_status: NodeStatus::Offline,
            changed_at,
            duration_ms: Some(1000),
        })
        .unwrap();
        db.add_monitoring_result(&MonitoringResult {
            id: None,
            node_id,
            timestamp: changed_at,
            status: NodeStatus::Offline,
            response_time: None,
            details: None,
        })
        .unwrap();

        let (status, last_change) = load_node_state(&db, node_id, NodeStatus::Online);
        assert_eq!(status, NodeStatus::Offline);
        // Compare at millisecond precision: RFC 3339 storage may drop sub-ms digits
        assert_eq!(
            last_change.map(|t| t.timestamp_millis()),
            Some(changed_at.timestamp_millis())
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

    // -- apply_check_result tests --

    #[test]
    fn test_apply_first_check_records_result_but_no_transition() {
        let (_dir, db) = temp_db();
        let mut node = make_node(NodeStatus::Online, 0, 3);
        let node_id = db.add_node(&node).unwrap();
        node.id = Some(node_id);
        let mut previous_statuses = HashMap::new();
        let mut last_changes = HashMap::new();

        apply_check_result(
            &db,
            &mut node,
            make_result(NodeStatus::Online),
            &mut previous_statuses,
            &mut last_changes,
        );

        assert_eq!(node.status, NodeStatus::Online);
        assert_eq!(node.response_time, Some(12));
        assert!(node.last_check.is_some());
        assert_eq!(previous_statuses.get(&node_id), Some(&NodeStatus::Online));
        let stored = db.get_latest_monitoring_result(node_id).unwrap().unwrap();
        assert_eq!(stored.node_id, node_id);
        assert!(db.get_latest_status_change(node_id).unwrap().is_none());
        assert_eq!(
            db.get_all_nodes().unwrap()[0].status,
            NodeStatus::Online,
            "runtime state is persisted"
        );
    }

    #[test]
    fn test_apply_transition_records_change_with_duration() {
        let (_dir, db) = temp_db();
        let mut node = make_node(NodeStatus::Online, 0, 3);
        let node_id = db.add_node(&node).unwrap();
        node.id = Some(node_id);
        let mut previous_statuses = HashMap::from([(node_id, NodeStatus::Online)]);
        let mut last_changes =
            HashMap::from([(node_id, Utc::now() - chrono::Duration::seconds(30))]);

        apply_check_result(
            &db,
            &mut node,
            make_result(NodeStatus::Offline),
            &mut previous_statuses,
            &mut last_changes,
        );

        assert_eq!(node.status, NodeStatus::Degraded);
        assert_eq!(node.consecutive_failures, 1);
        assert_eq!(node.response_time, None);
        let change = db.get_latest_status_change(node_id).unwrap().unwrap();
        assert_eq!(change.from_status, NodeStatus::Online);
        assert_eq!(change.to_status, NodeStatus::Degraded);
        assert!(change.duration_ms.unwrap() >= 29_000);
        assert_eq!(
            db.get_latest_monitoring_result(node_id)
                .unwrap()
                .unwrap()
                .status,
            NodeStatus::Degraded,
            "the stored result carries the evaluated status, not the raw check"
        );
        assert!(last_changes[&node_id] > Utc::now() - chrono::Duration::seconds(5));
    }

    #[test]
    fn test_apply_unchanged_status_stores_nothing() {
        let (_dir, db) = temp_db();
        let mut node = make_node(NodeStatus::Online, 0, 3);
        let node_id = db.add_node(&node).unwrap();
        node.id = Some(node_id);
        let mut previous_statuses = HashMap::from([(node_id, NodeStatus::Online)]);
        let mut last_changes = HashMap::new();

        apply_check_result(
            &db,
            &mut node,
            make_result(NodeStatus::Online),
            &mut previous_statuses,
            &mut last_changes,
        );

        assert!(db.get_latest_monitoring_result(node_id).unwrap().is_none());
        assert!(db.get_latest_status_change(node_id).unwrap().is_none());
        assert!(node.last_check.is_some(), "runtime state still updates");
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

    // -- should_record_status_change tests --

    #[test]
    fn test_same_status_no_record() {
        assert!(!should_record_status_change(
            NodeStatus::Online,
            NodeStatus::Online
        ));
        assert!(!should_record_status_change(
            NodeStatus::Offline,
            NodeStatus::Offline
        ));
        assert!(!should_record_status_change(
            NodeStatus::Degraded,
            NodeStatus::Degraded
        ));
    }

    #[test]
    fn test_different_status_records() {
        assert!(should_record_status_change(
            NodeStatus::Online,
            NodeStatus::Degraded
        ));
        assert!(should_record_status_change(
            NodeStatus::Degraded,
            NodeStatus::Offline
        ));
        assert!(should_record_status_change(
            NodeStatus::Offline,
            NodeStatus::Online
        ));
        assert!(should_record_status_change(
            NodeStatus::Degraded,
            NodeStatus::Online
        ));
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
