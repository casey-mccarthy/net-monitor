mod common;

use common::{fixtures, NodeBuilder, TestDatabase};
use net_monitor::models::NodeStatus;
use net_monitor::monitor::check_node;

#[tokio::test]
#[cfg_attr(not(feature = "network-tests"), ignore)]
async fn test_full_monitoring_workflow() {
    let test_db = TestDatabase::new();

    // 1. Add a node to the database
    let node = fixtures::http_node();
    let node_id = test_db.db.add_node(&node).unwrap();
    assert!(node_id > 0);

    // 2. Retrieve the node from database
    let nodes = test_db.db.get_all_nodes().unwrap();
    assert_eq!(nodes.len(), 1);
    let mut retrieved_node = nodes[0].clone();
    retrieved_node.id = Some(node_id);

    // 3. Check the node (monitor it)
    let monitoring_result = check_node(&retrieved_node).await.unwrap();
    assert_eq!(monitoring_result.node_id, node_id);
    assert_eq!(monitoring_result.status, NodeStatus::Online);
    assert!(monitoring_result.response_time.is_some());
    assert!(monitoring_result.details.is_some());

    // 4. Store the monitoring result
    let result_id = test_db
        .db
        .add_monitoring_result(&monitoring_result)
        .unwrap();
    assert!(result_id > 0);

    // 5. Update the node with the new status
    retrieved_node.status = monitoring_result.status;
    retrieved_node.last_check = Some(monitoring_result.timestamp);
    retrieved_node.response_time = monitoring_result.response_time;
    test_db.db.update_node(&retrieved_node).unwrap();

    // 6. Verify the update
    let updated_nodes = test_db.db.get_all_nodes().unwrap();
    assert_eq!(updated_nodes.len(), 1);
    assert_eq!(updated_nodes[0].status, NodeStatus::Online);
    assert!(updated_nodes[0].last_check.is_some());
    assert!(updated_nodes[0].response_time.is_some());
}

#[tokio::test]
#[cfg_attr(not(feature = "network-tests"), ignore)]
async fn test_monitoring_failure_workflow() {
    let test_db = TestDatabase::new();

    // Create a node that will fail (expecting 200 but will get 404)
    let node = fixtures::http_failure_node();

    let node_id = test_db.db.add_node(&node).unwrap();
    let nodes = test_db.db.get_all_nodes().unwrap();
    let mut retrieved_node = nodes[0].clone();
    retrieved_node.id = Some(node_id);

    // Check the node (should fail)
    let monitoring_result = check_node(&retrieved_node).await.unwrap();
    assert_eq!(monitoring_result.node_id, node_id);
    assert_eq!(monitoring_result.status, NodeStatus::Offline);
    assert!(monitoring_result.response_time.is_some());
    assert!(monitoring_result.details.is_some());

    // Store the monitoring result
    let result_id = test_db
        .db
        .add_monitoring_result(&monitoring_result)
        .unwrap();
    assert!(result_id > 0);

    // Update the node with the failure status
    retrieved_node.status = monitoring_result.status;
    retrieved_node.last_check = Some(monitoring_result.timestamp);
    retrieved_node.response_time = monitoring_result.response_time;
    test_db.db.update_node(&retrieved_node).unwrap();

    // Verify the update
    let updated_nodes = test_db.db.get_all_nodes().unwrap();
    assert_eq!(updated_nodes.len(), 1);
    assert_eq!(updated_nodes[0].status, NodeStatus::Offline);
}

#[tokio::test]
#[cfg_attr(not(feature = "network-tests"), ignore)]
async fn test_concurrent_monitoring() {
    let test_db = TestDatabase::new();

    // Create multiple nodes
    let mut nodes = Vec::new();
    for i in 0..3 {
        let node = NodeBuilder::new()
            .name(format!("Concurrent Test Node {}", i))
            .http("https://httpbin.org/status/200", 200)
            .build();
        let node_id = test_db.db.add_node(&node).unwrap();
        nodes.push((node_id, node));
    }

    // Monitor all nodes concurrently
    let mut handles = Vec::new();
    for (node_id, node) in nodes {
        let db_clone = test_db.db.clone();
        let handle = tokio::spawn(async move {
            let mut node_with_id = node;
            node_with_id.id = Some(node_id);

            let result = check_node(&node_with_id).await.unwrap();
            db_clone.add_monitoring_result(&result).unwrap();

            result
        });
        handles.push(handle);
    }

    // Wait for all monitoring to complete
    let results: Vec<net_monitor::models::MonitoringResult> = futures::future::join_all(handles)
        .await
        .into_iter()
        .map(|r| r.unwrap())
        .collect();

    assert_eq!(results.len(), 3);

    // Verify all results are successful
    for result in results {
        assert_eq!(result.status, NodeStatus::Online);
        assert!(result.response_time.is_some());
    }
}

#[tokio::test]
#[cfg_attr(not(feature = "network-tests"), ignore)]
async fn test_http_monitoring_with_different_status_codes() {
    let test_db = TestDatabase::new();

    // Test different expected status codes
    let status_codes = vec![200, 201, 204, 301, 404, 500];

    for code in status_codes {
        let node = NodeBuilder::new()
            .name(format!("HTTP {} Test", code))
            .http(format!("https://httpbin.org/status/{}", code), code)
            .build();

        let node_id = test_db.db.add_node(&node).unwrap();
        let mut node_with_id = node;
        node_with_id.id = Some(node_id);

        let result = check_node(&node_with_id).await.unwrap();

        // When expected status matches actual status, should be Online
        assert_eq!(result.status, NodeStatus::Online);
        assert!(result.response_time.is_some());
    }
}

#[tokio::test]
async fn test_monitoring_invalid_url() {
    let test_db = TestDatabase::new();

    let node = NodeBuilder::new()
        .name("Invalid URL Test")
        .http("https://invalid-domain-that-does-not-exist-12345.com", 200)
        .build();

    let node_id = test_db.db.add_node(&node).unwrap();
    let mut node_with_id = node;
    node_with_id.id = Some(node_id);

    let result = check_node(&node_with_id).await.unwrap();

    assert_eq!(result.node_id, node_id);
    assert_eq!(result.status, NodeStatus::Offline);
    assert!(result.response_time.is_some());
    assert!(result.details.is_some());
}
