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
    assert_eq!(
        "Unknown".parse::<NodeStatus>().unwrap(),
        NodeStatus::Unknown
    );
    assert_eq!(
        "Invalid".parse::<NodeStatus>().unwrap(),
        NodeStatus::Unknown
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
        from_status: NodeStatus::Unknown,
        to_status: NodeStatus::Online,
        changed_at: Utc::now(),
        duration_ms: None,
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
            from_status: NodeStatus::Unknown,
            to_status: NodeStatus::Online,
            changed_at: now - Duration::seconds(300),
            duration_ms: None,
        },
        StatusChange {
            id: None,
            node_id,
            from_status: NodeStatus::Online,
            to_status: NodeStatus::Offline,
            changed_at: now - Duration::seconds(200),
            duration_ms: Some(100000),
        },
        StatusChange {
            id: None,
            node_id,
            from_status: NodeStatus::Offline,
            to_status: NodeStatus::Online,
            changed_at: now - Duration::seconds(100),
            duration_ms: Some(100000),
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
        from_status: NodeStatus::Unknown,
        to_status: NodeStatus::Online,
        changed_at: now,
        duration_ms: None,
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
    assert!(duration.is_none());

    // Add a status change 5 seconds ago
    let changed_at = Utc::now() - Duration::seconds(5);
    let status_change = StatusChange {
        id: None,
        node_id,
        from_status: NodeStatus::Unknown,
        to_status: NodeStatus::Online,
        changed_at,
        duration_ms: None,
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

    // No status changes - should return 0%
    let uptime = test_db
        .db
        .calculate_uptime_percentage(node_id, start_time, end_time)
        .unwrap();
    assert_eq!(uptime, 0.0);

    // Add status changes: Online for 400s, Offline for 300s, Online again for 300s
    let changes = vec![
        StatusChange {
            id: None,
            node_id,
            from_status: NodeStatus::Unknown,
            to_status: NodeStatus::Online,
            changed_at: start_time,
            duration_ms: None,
        },
        StatusChange {
            id: None,
            node_id,
            from_status: NodeStatus::Online,
            to_status: NodeStatus::Offline,
            changed_at: start_time + Duration::seconds(400),
            duration_ms: Some(400000),
        },
        StatusChange {
            id: None,
            node_id,
            from_status: NodeStatus::Offline,
            to_status: NodeStatus::Online,
            changed_at: start_time + Duration::seconds(700),
            duration_ms: Some(300000),
        },
    ];

    for change in &changes {
        test_db.db.add_status_change(change).unwrap();
    }

    // Uptime should be (400s + 300s) / 1000s = 70%
    let uptime = test_db
        .db
        .calculate_uptime_percentage(node_id, start_time, end_time)
        .unwrap();
    // Allow for small variance
    assert!((uptime - 70.0).abs() < 1.0);
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
        from_status: NodeStatus::Unknown,
        to_status: NodeStatus::Online,
        changed_at: Utc::now(),
        duration_ms: None,
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
