mod common;

use chrono::{Duration, Utc};
use common::{assertions, fixtures, NodeBuilder, TestDatabase};
use net_monitor::models::{MonitorDetail, MonitoringResult, NodeStatus, StatusChange};

#[test]
fn test_database_persistence() {
    let test_db = TestDatabase::new();

    // Add multiple nodes
    let http_node = fixtures::http_node();
    let ping_node = fixtures::ping_node();

    let http_id = test_db.db.add_node(&http_node).unwrap();
    let ping_id = test_db.db.add_node(&ping_node).unwrap();

    // Verify nodes are stored
    let nodes = test_db.db.get_all_nodes().unwrap();
    assert_eq!(nodes.len(), 2);

    // Create a second connection to the same database file to test persistence
    let db2 = net_monitor::database::Database::new(test_db.path()).unwrap();

    // Verify nodes are still there
    let nodes2 = db2.get_all_nodes().unwrap();
    assert_eq!(nodes2.len(), 2);

    // Verify the nodes have the correct data
    let http_node_retrieved = nodes2.iter().find(|n| n.name == "Test HTTP Node").unwrap();
    let ping_node_retrieved = nodes2.iter().find(|n| n.name == "Test Ping Node").unwrap();

    assert_eq!(http_node_retrieved.id, Some(http_id));
    assert_eq!(ping_node_retrieved.id, Some(ping_id));

    assertions::assert_http_node(http_node_retrieved, "https://httpbin.org/status/200", 200);
    assertions::assert_ping_node(ping_node_retrieved, "127.0.0.1", 1, 1);
}

#[test]
fn test_add_and_retrieve_http_node() {
    let test_db = TestDatabase::new();

    let node = fixtures::http_node();
    let node_id = test_db.db.add_node(&node).unwrap();
    assert!(node_id > 0);

    let nodes = test_db.db.get_all_nodes().unwrap();
    assert_eq!(nodes.len(), 1);

    let retrieved = &nodes[0];
    assert_eq!(retrieved.id, Some(node_id));
    assert_eq!(retrieved.name, "Test HTTP Node");
    assertions::assert_http_node(retrieved, "https://httpbin.org/status/200", 200);
}

#[test]
fn test_add_and_retrieve_ping_node() {
    let test_db = TestDatabase::new();

    let node = fixtures::ping_node();
    let node_id = test_db.db.add_node(&node).unwrap();
    assert!(node_id > 0);

    let nodes = test_db.db.get_all_nodes().unwrap();
    assert_eq!(nodes.len(), 1);

    let retrieved = &nodes[0];
    assert_eq!(retrieved.id, Some(node_id));
    assert_eq!(retrieved.name, "Test Ping Node");
    assertions::assert_ping_node(retrieved, "127.0.0.1", 1, 1);
}

#[test]
fn test_update_node() {
    let test_db = TestDatabase::new();

    let mut node = fixtures::http_node();
    let node_id = test_db.db.add_node(&node).unwrap();

    // Update the node
    node.id = Some(node_id);
    node.status = NodeStatus::Online;
    node.response_time = Some(150);

    test_db.db.update_node(&node).unwrap();

    // Verify the update
    let updated_nodes = test_db.db.get_all_nodes().unwrap();
    assert_eq!(updated_nodes.len(), 1);
    assert_eq!(updated_nodes[0].status, NodeStatus::Online);
    assert_eq!(updated_nodes[0].response_time, Some(150));
}

#[test]
fn test_delete_node() {
    let test_db = TestDatabase::new();

    let node = fixtures::http_node();
    let node_id = test_db.db.add_node(&node).unwrap();

    // Verify node exists
    let nodes = test_db.db.get_all_nodes().unwrap();
    assert_eq!(nodes.len(), 1);

    // Delete the node
    test_db.db.delete_node(node_id).unwrap();

    // Verify node is deleted
    let nodes = test_db.db.get_all_nodes().unwrap();
    assert_eq!(nodes.len(), 0);
}

#[test]
fn test_add_multiple_nodes() {
    let test_db = TestDatabase::new();

    // Add multiple nodes of different types
    for i in 0..3 {
        let node = NodeBuilder::new()
            .name(format!("HTTP Node {}", i))
            .http("https://example.com", 200)
            .build();
        test_db.db.add_node(&node).unwrap();
    }

    for i in 0..2 {
        let node = NodeBuilder::new()
            .name(format!("Ping Node {}", i))
            .ping("127.0.0.1", 1, 1)
            .build();
        test_db.db.add_node(&node).unwrap();
    }

    // Verify all nodes were added
    let nodes = test_db.db.get_all_nodes().unwrap();
    assert_eq!(nodes.len(), 5);
}

// ============================================================================
// Unit tests moved from src/database.rs
// ============================================================================

#[test]
fn test_database_creation() {
    let test_db = TestDatabase::new();
    assert!(test_db.path().exists());
}

#[test]
fn test_unit_add_and_get_http_node() {
    let test_db = TestDatabase::new();
    let node = fixtures::unit_test_http_node();

    let id = test_db.db.add_node(&node).unwrap();
    assert!(id > 0);

    let nodes = test_db.db.get_all_nodes().unwrap();
    assert_eq!(nodes.len(), 1);

    let retrieved_node = &nodes[0];
    assert_eq!(retrieved_node.id, Some(id));
    assert_eq!(retrieved_node.name, "Test HTTP Node");
    assert_eq!(retrieved_node.status, NodeStatus::Online);
    assert_eq!(retrieved_node.monitoring_interval, 60);

    match &retrieved_node.detail {
        MonitorDetail::Http {
            url,
            expected_status,
        } => {
            assert_eq!(url, "https://example.com");
            assert_eq!(*expected_status, 200);
        }
        _ => panic!("Expected HTTP monitor detail"),
    }
}

