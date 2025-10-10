mod common;

use chrono::Utc;
use common::{assertions, fixtures, NodeBuilder, TestDatabase};
use net_monitor::models::{MonitorDetail, MonitoringResult, NodeStatus};

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
