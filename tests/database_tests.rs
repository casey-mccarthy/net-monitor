mod common;

use common::{assertions, fixtures, NodeBuilder, TestDatabase};
use net_monitor::models::NodeStatus;

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