#[test]
fn test_unit_add_and_get_ping_node() {
    let test_db = TestDatabase::new();
    let node = fixtures::unit_test_ping_node();

    let id = test_db.db.add_node(&node).unwrap();
    assert!(id > 0);

    let nodes = test_db.db.get_all_nodes().unwrap();
    assert_eq!(nodes.len(), 1);

    let retrieved_node = &nodes[0];
    assert_eq!(retrieved_node.id, Some(id));
    assert_eq!(retrieved_node.name, "Test Ping Node");
    assert_eq!(retrieved_node.status, NodeStatus::Offline);
    assert_eq!(retrieved_node.monitoring_interval, 30);

    match &retrieved_node.detail {
        MonitorDetail::Ping {
            host,
            count,
            timeout,
        } => {
            assert_eq!(host, "192.168.1.1");
            assert_eq!(*count, 4);
            assert_eq!(*timeout, 5);
        }
        _ => panic!("Expected Ping monitor detail"),
    }
}

#[test]
fn test_unit_update_node() {
    let test_db = TestDatabase::new();
    let mut node = fixtures::unit_test_http_node();

    let id = test_db.db.add_node(&node).unwrap();
    node.id = Some(id);
    node.name = "Updated HTTP Node".to_string();
    node.status = NodeStatus::Offline;

    test_db.db.update_node(&node).unwrap();

    let nodes = test_db.db.get_all_nodes().unwrap();
    assert_eq!(nodes.len(), 1);

    let retrieved_node = &nodes[0];
    assert_eq!(retrieved_node.name, "Updated HTTP Node");
    assert_eq!(retrieved_node.status, NodeStatus::Offline);
}

#[test]
fn test_unit_delete_node() {
    let test_db = TestDatabase::new();
    let node = fixtures::unit_test_http_node();

    let id = test_db.db.add_node(&node).unwrap();

    let nodes = test_db.db.get_all_nodes().unwrap();
    assert_eq!(nodes.len(), 1);

    test_db.db.delete_node(id).unwrap();

    let nodes = test_db.db.get_all_nodes().unwrap();
    assert_eq!(nodes.len(), 0);
}

#[test]
fn test_update_node_runtime_state_preserves_configuration() {
    let test_db = TestDatabase::new();
    let node = fixtures::unit_test_http_node();
    let node_id = test_db.db.add_node(&node).unwrap();

    // Simulate the user editing the node's configuration while a check is in flight
    let mut edited = node.clone();
    edited.id = Some(node_id);
    edited.name = "Renamed by user".to_string();
    edited.monitoring_interval = 123;
    edited.detail = MonitorDetail::Http {
        url: "https://edited.example.com".to_string(),
        expected_status: 204,
    };
    test_db.db.update_node(&edited).unwrap();

    // The monitoring engine still holds the original (stale) configuration but
    // has fresh runtime state from the check it just finished
    let mut stale = node.clone();
    stale.id = Some(node_id);
    stale.status = NodeStatus::Online;
    stale.last_check = Some(Utc::now());
    stale.response_time = Some(42);
    stale.consecutive_failures = 0;
    test_db.db.update_node_runtime_state(&stale).unwrap();

    let stored = test_db.db.get_all_nodes().unwrap().remove(0);

    // Runtime state was written...
    assert_eq!(stored.status, NodeStatus::Online);
    assert!(stored.last_check.is_some());
    assert_eq!(stored.response_time, Some(42));
    assert_eq!(stored.consecutive_failures, 0);

    // ...but the user's configuration edit survived
    assert_eq!(stored.name, "Renamed by user");
    assert_eq!(stored.monitoring_interval, 123);
    assert_eq!(stored.detail, edited.detail);
}

#[test]
fn test_update_node_runtime_state_requires_id() {
    let test_db = TestDatabase::new();
    let node = fixtures::unit_test_http_node();
    assert!(node.id.is_none());
    assert!(test_db.db.update_node_runtime_state(&node).is_err());
}

#[test]
fn test_add_monitoring_result() {
    let test_db = TestDatabase::new();
    let node = fixtures::unit_test_http_node();
    let node_id = test_db.db.add_node(&node).unwrap();

    let result = MonitoringResult {
        id: None,
        node_id,
        timestamp: Utc::now(),
        status: NodeStatus::Online,
        response_time: Some(150),
        details: Some("Success".to_string()),
    };

    let result_id = test_db.db.add_monitoring_result(&result).unwrap();
    assert!(result_id > 0);
}

#[test]
fn test_multiple_nodes() {
    let test_db = TestDatabase::new();

    let http = fixtures::unit_test_http_node();
    let ping = fixtures::unit_test_ping_node();

    test_db.db.add_node(&http).unwrap();
    test_db.db.add_node(&ping).unwrap();

    let nodes = test_db.db.get_all_nodes().unwrap();
    assert_eq!(nodes.len(), 2);

    // Check that nodes are ordered by name
    assert_eq!(nodes[0].name, "Test HTTP Node");
    assert_eq!(nodes[1].name, "Test Ping Node");
}

#[test]
fn test_node_status_parsing() {
    assert_eq!("Online".parse::<NodeStatus>().unwrap(), NodeStatus::Online);
    assert_eq!(
        "Offline".parse::<NodeStatus>().unwrap(),
        NodeStatus::Offline
    );
    // Unknown and invalid strings default to Offline
    assert_eq!(
        "Unknown".parse::<NodeStatus>().unwrap(),
        NodeStatus::Offline
    );
    assert_eq!(
        "Invalid".parse::<NodeStatus>().unwrap(),
        NodeStatus::Offline
    );
}

// Note: test_monitor_detail_to_db_params was removed because to_db_params() is
// a private method. This functionality is indirectly tested through add_node tests.

#[test]
fn test_node_with_response_time() {
    let test_db = TestDatabase::new();
    let mut node = fixtures::unit_test_http_node();
    node.response_time = Some(250);

    let _id = test_db.db.add_node(&node).unwrap();

    let nodes = test_db.db.get_all_nodes().unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].response_time, Some(250));
}

#[test]
fn test_node_with_last_check() {
    let test_db = TestDatabase::new();
    let mut node = fixtures::unit_test_http_node();
    let now = Utc::now();
    node.last_check = Some(now);

    let _id = test_db.db.add_node(&node).unwrap();

    let nodes = test_db.db.get_all_nodes().unwrap();
    assert_eq!(nodes.len(), 1);
    assert!(nodes[0].last_check.is_some());
    // Allow for small time differences due to database operations
    let time_diff = (nodes[0].last_check.unwrap() - now).num_seconds().abs();
    assert!(time_diff < 5);
}

