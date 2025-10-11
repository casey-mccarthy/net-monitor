// Unit tests for models module
// Moved from src/models.rs to follow Rust best practices

use chrono::Utc;
use net_monitor::models::{MonitorDetail, MonitoringResult, Node, NodeImport, NodeStatus};

#[test]
fn test_monitor_detail_display() {
    let http_detail = MonitorDetail::Http {
        url: "https://example.com".to_string(),
        expected_status: 200,
    };
    assert_eq!(http_detail.to_string(), "HTTP");

    let ping_detail = MonitorDetail::Ping {
        host: "192.168.1.1".to_string(),
        count: 4,
        timeout: 5,
    };
    assert_eq!(ping_detail.to_string(), "Ping");
}

#[test]
fn test_monitor_detail_serialization() {
    let http_detail = MonitorDetail::Http {
        url: "https://example.com".to_string(),
        expected_status: 200,
    };
    let serialized = serde_json::to_string(&http_detail).unwrap();
    let deserialized: MonitorDetail = serde_json::from_str(&serialized).unwrap();
    assert_eq!(http_detail, deserialized);

    let ping_detail = MonitorDetail::Ping {
        host: "192.168.1.1".to_string(),
        count: 4,
        timeout: 5,
    };
    let serialized = serde_json::to_string(&ping_detail).unwrap();
    let deserialized: MonitorDetail = serde_json::from_str(&serialized).unwrap();
    assert_eq!(ping_detail, deserialized);
}

#[test]
fn test_node_status_display() {
    assert_eq!(NodeStatus::Online.to_string(), "Online");
    assert_eq!(NodeStatus::Offline.to_string(), "Offline");
}

#[test]
fn test_node_status_serialization() {
    let status = NodeStatus::Online;
    let serialized = serde_json::to_string(&status).unwrap();
    let deserialized: NodeStatus = serde_json::from_str(&serialized).unwrap();
    assert_eq!(status, deserialized);
}

#[test]
fn test_node_creation() {
    let node = Node {
        id: Some(1),
        name: "Test Node".to_string(),
        detail: MonitorDetail::Http {
            url: "https://example.com".to_string(),
            expected_status: 200,
        },
        status: NodeStatus::Online,
        last_check: Some(Utc::now()),
        response_time: Some(150),
        monitoring_interval: 60,
        credential_id: None,
    };

    assert_eq!(node.id, Some(1));
    assert_eq!(node.name, "Test Node");
    assert_eq!(node.status, NodeStatus::Online);
    assert_eq!(node.response_time, Some(150));
    assert_eq!(node.monitoring_interval, 60);
    assert_eq!(node.credential_id, None);
}

#[test]
fn test_node_serialization() {
    let node = Node {
        id: Some(1),
        name: "Test Node".to_string(),
        detail: MonitorDetail::Ping {
            host: "192.168.1.1".to_string(),
            count: 4,
            timeout: 5,
        },
        status: NodeStatus::Online,
        last_check: Some(Utc::now()),
        response_time: Some(150),
        monitoring_interval: 60,
        credential_id: None,
    };

    let serialized = serde_json::to_string(&node).unwrap();
    let deserialized: Node = serde_json::from_str(&serialized).unwrap();

    // Note: We can't directly compare due to timestamp precision differences
    assert_eq!(deserialized.id, node.id);
    assert_eq!(deserialized.name, node.name);
    assert_eq!(deserialized.status, node.status);
    assert_eq!(deserialized.response_time, node.response_time);
    assert_eq!(deserialized.monitoring_interval, node.monitoring_interval);
}

#[test]
fn test_monitoring_result_creation() {
    let result = MonitoringResult {
        id: Some(1),
        node_id: 1,
        timestamp: Utc::now(),
        status: NodeStatus::Online,
        response_time: Some(150),
        details: Some("Success".to_string()),
    };

    assert_eq!(result.id, Some(1));
    assert_eq!(result.node_id, 1);
    assert_eq!(result.status, NodeStatus::Online);
    assert_eq!(result.response_time, Some(150));
    assert_eq!(result.details, Some("Success".to_string()));
}

#[test]
fn test_node_import_creation() {
    let node_import = NodeImport {
        name: "Test Node".to_string(),
        detail: MonitorDetail::Http {
            url: "https://example.com".to_string(),
            expected_status: 200,
        },
        monitoring_interval: 60,
        credential_id: None,
    };

    assert_eq!(node_import.name, "Test Node");
    assert_eq!(node_import.monitoring_interval, 60);
    assert_eq!(node_import.credential_id, None);
}

#[test]
fn test_node_import_serialization() {
    let node_import = NodeImport {
        name: "Test Node".to_string(),
        detail: MonitorDetail::Http {
            url: "https://example.com".to_string(),
            expected_status: 200,
        },
        monitoring_interval: 60,
        credential_id: None,
    };

    let serialized = serde_json::to_string(&node_import).unwrap();
    let deserialized: NodeImport = serde_json::from_str(&serialized).unwrap();
    assert_eq!(node_import.name, deserialized.name);
    assert_eq!(
        node_import.monitoring_interval,
        deserialized.monitoring_interval
    );
}

#[test]
fn test_monitor_detail_partial_eq() {
    let http1 = MonitorDetail::Http {
        url: "https://example.com".to_string(),
        expected_status: 200,
    };
    let http2 = MonitorDetail::Http {
        url: "https://example.com".to_string(),
        expected_status: 200,
    };
    let http3 = MonitorDetail::Http {
        url: "https://example.org".to_string(),
        expected_status: 200,
    };

    assert_eq!(http1, http2);
    assert_ne!(http1, http3);
}

#[test]
fn test_node_status_partial_eq() {
    assert_eq!(NodeStatus::Online, NodeStatus::Online);
    assert_ne!(NodeStatus::Online, NodeStatus::Offline);
    assert_eq!(NodeStatus::Offline, NodeStatus::Offline);
}
