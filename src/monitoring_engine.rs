//! Monitoring engine that manages the background monitoring loop.
//!
//! This module extracts all monitoring orchestration logic out of the TUI,
//! implementing a soft/hard state model inspired by Nagios, Zabbix, and
//! Uptime Kuma to reduce false positives:
//!
//! - **Online**: Node is responding. A single check failure transitions to Degraded.
//! - **Degraded** (soft state): Node failed a check but hasn't yet been confirmed down.
//!   Retries happen at a shorter `retry_interval`. No status change event is recorded.
//! - **Offline** (hard state): Node has failed `max_check_attempts` consecutive checks.
//!   A status change event is recorded and persisted.
//!
//! Recovery from either Degraded or Offline is immediate on the first successful check.

use crate::database::Database;
use crate::models::{Node, NodeStatus, StatusChange};
use crate::monitor::check_node;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use tracing::info;

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
fn run_monitoring_loop(
    db: Database,
    initial_nodes: Vec<Node>,
    update_tx: mpsc::Sender<Node>,
    stop_rx: mpsc::Receiver<()>,
    config_rx: mpsc::Receiver<NodeConfigUpdate>,
) {
    let mut last_check_times: HashMap<i64, Instant> = HashMap::new();

    // Initialize previous_statuses from database to avoid duplicate records on restart
    let mut previous_statuses: HashMap<i64, NodeStatus> = initial_nodes
        .iter()
        .filter_map(|n| {
            n.id.and_then(|id| {
                db.get_latest_monitoring_result(id)
                    .ok()
                    .flatten()
                    .map(|result| (id, result.status))
                    .or(Some((id, n.status)))
            })
        })
        .collect();

    let mut last_status_change_times: HashMap<i64, DateTime<Utc>> = HashMap::new();
    let mut current_nodes = initial_nodes;
    let runtime = tokio::runtime::Runtime::new().unwrap();

    loop {
        // Process configuration updates
        process_config_updates(
            &config_rx,
            &db,
            &mut current_nodes,
            &mut last_check_times,
            &mut previous_statuses,
            &mut last_status_change_times,
        );

        // Check each node
        for node in &mut current_nodes {
            let node_id = node.id.unwrap_or(0);
            if node_id == 0 {
                continue;
            }

            if !should_check_node(node, node_id, &last_check_times) {
                continue;
            }

            last_check_times.insert(node_id, Instant::now());
            let previous_status = previous_statuses.get(&node_id).copied();
            let result = runtime.block_on(check_node(node));

            if let Ok(mut check_result) = result {
                let check_succeeded = check_result.status == NodeStatus::Online;

                // Apply soft/hard state logic
                let new_status = evaluate_node_status(node, check_succeeded);

                check_result.status = new_status;

                // Record status change events (only for confirmed transitions)
                if let Some(prev_status) = previous_status {
                    if should_record_status_change(prev_status, new_status) {
                        let current_time = Utc::now();
                        let duration_ms =
                            last_status_change_times.get(&node_id).map(|last_change| {
                                StatusChange::calculate_duration(*last_change, current_time)
                            });

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
                }

                previous_statuses.insert(node_id, new_status);
                node.status = new_status;
                node.last_check = Some(check_result.timestamp);
                node.response_time = check_result.response_time;
                check_result.node_id = node_id;

                let _ = db.update_node(node);

                // Record monitoring result on confirmed status changes or first check
                if let Some(prev_status) = previous_status {
                    if should_record_status_change(prev_status, new_status) {
                        let _ = db.add_monitoring_result(&check_result);
                    }
                } else {
                    // First check ever for this node
                    let _ = db.add_monitoring_result(&check_result);
                }

                if update_tx.send(node.clone()).is_err() {
                    break;
                }
            }
        }

        // Check for stop signal with 1-second timeout
        match stop_rx.recv_timeout(Duration::from_secs(1)) {
            Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
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
                        let status = db
                            .get_latest_monitoring_result(node_id)
                            .ok()
                            .flatten()
                            .map(|result| result.status)
                            .unwrap_or(node.status);
                        previous_statuses.insert(node_id, status);
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
    use crate::models::{MonitorDetail, DEFAULT_MAX_CHECK_ATTEMPTS};

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
            credential_id: None,
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