// ========== Status Change Tests ==========

#[test]
fn test_add_status_change() {
    let test_db = TestDatabase::new();
    let node = fixtures::unit_test_http_node();
    let node_id = test_db.db.add_node(&node).unwrap();

    let status_change = StatusChange {
        id: None,
        node_id,
        from_status: NodeStatus::Offline,
        to_status: NodeStatus::Online,
        changed_at: Utc::now(),
        duration_ms: None,
        last_success_at: None,
    };

    let change_id = test_db.db.add_status_change(&status_change).unwrap();
    assert!(change_id > 0);
}

#[test]
fn test_get_status_changes() {
    let test_db = TestDatabase::new();
    let node = fixtures::unit_test_http_node();
    let node_id = test_db.db.add_node(&node).unwrap();

    // Add multiple status changes
    let now = Utc::now();
    let changes = vec![
        StatusChange {
            id: None,
            node_id,
            from_status: NodeStatus::Offline,
            to_status: NodeStatus::Online,
            changed_at: now - Duration::seconds(300),
            duration_ms: None,
            last_success_at: None,
        },
        StatusChange {
            id: None,
            node_id,
            from_status: NodeStatus::Online,
            to_status: NodeStatus::Offline,
            changed_at: now - Duration::seconds(200),
            duration_ms: Some(100000),
            last_success_at: None,
        },
        StatusChange {
            id: None,
            node_id,
            from_status: NodeStatus::Offline,
            to_status: NodeStatus::Online,
            changed_at: now - Duration::seconds(100),
            duration_ms: Some(100000),
            last_success_at: None,
        },
    ];

    for change in &changes {
        test_db.db.add_status_change(change).unwrap();
    }

    // Get all status changes
    let retrieved = test_db.db.get_status_changes(node_id, None).unwrap();
    assert_eq!(retrieved.len(), 3);

    // Verify they're ordered by most recent first
    assert_eq!(retrieved[0].to_status, NodeStatus::Online);
    assert_eq!(retrieved[1].to_status, NodeStatus::Offline);
    assert_eq!(retrieved[2].to_status, NodeStatus::Online);

    // Get limited status changes
    let limited = test_db.db.get_status_changes(node_id, Some(2)).unwrap();
    assert_eq!(limited.len(), 2);
}

#[test]
fn test_get_latest_status_change() {
    let test_db = TestDatabase::new();
    let node = fixtures::unit_test_http_node();
    let node_id = test_db.db.add_node(&node).unwrap();

    // Initially, no status changes
    let latest = test_db.db.get_latest_status_change(node_id).unwrap();
    assert!(latest.is_none());

    // Add a status change
    let now = Utc::now();
    let status_change = StatusChange {
        id: None,
        node_id,
        from_status: NodeStatus::Offline,
        to_status: NodeStatus::Online,
        changed_at: now,
        duration_ms: None,
        last_success_at: None,
    };
    test_db.db.add_status_change(&status_change).unwrap();

    // Get the latest
    let latest = test_db.db.get_latest_status_change(node_id).unwrap();
    assert!(latest.is_some());
    let latest_change = latest.unwrap();
    assert_eq!(latest_change.to_status, NodeStatus::Online);

    // Add another status change
    let second_change = StatusChange {
        id: None,
        node_id,
        from_status: NodeStatus::Online,
        to_status: NodeStatus::Offline,
        changed_at: now + Duration::seconds(60),
        duration_ms: Some(60000),
        last_success_at: None,
    };
    test_db.db.add_status_change(&second_change).unwrap();

    // Latest should now be Offline
    let latest = test_db.db.get_latest_status_change(node_id).unwrap();
    assert!(latest.is_some());
    assert_eq!(latest.unwrap().to_status, NodeStatus::Offline);
}

#[test]
fn test_get_current_status_duration() {
    let test_db = TestDatabase::new();
    let node = fixtures::unit_test_http_node();
    let node_id = test_db.db.add_node(&node).unwrap();

    // No status changes yet
    let duration = test_db.db.get_current_status_duration(node_id).unwrap();
    assert!(duration.is_none(), "never checked, so no duration");

    // A node that has been checked but never changed status has been in its
    // status since that first check.
    let first_check = Utc::now() - Duration::seconds(20);
    test_db
        .db
        .add_monitoring_result(&MonitoringResult {
            id: None,
            node_id,
            timestamp: first_check,
            status: NodeStatus::Online,
            response_time: Some(1),
            details: None,
        })
        .unwrap();
    let since_first_check = test_db
        .db
        .get_current_status_duration(node_id)
        .unwrap()
        .unwrap();
    assert!((19_000..=22_000).contains(&since_first_check));

    // Add a status change 5 seconds ago
    let changed_at = Utc::now() - Duration::seconds(5);
    let status_change = StatusChange {
        id: None,
        node_id,
        from_status: NodeStatus::Offline,
        to_status: NodeStatus::Online,
        changed_at,
        duration_ms: None,
        last_success_at: None,
    };
    test_db.db.add_status_change(&status_change).unwrap();

    // Duration should be approximately 5000ms
    let duration = test_db.db.get_current_status_duration(node_id).unwrap();
    assert!(duration.is_some());
    let duration_ms = duration.unwrap();
    // Allow for some variance due to test execution time
    assert!((4500..=6000).contains(&duration_ms));
}

