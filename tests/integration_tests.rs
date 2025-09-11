use net_monitor::database::Database;
use net_monitor::models::{MonitorDetail, MonitoringResult, Node, NodeStatus};
use net_monitor::monitor::check_node;
use std::fs;
use tempfile::NamedTempFile;

/// Creates a temporary database for integration testing
fn create_test_database() -> (Database, NamedTempFile) {
    let temp_file = NamedTempFile::new().unwrap();
    let db = Database::new(temp_file.path()).unwrap();
    (db, temp_file)
}

/// Creates a test HTTP node for integration testing
fn create_test_http_node() -> Node {
    Node {
        id: None,
        name: "Integration Test HTTP Node".to_string(),
        detail: MonitorDetail::Http {
            url: "https://httpbin.org/status/200".to_string(),
            expected_status: 200,
        },
        status: NodeStatus::Unknown,
        last_check: None,
        response_time: None,
        monitoring_interval: 60,
        credential_id: None,
    }
}

#[tokio::test]
#[cfg_attr(not(feature = "network-tests"), ignore)]
async fn test_full_monitoring_workflow() {
    let (db, temp_file) = create_test_database();

    // 1. Add a node to the database
    let node = create_test_http_node();
    let node_id = db.add_node(&node).unwrap();
    assert!(node_id > 0);

    // 2. Retrieve the node from database
    let nodes = db.get_all_nodes().unwrap();
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
    let result_id = db.add_monitoring_result(&monitoring_result).unwrap();
    assert!(result_id > 0);

    // 5. Update the node with the new status
    retrieved_node.status = monitoring_result.status;
    retrieved_node.last_check = Some(monitoring_result.timestamp);
    retrieved_node.response_time = monitoring_result.response_time;
    db.update_node(&retrieved_node).unwrap();

    // 6. Verify the update
    let updated_nodes = db.get_all_nodes().unwrap();
    assert_eq!(updated_nodes.len(), 1);
    assert_eq!(updated_nodes[0].status, NodeStatus::Online);
    assert!(updated_nodes[0].last_check.is_some());
    assert!(updated_nodes[0].response_time.is_some());

    drop(db);
    fs::remove_file(temp_file.path()).unwrap();
}

#[tokio::test]
#[cfg_attr(not(feature = "network-tests"), ignore)]
async fn test_monitoring_failure_workflow() {
    let (db, temp_file) = create_test_database();

    // Create a node that will fail (expecting 200 but will get 404)
    let node = Node {
        id: None,
        name: "Integration Test HTTP Failure Node".to_string(),
        detail: MonitorDetail::Http {
            url: "https://httpbin.org/status/404".to_string(),
            expected_status: 200, // Expecting 200 but will get 404
        },
        status: NodeStatus::Unknown,
        last_check: None,
        response_time: None,
        monitoring_interval: 60,
        credential_id: None,
    };

    let node_id = db.add_node(&node).unwrap();
    let nodes = db.get_all_nodes().unwrap();
    let mut retrieved_node = nodes[0].clone();
    retrieved_node.id = Some(node_id);

    // Check the node (should fail)
    let monitoring_result = check_node(&retrieved_node).await.unwrap();
    assert_eq!(monitoring_result.node_id, node_id);
    assert_eq!(monitoring_result.status, NodeStatus::Offline);
    assert!(monitoring_result.response_time.is_some());
    assert!(monitoring_result.details.is_some());

    // Store the monitoring result
    let result_id = db.add_monitoring_result(&monitoring_result).unwrap();
    assert!(result_id > 0);

    // Update the node with the failure status
    retrieved_node.status = monitoring_result.status;
    retrieved_node.last_check = Some(monitoring_result.timestamp);
    retrieved_node.response_time = monitoring_result.response_time;
    db.update_node(&retrieved_node).unwrap();

    // Verify the update
    let updated_nodes = db.get_all_nodes().unwrap();
    assert_eq!(updated_nodes.len(), 1);
    assert_eq!(updated_nodes[0].status, NodeStatus::Offline);

    drop(db);
    fs::remove_file(temp_file.path()).unwrap();
}

