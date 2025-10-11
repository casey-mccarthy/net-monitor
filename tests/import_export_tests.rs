mod common;

use common::{fixtures, TestDatabase};
use net_monitor::models::{NodeImport, NodeStatus};

#[test]
fn test_node_import_export_workflow() {
    let test_db = TestDatabase::new();

    // Create a node
    let node = fixtures::http_node();
    let node_id = test_db.db.add_node(&node).unwrap();

    // Export the node (simulate by creating NodeImport from Node)
    let node_import = NodeImport {
        name: node.name.clone(),
        detail: node.detail.clone(),
        monitoring_interval: node.monitoring_interval,
        credential_id: None,
    };

    // Serialize to JSON
    let json = serde_json::to_string(&node_import).unwrap();

    // Deserialize from JSON
    let imported_node: NodeImport = serde_json::from_str(&json).unwrap();

    // Verify the import/export worked correctly
    assert_eq!(imported_node.name, node.name);
    assert_eq!(imported_node.monitoring_interval, node.monitoring_interval);

    // Create a new node from the import
    let new_node = net_monitor::models::Node {
        id: None,
        name: imported_node.name,
        detail: imported_node.detail,
        status: NodeStatus::Offline,
        last_check: None,
        response_time: None,
        monitoring_interval: imported_node.monitoring_interval,
        credential_id: imported_node.credential_id,
    };

    let new_node_id = test_db.db.add_node(&new_node).unwrap();
    assert!(new_node_id != node_id); // Should be a different ID

    let all_nodes = test_db.db.get_all_nodes().unwrap();
    assert_eq!(all_nodes.len(), 2);
}

#[test]
fn test_export_multiple_nodes_to_json() {
    let test_db = TestDatabase::new();

    // Add multiple nodes
    let http_node = fixtures::http_node();
    let ping_node = fixtures::ping_node();

    test_db.db.add_node(&http_node).unwrap();
    test_db.db.add_node(&ping_node).unwrap();

    // Get all nodes
    let nodes = test_db.db.get_all_nodes().unwrap();

    // Convert to NodeImport and serialize
    let node_imports: Vec<NodeImport> = nodes
        .iter()
        .map(|n| NodeImport {
            name: n.name.clone(),
            detail: n.detail.clone(),
            monitoring_interval: n.monitoring_interval,
            credential_id: n.credential_id.clone(),
        })
        .collect();

    let json = serde_json::to_string_pretty(&node_imports).unwrap();

    // Deserialize back
    let imported: Vec<NodeImport> = serde_json::from_str(&json).unwrap();

    assert_eq!(imported.len(), 2);
    assert_eq!(imported[0].name, http_node.name);
    assert_eq!(imported[1].name, ping_node.name);
}

#[test]
fn test_import_nodes_from_json() {
    let test_db = TestDatabase::new();

    // Create JSON representation of nodes
    let json = r#"[
        {
            "name": "Imported HTTP Node",
            "detail": {
                "type": "Http",
                "url": "https://example.com",
                "expected_status": 200
            },
            "monitoring_interval": 60,
            "credential_id": null
        },
        {
            "name": "Imported Ping Node",
            "detail": {
                "type": "Ping",
                "host": "192.168.1.1",
                "count": 4,
                "timeout": 5
            },
            "monitoring_interval": 30,
            "credential_id": null
        }
    ]"#;

    // Deserialize
    let imported_nodes: Vec<NodeImport> = serde_json::from_str(json).unwrap();
    assert_eq!(imported_nodes.len(), 2);

    // Import into database
    for import in imported_nodes {
        let node = net_monitor::models::Node {
            id: None,
            name: import.name,
            detail: import.detail,
            status: NodeStatus::Offline,
            last_check: None,
            response_time: None,
            monitoring_interval: import.monitoring_interval,
            credential_id: import.credential_id,
        };
        test_db.db.add_node(&node).unwrap();
    }

    // Verify nodes were imported
    let all_nodes = test_db.db.get_all_nodes().unwrap();
    assert_eq!(all_nodes.len(), 2);

    assert_eq!(all_nodes[0].name, "Imported HTTP Node");
    assert_eq!(all_nodes[1].name, "Imported Ping Node");
}

#[test]
fn test_node_import_preserves_detail_types() {
    let _test_db = TestDatabase::new();

    // Create nodes of each type
    let http_node = fixtures::http_node();
    let ping_node = fixtures::ping_node();

    // Export and re-import HTTP node
    let http_import = NodeImport {
        name: http_node.name.clone(),
        detail: http_node.detail.clone(),
        monitoring_interval: http_node.monitoring_interval,
        credential_id: None,
    };

    let http_json = serde_json::to_string(&http_import).unwrap();
    let http_reimport: NodeImport = serde_json::from_str(&http_json).unwrap();

    assert_eq!(http_reimport.detail, http_node.detail);

    // Export and re-import Ping node
    let ping_import = NodeImport {
        name: ping_node.name.clone(),
        detail: ping_node.detail.clone(),
        monitoring_interval: ping_node.monitoring_interval,
        credential_id: None,
    };

    let ping_json = serde_json::to_string(&ping_import).unwrap();
    let ping_reimport: NodeImport = serde_json::from_str(&ping_json).unwrap();

    assert_eq!(ping_reimport.detail, ping_node.detail);
}