#[test]
fn test_calculate_uptime_percentage() {
    let test_db = TestDatabase::new();
    let node = fixtures::unit_test_http_node();
    let node_id = test_db.db.add_node(&node).unwrap();

    let start_time = Utc::now() - Duration::seconds(1000);
    let end_time = Utc::now();

    // No status changes - should return 100% (assumes node is online)
    let uptime = test_db
        .db
        .calculate_uptime_percentage(node_id, start_time, end_time)
        .unwrap();
    assert_eq!(uptime, 100.0);

    // Add status changes: Offline for 300s in the middle
    // Timeline: Online (400s) -> Offline (300s) -> Online (300s)
    let changes = vec![
        StatusChange {
            id: None,
            node_id,
            from_status: NodeStatus::Offline,
            to_status: NodeStatus::Online,
            changed_at: start_time,
            duration_ms: None,
            last_success_at: None,
        },
        StatusChange {
            id: None,
            node_id,
            from_status: NodeStatus::Online,
            to_status: NodeStatus::Offline,
            changed_at: start_time + Duration::seconds(400),
            duration_ms: Some(400000),
            last_success_at: None,
        },
        StatusChange {
            id: None,
            node_id,
            from_status: NodeStatus::Offline,
            to_status: NodeStatus::Online,
            changed_at: start_time + Duration::seconds(700),
            duration_ms: Some(300000),
            last_success_at: None,
        },
    ];

    for change in &changes {
        test_db.db.add_status_change(change).unwrap();
    }

    // Uptime should be 100% - (300s offline / 1000s total * 100%) = 70%
    let uptime = test_db
        .db
        .calculate_uptime_percentage(node_id, start_time, end_time)
        .unwrap();
    // Allow for small variance
    assert!((uptime - 70.0).abs() < 1.0, "Expected ~70%, got {}", uptime);
}

#[test]
fn test_uptime_with_offline_period_starting_before_window() {
    // Bug fix test: Offline period starts before window, ends during window
    let test_db = TestDatabase::new();
    let node = fixtures::unit_test_http_node();
    let node_id = test_db.db.add_node(&node).unwrap();

    let base_time = Utc::now() - Duration::seconds(1000);

    // Timeline:
    // T+0: Online
    // T+200: Goes Offline
    // T+500: WINDOW START (node is offline)
    // T+800: Comes back Online
    // T+1500: WINDOW END (node is online)
    let changes = vec![
        StatusChange {
            id: None,
            node_id,
            from_status: NodeStatus::Offline,
            to_status: NodeStatus::Online,
            changed_at: base_time,
            duration_ms: None,
            last_success_at: None,
        },
        StatusChange {
            id: None,
            node_id,
            from_status: NodeStatus::Online,
            to_status: NodeStatus::Offline,
            changed_at: base_time + Duration::seconds(200),
            duration_ms: Some(200000),
            last_success_at: None,
        },
        StatusChange {
            id: None,
            node_id,
            from_status: NodeStatus::Offline,
            to_status: NodeStatus::Online,
            changed_at: base_time + Duration::seconds(800),
            duration_ms: Some(600000),
            last_success_at: None,
        },
    ];

    for change in &changes {
        test_db.db.add_status_change(change).unwrap();
    }

    let window_start = base_time + Duration::seconds(500);
    let window_end = base_time + Duration::seconds(1500);

    // Expected: 300s offline (T+500 to T+800) out of 1000s window = 70% uptime
    let uptime = test_db
        .db
        .calculate_uptime_percentage(node_id, window_start, window_end)
        .unwrap();

    assert!(
        (uptime - 70.0).abs() < 1.0,
        "Expected ~70% uptime, got {}%",
        uptime
    );
}

#[test]
fn test_uptime_with_node_offline_at_window_start() {
    // Bug fix test: Node is offline at window start (status change before window)
    let test_db = TestDatabase::new();
    let node = fixtures::unit_test_http_node();
    let node_id = test_db.db.add_node(&node).unwrap();

    let base_time = Utc::now() - Duration::seconds(1000);

    // Timeline:
    // T+0: Goes Offline (before our window)
    // T+500: WINDOW START (node is offline)
    // T+700: Comes back Online
    // T+1500: WINDOW END (node is online)
    let changes = vec![
        StatusChange {
            id: None,
            node_id,
            from_status: NodeStatus::Online,
            to_status: NodeStatus::Offline,
            changed_at: base_time,
            duration_ms: Some(100000),
            last_success_at: None,
        },
        StatusChange {
            id: None,
            node_id,
            from_status: NodeStatus::Offline,
            to_status: NodeStatus::Online,
            changed_at: base_time + Duration::seconds(700),
            duration_ms: Some(700000),
            last_success_at: None,
        },
    ];

    for change in &changes {
        test_db.db.add_status_change(change).unwrap();
    }

    let window_start = base_time + Duration::seconds(500);
    let window_end = base_time + Duration::seconds(1500);

    // Expected: 200s offline (T+500 to T+700) out of 1000s window = 80% uptime
    let uptime = test_db
        .db
        .calculate_uptime_percentage(node_id, window_start, window_end)
        .unwrap();

    assert!(
        (uptime - 80.0).abs() < 1.0,
        "Expected ~80% uptime, got {}%",
        uptime
    );
}

#[test]
fn test_uptime_with_node_offline_past_window_end() {
    // Bug fix test: Node goes offline during window and stays offline past window end
    let test_db = TestDatabase::new();
    let node = fixtures::unit_test_http_node();
    let node_id = test_db.db.add_node(&node).unwrap();

    let base_time = Utc::now() - Duration::seconds(1000);

    // Timeline:
    // T+0: WINDOW START (node online by default)
    // T+300: Goes Offline
    // T+1000: WINDOW END (node still offline)
    // T+1500: Comes back Online (after window)
    let changes = vec![
        StatusChange {
            id: None,
            node_id,
            from_status: NodeStatus::Online,
            to_status: NodeStatus::Offline,
            changed_at: base_time + Duration::seconds(300),
            duration_ms: Some(300000),
            last_success_at: None,
        },
        StatusChange {
            id: None,
            node_id,
            from_status: NodeStatus::Offline,
            to_status: NodeStatus::Online,
            changed_at: base_time + Duration::seconds(1500),
            duration_ms: Some(1200000),
            last_success_at: None,
        },
    ];

    for change in &changes {
        test_db.db.add_status_change(change).unwrap();
    }

    let window_start = base_time;
    let window_end = base_time + Duration::seconds(1000);

    // Expected: 700s offline (T+300 to T+1000) out of 1000s window = 30% uptime
    let uptime = test_db
        .db
        .calculate_uptime_percentage(node_id, window_start, window_end)
        .unwrap();

    assert!(
        (uptime - 30.0).abs() < 1.0,
        "Expected ~30% uptime, got {}%",
        uptime
    );
}