#[test]
fn test_database_persistence() {
    let (db, temp_file) = create_test_database();

    // Add multiple nodes
    let http_node = create_test_http_node();
    let ping_node = Node {
        id: None,
        name: "Integration Test Ping Node".to_string(),
        detail: MonitorDetail::Ping {
            host: "127.0.0.1".to_string(),
            count: 1,
            timeout: 1,
        },
        status: NodeStatus::Unknown,
        last_check: None,
        response_time: None,
        monitoring_interval: 30,
        credential_id: None,
    };

    let http_id = db.add_node(&http_node).unwrap();
    let ping_id = db.add_node(&ping_node).unwrap();

    // Verify nodes are stored
    let nodes = db.get_all_nodes().unwrap();
    assert_eq!(nodes.len(), 2);

    // Drop the database connection
    drop(db);

    // Recreate the database connection to the same file
    let db2 = Database::new(temp_file.path()).unwrap();

    // Verify nodes are still there
    let nodes2 = db2.get_all_nodes().unwrap();
    assert_eq!(nodes2.len(), 2);

    // Verify the nodes have the correct data
    let http_node_retrieved = nodes2
        .iter()
        .find(|n| n.name == "Integration Test HTTP Node")
        .unwrap();
    let ping_node_retrieved = nodes2
        .iter()
        .find(|n| n.name == "Integration Test Ping Node")
        .unwrap();

    assert_eq!(http_node_retrieved.id, Some(http_id));
    assert_eq!(ping_node_retrieved.id, Some(ping_id));

    if let MonitorDetail::Http {
        url,
        expected_status,
    } = &http_node_retrieved.detail
    {
        assert_eq!(url, "https://httpbin.org/status/200");
        assert_eq!(*expected_status, 200);
    } else {
        panic!("Expected HTTP monitor detail");
    }

    if let MonitorDetail::Ping {
        host,
        count,
        timeout,
    } = &ping_node_retrieved.detail
    {
        assert_eq!(host, "127.0.0.1");
        assert_eq!(*count, 1);
        assert_eq!(*timeout, 1);
    } else {
        panic!("Expected Ping monitor detail");
    }

    drop(db2);
    fs::remove_file(temp_file.path()).unwrap();
}

#[tokio::test]
#[cfg_attr(not(feature = "network-tests"), ignore)]
async fn test_concurrent_monitoring() {
    let (db, temp_file) = create_test_database();

    // Create multiple nodes
    let mut nodes = Vec::new();
    for i in 0..3 {
        let node = Node {
            id: None,
            name: format!("Concurrent Test Node {}", i),
            detail: MonitorDetail::Http {
                url: "https://httpbin.org/status/200".to_string(),
                expected_status: 200,
            },
            status: NodeStatus::Unknown,
            last_check: None,
            response_time: None,
            monitoring_interval: 60,
            credential_id: None,
        };
        let node_id = db.add_node(&node).unwrap();
        nodes.push((node_id, node));
    }

    // Monitor all nodes concurrently
    let mut handles = Vec::new();
    for (node_id, node) in nodes {
        let db_clone = db.clone();
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
    let results: Vec<MonitoringResult> = futures::future::join_all(handles)
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

    drop(db);
    fs::remove_file(temp_file.path()).unwrap();
}

#[test]
fn test_node_import_export_workflow() {
    let (db, temp_file) = create_test_database();

    // Create a node
    let node = create_test_http_node();
    let node_id = db.add_node(&node).unwrap();

    // Export the node (simulate by creating NodeImport from Node)
    let node_import = net_monitor::models::NodeImport {
        name: node.name.clone(),
        detail: node.detail.clone(),
        monitoring_interval: node.monitoring_interval,
        credential_id: None,
    };

    // Serialize to JSON
    let json = serde_json::to_string(&node_import).unwrap();

    // Deserialize from JSON
    let imported_node: net_monitor::models::NodeImport = serde_json::from_str(&json).unwrap();

    // Verify the import/export worked correctly
    assert_eq!(imported_node.name, node.name);
    assert_eq!(imported_node.monitoring_interval, node.monitoring_interval);

    // Create a new node from the import
    let new_node = Node {
        id: None,
        name: imported_node.name,
        detail: imported_node.detail,
        status: NodeStatus::Unknown,
        last_check: None,
        response_time: None,
        monitoring_interval: imported_node.monitoring_interval,
        credential_id: imported_node.credential_id,
    };

    let new_node_id = db.add_node(&new_node).unwrap();
    assert!(new_node_id != node_id); // Should be a different ID

    let all_nodes = db.get_all_nodes().unwrap();
    assert_eq!(all_nodes.len(), 2);

    drop(db);
    fs::remove_file(temp_file.path()).unwrap();
}