#[test]
fn test_uptime_with_multiple_transitions_across_boundaries() {
    // Bug fix test: Multiple status changes spanning window boundaries
    let test_db = TestDatabase::new();
    let node = fixtures::unit_test_http_node();
    let node_id = test_db.db.add_node(&node).unwrap();

    let base_time = Utc::now() - Duration::seconds(2000);

    // Timeline:
    // T+0: Offline (before window)
    // T+100: Online
    // T+200: Offline
    // T+500: WINDOW START (offline)
    // T+600: Online
    // T+800: Offline
    // T+1000: Online
    // T+1500: WINDOW END (online)
    // T+1800: Offline (after window)
    let changes = vec![
        StatusChange {
            id: None,
            node_id,
            from_status: NodeStatus::Online,
            to_status: NodeStatus::Offline,
            changed_at: base_time,
            duration_ms: None,
            last_success_at: None,
        },
        StatusChange {
            id: None,
            node_id,
            from_status: NodeStatus::Offline,
            to_status: NodeStatus::Online,
            changed_at: base_time + Duration::seconds(100),
            duration_ms: Some(100000),
            last_success_at: None,
        },
        StatusChange {
            id: None,
            node_id,
            from_status: NodeStatus::Online,
            to_status: NodeStatus::Offline,
            changed_at: base_time + Duration::seconds(200),
            duration_ms: Some(100000),
            last_success_at: None,
        },
        StatusChange {
            id: None,
            node_id,
            from_status: NodeStatus::Offline,
            to_status: NodeStatus::Online,
            changed_at: base_time + Duration::seconds(600),
            duration_ms: Some(400000),
            last_success_at: None,
        },
        StatusChange {
            id: None,
            node_id,
            from_status: NodeStatus::Online,
            to_status: NodeStatus::Offline,
            changed_at: base_time + Duration::seconds(800),
            duration_ms: Some(200000),
            last_success_at: None,
        },
        StatusChange {
            id: None,
            node_id,
            from_status: NodeStatus::Offline,
            to_status: NodeStatus::Online,
            changed_at: base_time + Duration::seconds(1000),
            duration_ms: Some(200000),
            last_success_at: None,
        },
        StatusChange {
            id: None,
            node_id,
            from_status: NodeStatus::Online,
            to_status: NodeStatus::Offline,
            changed_at: base_time + Duration::seconds(1800),
            duration_ms: Some(800000),
            last_success_at: None,
        },
    ];

    for change in &changes {
        test_db.db.add_status_change(change).unwrap();
    }

    let window_start = base_time + Duration::seconds(500);
    let window_end = base_time + Duration::seconds(1500);

    // Within window [500-1500]:
    // - Offline: [500-600] = 100s
    // - Online: [600-800] = 200s
    // - Offline: [800-1000] = 200s
    // - Online: [1000-1500] = 500s
    // Total offline in window: 300s out of 1000s = 70% uptime
    let uptime = test_db
        .db
        .calculate_uptime_percentage(node_id, window_start, window_end)
        .unwrap();

    assert!(
        (uptime - 70.0).abs() < 1.0,
        "Expected ~70% uptime, got {}%",
        uptime
    );
}

#[test]
fn test_uptime_with_invalid_time_window() {
    // Bug fix test: Division by zero prevention
    let test_db = TestDatabase::new();
    let node = fixtures::unit_test_http_node();
    let node_id = test_db.db.add_node(&node).unwrap();

    let time = Utc::now();

    // Test with start_time == end_time
    let result = test_db.db.calculate_uptime_percentage(node_id, time, time);

    assert!(result.is_err(), "Should error on zero-duration window");

    // Test with start_time > end_time
    let result =
        test_db
            .db
            .calculate_uptime_percentage(node_id, time + Duration::seconds(100), time);

    assert!(
        result.is_err(),
        "Should error when start_time is after end_time"
    );
}

#[test]
fn test_uptime_no_status_changes_before_window() {
    // Edge case: No status changes exist before window start
    let test_db = TestDatabase::new();
    let node = fixtures::unit_test_http_node();
    let node_id = test_db.db.add_node(&node).unwrap();

    let base_time = Utc::now();

    // Add a status change that happens AFTER window start
    let change = StatusChange {
        id: None,
        node_id,
        from_status: NodeStatus::Online,
        to_status: NodeStatus::Offline,
        changed_at: base_time + Duration::seconds(500),
        duration_ms: Some(500000),
        last_success_at: None,
    };

    test_db.db.add_status_change(&change).unwrap();

    // Window is before the first status change
    let window_start = base_time;
    let window_end = base_time + Duration::seconds(1000);

    // Expected: Node assumed online initially, then offline from T+500 to T+1000
    // 500s offline out of 1000s = 50% uptime
    let uptime = test_db
        .db
        .calculate_uptime_percentage(node_id, window_start, window_end)
        .unwrap();

    assert!(
        (uptime - 50.0).abs() < 1.0,
        "Expected ~50% uptime, got {}%",
        uptime
    );
}

#[test]
fn test_status_change_with_duration() {
    let test_db = TestDatabase::new();
    let node = fixtures::unit_test_http_node();
    let node_id = test_db.db.add_node(&node).unwrap();

    let status_change = StatusChange {
        id: None,
        node_id,
        from_status: NodeStatus::Online,
        to_status: NodeStatus::Offline,
        changed_at: Utc::now(),
        duration_ms: Some(120000), // 2 minutes
        last_success_at: None,
    };

    test_db.db.add_status_change(&status_change).unwrap();

    let changes = test_db.db.get_status_changes(node_id, None).unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].duration_ms, Some(120000));
}

#[test]
fn test_status_change_helper_methods() {
    // Test is_degradation
    let degradation = StatusChange {
        id: None,
        node_id: 1,
        from_status: NodeStatus::Online,
        to_status: NodeStatus::Offline,
        changed_at: Utc::now(),
        duration_ms: None,
        last_success_at: None,
    };
    assert!(degradation.is_degradation());
    assert!(!degradation.is_recovery());

    // Test is_recovery
    let recovery = StatusChange {
        id: None,
        node_id: 1,
        from_status: NodeStatus::Offline,
        to_status: NodeStatus::Online,
        changed_at: Utc::now(),
        duration_ms: None,
        last_success_at: None,
    };
    assert!(recovery.is_recovery());
    assert!(!recovery.is_degradation());

    // Test description
    assert_eq!(degradation.description(), "Online → Offline");
    assert_eq!(recovery.description(), "Offline → Online");
}

#[test]
fn test_status_change_calculate_duration() {
    let start = Utc::now();
    let end = start + Duration::seconds(120);

    let duration_ms = StatusChange::calculate_duration(start, end);
    assert_eq!(duration_ms, 120000); // 120 seconds = 120000 milliseconds
}

#[test]
fn test_status_changes_cascade_delete() {
    let test_db = TestDatabase::new();
    let node = fixtures::unit_test_http_node();
    let node_id = test_db.db.add_node(&node).unwrap();

    // Add status changes
    let status_change = StatusChange {
        id: None,
        node_id,
        from_status: NodeStatus::Offline,
        to_status: NodeStatus::Online,
        changed_at: Utc::now(),
        duration_ms: None,
        last_success_at: None,
    };
    test_db.db.add_status_change(&status_change).unwrap();

    // Verify status change exists
    let changes = test_db.db.get_status_changes(node_id, None).unwrap();
    assert_eq!(changes.len(), 1);

    // Delete the node
    test_db.db.delete_node(node_id).unwrap();

    // Status changes should be cascaded deleted
    let changes = test_db.db.get_status_changes(node_id, None).unwrap();
    assert_eq!(changes.len(), 0);
}

#[test]
fn test_get_latest_monitoring_result_no_results() {
    let test_db = TestDatabase::new();
    let node = fixtures::unit_test_http_node();
    let node_id = test_db.db.add_node(&node).unwrap();

    // Should return None when no monitoring results exist
    let result = test_db.db.get_latest_monitoring_result(node_id).unwrap();
    assert!(result.is_none());
}

#[test]
fn test_get_latest_monitoring_result_single_result() {
    let test_db = TestDatabase::new();
    let node = fixtures::unit_test_http_node();
    let node_id = test_db.db.add_node(&node).unwrap();

    // Add a monitoring result
    let monitoring_result = MonitoringResult {
        id: None,
        node_id,
        timestamp: Utc::now(),
        status: NodeStatus::Online,
        response_time: Some(100),
        details: Some("Test result".to_string()),
    };
    test_db
        .db
        .add_monitoring_result(&monitoring_result)
        .unwrap();

    // Should return the result
    let result = test_db.db.get_latest_monitoring_result(node_id).unwrap();
    assert!(result.is_some());
    let result = result.unwrap();
    assert_eq!(result.node_id, node_id);
    assert_eq!(result.status, NodeStatus::Online);
    assert_eq!(result.response_time, Some(100));
}

#[test]
fn test_get_latest_monitoring_result_multiple_results() {
    use std::thread::sleep;
    use std::time::Duration;

    let test_db = TestDatabase::new();
    let node = fixtures::unit_test_http_node();
    let node_id = test_db.db.add_node(&node).unwrap();

    // Add first monitoring result (Offline)
    let result1 = MonitoringResult {
        id: None,
        node_id,
        timestamp: Utc::now(),
        status: NodeStatus::Offline,
        response_time: Some(100),
        details: Some("First check".to_string()),
    };
    test_db.db.add_monitoring_result(&result1).unwrap();

    // Wait a bit to ensure different timestamps
    sleep(Duration::from_millis(10));

    // Add second monitoring result (Online) - this should be the latest
    let result2 = MonitoringResult {
        id: None,
        node_id,
        timestamp: Utc::now(),
        status: NodeStatus::Online,
        response_time: Some(150),
        details: Some("Second check".to_string()),
    };
    test_db.db.add_monitoring_result(&result2).unwrap();

    // Should return the most recent result (Online)
    let latest = test_db.db.get_latest_monitoring_result(node_id).unwrap();
    assert!(latest.is_some());
    let latest = latest.unwrap();
    assert_eq!(latest.status, NodeStatus::Online);
    assert_eq!(latest.response_time, Some(150));
    assert_eq!(latest.details, Some("Second check".to_string()));
}

#[test]
fn test_monitoring_result_not_recorded_on_unchanged_status() {
    // This test verifies the behavior that monitoring results should only be
    // recorded when status changes, not on every check
    let test_db = TestDatabase::new();
    let node = fixtures::unit_test_http_node();
    let node_id = test_db.db.add_node(&node).unwrap();

    // Simulate first check - Online status
    let result1 = MonitoringResult {
        id: None,
        node_id,
        timestamp: Utc::now(),
        status: NodeStatus::Online,
        response_time: Some(100),
        details: Some("First check - Online".to_string()),
    };
    test_db.db.add_monitoring_result(&result1).unwrap();

    // Get the latest result
    let latest = test_db.db.get_latest_monitoring_result(node_id).unwrap();
    assert!(latest.is_some());
    let latest_status = latest.unwrap().status;
    assert_eq!(latest_status, NodeStatus::Online);

    // In the actual monitoring code, if the status hasn't changed,
    // add_monitoring_result would NOT be called. This test just verifies
    // that we can check the previous status correctly.

    // Simulate status change - now Offline
    let result2 = MonitoringResult {
        id: None,
        node_id,
        timestamp: Utc::now(),
        status: NodeStatus::Offline,
        response_time: Some(200),
        details: Some("Status changed to Offline".to_string()),
    };
    test_db.db.add_monitoring_result(&result2).unwrap();

    // Verify the latest status is now Offline
    let latest = test_db.db.get_latest_monitoring_result(node_id).unwrap();
    assert!(latest.is_some());
    assert_eq!(latest.unwrap().status, NodeStatus::Offline);
}

// ========== Display Order Tests ==========

#[test]
fn test_display_order_on_new_nodes() {
    let test_db = TestDatabase::new();

    // Add nodes in non-alphabetical order
    let node_c = NodeBuilder::new()
        .name("Charlie")
        .http("https://example.com/c", 200)
        .build();
    let node_a = NodeBuilder::new()
        .name("Alpha")
        .http("https://example.com/a", 200)
        .build();
    let node_b = NodeBuilder::new()
        .name("Bravo")
        .http("https://example.com/b", 200)
        .build();

    test_db.db.add_node(&node_c).unwrap();
    test_db.db.add_node(&node_a).unwrap();
    test_db.db.add_node(&node_b).unwrap();

    // get_all_nodes should return them in insertion order (display_order)
    let nodes = test_db.db.get_all_nodes().unwrap();
    assert_eq!(nodes.len(), 3);
    assert_eq!(nodes[0].name, "Charlie");
    assert_eq!(nodes[1].name, "Alpha");
    assert_eq!(nodes[2].name, "Bravo");
}

#[test]
fn test_update_display_orders() {
    let test_db = TestDatabase::new();

    let node_a = NodeBuilder::new()
        .name("Alpha")
        .http("https://example.com/a", 200)
        .build();
    let node_b = NodeBuilder::new()
        .name("Bravo")
        .http("https://example.com/b", 200)
        .build();
    let node_c = NodeBuilder::new()
        .name("Charlie")
        .http("https://example.com/c", 200)
        .build();

    let id_a = test_db.db.add_node(&node_a).unwrap();
    let id_b = test_db.db.add_node(&node_b).unwrap();
    let id_c = test_db.db.add_node(&node_c).unwrap();

    // Reverse the order: Charlie, Bravo, Alpha
    let new_order = vec![(id_c, 0i64), (id_b, 1i64), (id_a, 2i64)];
    test_db.db.update_node_display_orders(&new_order).unwrap();

    let nodes = test_db.db.get_all_nodes().unwrap();
    assert_eq!(nodes.len(), 3);
    assert_eq!(nodes[0].name, "Charlie");
    assert_eq!(nodes[1].name, "Bravo");
    assert_eq!(nodes[2].name, "Alpha");
}

#[test]
fn test_degraded_state_survives_reopen() {
    let test_db = TestDatabase::new();

    let mut node = NodeBuilder::new()
        .name("Soft Failing Node")
        .http("https://example.com/degraded", 200)
        .build();
    node.status = NodeStatus::Degraded;
    node.consecutive_failures = 2;
    node.max_check_attempts = 3;

    let id = test_db.db.add_node(&node).unwrap();

    // Reopening the database runs migrations again, simulating an app restart.
    let db2 = net_monitor::database::Database::new(test_db.path()).unwrap();

    let reopened = db2
        .get_all_nodes()
        .unwrap()
        .into_iter()
        .find(|n| n.id == Some(id))
        .expect("node should still exist after reopen");

    // A soft-state node must stay Degraded across restarts. Promoting it to
    // Offline would report a hard down that never happened, and resetting the
    // failure counter would restart confirmation from zero.
    assert_eq!(reopened.status, NodeStatus::Degraded);
    assert_eq!(reopened.consecutive_failures, 2);
}

// ========== Status change last success ==========

#[test]
fn test_status_change_round_trips_last_success_at() {
    let test_db = TestDatabase::new();
    let node_id = test_db
        .db
        .add_node(&fixtures::unit_test_http_node())
        .unwrap();
    let last_success = Utc::now() - Duration::minutes(5);

    test_db
        .db
        .add_status_change(&StatusChange {
            id: None,
            node_id,
            from_status: NodeStatus::Online,
            to_status: NodeStatus::Degraded,
            changed_at: Utc::now(),
            duration_ms: Some(1000),
            last_success_at: Some(last_success),
        })
        .unwrap();

    let stored = test_db
        .db
        .get_latest_status_change(node_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        stored.last_success_at.map(|t| t.timestamp_millis()),
        Some(last_success.timestamp_millis())
    );
}

#[test]
fn test_get_status_changes_ascending_is_oldest_first() {
    let test_db = TestDatabase::new();
    let node_id = test_db
        .db
        .add_node(&fixtures::unit_test_http_node())
        .unwrap();
    let now = Utc::now();
    for (offset, to_status) in [(300, NodeStatus::Offline), (200, NodeStatus::Online)] {
        test_db
            .db
            .add_status_change(&StatusChange {
                id: None,
                node_id,
                from_status: NodeStatus::Online,
                to_status,
                changed_at: now - Duration::seconds(offset),
                duration_ms: None,
                last_success_at: None,
            })
            .unwrap();
    }

    let changes = test_db.db.get_status_changes_ascending(node_id).unwrap();
    assert_eq!(changes.len(), 2);
    assert_eq!(changes[0].to_status, NodeStatus::Offline);
    assert_eq!(changes[1].to_status, NodeStatus::Online);
    assert!(changes[0].changed_at < changes[1].changed_at);
}

#[test]
fn test_last_success_at_is_added_to_an_old_database() {
    let test_db = TestDatabase::new();
    let path = test_db.path().to_path_buf();

    // Recreate the schema the previous release used.
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(
            "DROP TABLE status_changes;
             CREATE TABLE status_changes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                node_id INTEGER NOT NULL,
                from_status TEXT NOT NULL,
                to_status TEXT NOT NULL,
                changed_at TEXT NOT NULL,
                duration_ms INTEGER
             );
             INSERT INTO status_changes (node_id, from_status, to_status, changed_at, duration_ms)
             VALUES (1, 'Online', 'Offline', '2026-01-01T00:00:00+00:00', 5000);",
        )
        .unwrap();
    }

    let db = net_monitor::database::Database::new(&path).unwrap();
    let changes = db.get_status_changes_ascending(1).unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].duration_ms, Some(5000));
    assert_eq!(changes[0].last_success_at, None);
}

// ========== First check time ==========

#[test]
fn test_get_first_check_time() {
    let test_db = TestDatabase::new();
    let node_id = test_db
        .db
        .add_node(&fixtures::unit_test_http_node())
        .unwrap();
    assert_eq!(test_db.db.get_first_check_time(node_id).unwrap(), None);

    let first = Utc::now() - Duration::hours(2);
    for timestamp in [first + Duration::hours(1), first] {
        test_db
            .db
            .add_monitoring_result(&MonitoringResult {
                id: None,
                node_id,
                timestamp,
                status: NodeStatus::Online,
                response_time: Some(1),
                details: None,
            })
            .unwrap();
    }

    assert_eq!(
        test_db
            .db
            .get_first_check_time(node_id)
            .unwrap()
            .map(|t| t.timestamp_millis()),
        Some(first.timestamp_millis())
    );
}

// ========== record_check ==========

#[test]
fn test_record_check_writes_state_change_and_result_together() {
    let test_db = TestDatabase::new();
    let mut node = fixtures::unit_test_http_node();
    let node_id = test_db.db.add_node(&node).unwrap();
    node.id = Some(node_id);
    node.status = NodeStatus::Degraded;
    node.consecutive_failures = 1;

    let checked_at = Utc::now();
    let change = StatusChange {
        id: None,
        node_id,
        from_status: NodeStatus::Online,
        to_status: NodeStatus::Degraded,
        changed_at: checked_at,
        duration_ms: Some(60_000),
        last_success_at: Some(checked_at - Duration::minutes(1)),
    };
    let result = MonitoringResult {
        id: None,
        node_id,
        timestamp: checked_at,
        status: NodeStatus::Degraded,
        response_time: None,
        details: Some("timed out".to_string()),
    };

    test_db
        .db
        .record_check(&node, Some(&change), Some(&result))
        .unwrap();

    let stored_node = &test_db.db.get_all_nodes().unwrap()[0];
    assert_eq!(stored_node.status, NodeStatus::Degraded);
    assert_eq!(stored_node.consecutive_failures, 1);
    let stored_change = test_db
        .db
        .get_latest_status_change(node_id)
        .unwrap()
        .unwrap();
    assert_eq!(stored_change.to_status, NodeStatus::Degraded);
    let stored_result = test_db
        .db
        .get_latest_monitoring_result(node_id)
        .unwrap()
        .unwrap();
    assert_eq!(stored_result.details.as_deref(), Some("timed out"));
}

#[test]
fn test_record_check_with_nothing_but_runtime_state() {
    let test_db = TestDatabase::new();
    let mut node = fixtures::unit_test_http_node();
    let node_id = test_db.db.add_node(&node).unwrap();
    node.id = Some(node_id);
    node.response_time = Some(42);

    test_db.db.record_check(&node, None, None).unwrap();

    assert_eq!(
        test_db.db.get_all_nodes().unwrap()[0].response_time,
        Some(42)
    );
    assert!(test_db
        .db
        .get_latest_status_change(node_id)
        .unwrap()
        .is_none());
    assert!(test_db
        .db
        .get_latest_monitoring_result(node_id)
        .unwrap()
        .is_none());
}

#[test]
fn test_record_check_is_atomic() {
    let test_db = TestDatabase::new();
    let mut node = fixtures::unit_test_http_node();
    let node_id = test_db.db.add_node(&node).unwrap();
    node.id = Some(node_id);
    node.status = NodeStatus::Offline;

    // The runtime-state write refuses a node without an id, which fails the
    // transaction after nothing and before the status change.
    let mut unsaved = node.clone();
    unsaved.id = None;
    let change = StatusChange {
        id: None,
        node_id,
        from_status: NodeStatus::Online,
        to_status: NodeStatus::Offline,
        changed_at: Utc::now(),
        duration_ms: None,
        last_success_at: None,
    };

    assert!(test_db
        .db
        .record_check(&unsaved, Some(&change), None)
        .is_err());

    assert!(
        test_db
            .db
            .get_latest_status_change(node_id)
            .unwrap()
            .is_none(),
        "a failed write leaves no partial history behind"
    );
    assert_eq!(
        test_db.db.get_all_nodes().unwrap()[0].status,
        NodeStatus::Online,
        "and the node's status is untouched"
    );
}

// ========== Engine runs ==========

#[test]
fn test_engine_runs_record_start_and_heartbeat() {
    let test_db = TestDatabase::new();
    let started = Utc::now() - Duration::minutes(10);
    let run_id = test_db.db.start_engine_run(started).unwrap();

    let runs = test_db.db.get_engine_runs().unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].id, run_id);
    assert_eq!(
        runs[0].started_at.timestamp_millis(),
        started.timestamp_millis()
    );
    assert_eq!(
        runs[0].last_alive_at.timestamp_millis(),
        started.timestamp_millis(),
        "a run is alive at the moment it starts"
    );

    let alive = started + Duration::minutes(5);
    test_db.db.touch_engine_run(run_id, alive).unwrap();
    let runs = test_db.db.get_engine_runs().unwrap();
    assert_eq!(
        runs[0].last_alive_at.timestamp_millis(),
        alive.timestamp_millis()
    );
}

#[test]
fn test_engine_runs_are_oldest_first() {
    let test_db = TestDatabase::new();
    let now = Utc::now();
    test_db.db.start_engine_run(now).unwrap();
    test_db
        .db
        .start_engine_run(now - Duration::hours(1))
        .unwrap();

    let runs = test_db.db.get_engine_runs().unwrap();
    assert_eq!(runs.len(), 2);
    assert!(runs[0].started_at < runs[1].started_at);
}

#[test]
fn test_engine_runs_table_is_added_to_an_old_database() {
    let test_db = TestDatabase::new();
    let path = test_db.path().to_path_buf();
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch("DROP TABLE engine_runs;").unwrap();
    }
    let db = net_monitor::database::Database::new(&path).unwrap();
    assert!(db.get_engine_runs().unwrap().is_empty());
    db.start_engine_run(Utc::now()).unwrap();
    assert_eq!(db.get_engine_runs().unwrap().len(), 1);
}
