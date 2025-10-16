// Unit tests for models module
// Moved from src/models.rs to follow Rust best practices

use chrono::{Duration, Utc};
use net_monitor::connection::ConnectionType;
use net_monitor::models::{
    MonitorDetail, MonitoringResult, Node, NodeImport, NodeStatus, StatusChange,
};

// ========== MonitorDetail Tests ==========

#[test]
fn test_monitor_detail_display_http() {
    let http_detail = MonitorDetail::Http {
        url: "https://example.com".to_string(),
        expected_status: 200,
    };
    assert_eq!(http_detail.to_string(), "HTTP");
}

#[test]
fn test_monitor_detail_display_ping() {
    let ping_detail = MonitorDetail::Ping {
        host: "192.168.1.1".to_string(),
        count: 4,
        timeout: 5,
    };
    assert_eq!(ping_detail.to_string(), "Ping");
}

#[test]
fn test_monitor_detail_display_tcp() {
    let tcp_detail = MonitorDetail::Tcp {
        host: "192.168.1.1".to_string(),
        port: 8080,
        timeout: 5,
    };
    assert_eq!(tcp_detail.to_string(), "TCP");
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

// ========== MonitorDetail Connection Tests ==========

#[test]
fn test_monitor_detail_get_connection_target_http() {
    let detail = MonitorDetail::Http {
        url: "https://example.com".to_string(),
        expected_status: 200,
    };
    assert_eq!(detail.get_connection_target(), "https://example.com");
}

#[test]
fn test_monitor_detail_get_connection_target_ping() {
    let detail = MonitorDetail::Ping {
        host: "192.168.1.1".to_string(),
        count: 4,
        timeout: 5,
    };
    assert_eq!(detail.get_connection_target(), "192.168.1.1");
}

#[test]
fn test_monitor_detail_get_connection_target_tcp() {
    let detail = MonitorDetail::Tcp {
        host: "192.168.1.1".to_string(),
        port: 8080,
        timeout: 5,
    };
    assert_eq!(detail.get_connection_target(), "192.168.1.1:8080");
}

#[test]
fn test_monitor_detail_get_connection_type_http() {
    let detail = MonitorDetail::Http {
        url: "https://example.com".to_string(),
        expected_status: 200,
    };
    assert_eq!(detail.get_connection_type(), ConnectionType::Http);
}

#[test]
fn test_monitor_detail_get_connection_type_ping() {
    let detail = MonitorDetail::Ping {
        host: "192.168.1.1".to_string(),
        count: 4,
        timeout: 5,
    };
    assert_eq!(detail.get_connection_type(), ConnectionType::Ping);
}

#[test]
fn test_monitor_detail_get_connection_type_tcp() {
    let detail = MonitorDetail::Tcp {
        host: "192.168.1.1".to_string(),
        port: 8080,
        timeout: 5,
    };
    assert_eq!(detail.get_connection_type(), ConnectionType::Tcp);
}

// ========== MonitorDetail TCP Tests ==========

#[test]
fn test_monitor_detail_tcp_creation() {
    let tcp_detail = MonitorDetail::Tcp {
        host: "example.com".to_string(),
        port: 22,
        timeout: 10,
    };
    assert_eq!(tcp_detail.to_string(), "TCP");
}

#[test]
fn test_monitor_detail_tcp_serialization() {
    let tcp_detail = MonitorDetail::Tcp {
        host: "localhost".to_string(),
        port: 3000,
        timeout: 5,
    };
    let serialized = serde_json::to_string(&tcp_detail).unwrap();
    let deserialized: MonitorDetail = serde_json::from_str(&serialized).unwrap();
    assert_eq!(tcp_detail, deserialized);
}

#[test]
fn test_monitor_detail_tcp_debug() {
    let tcp_detail = MonitorDetail::Tcp {
        host: "192.168.1.1".to_string(),
        port: 443,
        timeout: 5,
    };
    let debug_str = format!("{:?}", tcp_detail);
    assert!(debug_str.contains("Tcp"));
    assert!(debug_str.contains("192.168.1.1"));
    assert!(debug_str.contains("443"));
}

#[test]
fn test_monitor_detail_clone() {
    let original = MonitorDetail::Http {
        url: "https://test.com".to_string(),
        expected_status: 404,
    };
    let cloned = original.clone();
    assert_eq!(original, cloned);
}

#[test]
fn test_monitor_detail_debug() {
    let detail = MonitorDetail::Ping {
        host: "example.com".to_string(),
        count: 3,
        timeout: 10,
    };
    let debug_str = format!("{:?}", detail);
    assert!(debug_str.contains("Ping"));
    assert!(debug_str.contains("example.com"));
}

// ========== StatusChange Tests ==========

#[test]
fn test_status_change_creation() {
    let now = Utc::now();
    let change = StatusChange {
        id: Some(1),
        node_id: 42,
        from_status: NodeStatus::Online,
        to_status: NodeStatus::Offline,
        changed_at: now,
        duration_ms: Some(60000),
    };

    assert_eq!(change.id, Some(1));
    assert_eq!(change.node_id, 42);
    assert_eq!(change.from_status, NodeStatus::Online);
    assert_eq!(change.to_status, NodeStatus::Offline);
    assert_eq!(change.duration_ms, Some(60000));
}

#[test]
fn test_status_change_calculate_duration() {
    let start = Utc::now();
    let end = start + Duration::seconds(120);
    let duration_ms = StatusChange::calculate_duration(start, end);
    assert_eq!(duration_ms, 120000);
}

#[test]
fn test_status_change_calculate_duration_negative() {
    let start = Utc::now();
    let end = start - Duration::seconds(30);
    let duration_ms = StatusChange::calculate_duration(start, end);
    assert_eq!(duration_ms, -30000);
}

#[test]
fn test_status_change_calculate_duration_zero() {
    let time = Utc::now();
    let duration_ms = StatusChange::calculate_duration(time, time);
    assert_eq!(duration_ms, 0);
}

#[test]
fn test_status_change_is_degradation_online_to_offline() {
    let change = StatusChange {
        id: None,
        node_id: 1,
        from_status: NodeStatus::Online,
        to_status: NodeStatus::Offline,
        changed_at: Utc::now(),
        duration_ms: None,
    };
    assert!(change.is_degradation());
}

#[test]
fn test_status_change_is_degradation_offline_to_online() {
    let change = StatusChange {
        id: None,
        node_id: 1,
        from_status: NodeStatus::Offline,
        to_status: NodeStatus::Online,
        changed_at: Utc::now(),
        duration_ms: None,
    };
    assert!(!change.is_degradation());
}

#[test]
fn test_status_change_is_recovery_offline_to_online() {
    let change = StatusChange {
        id: None,
        node_id: 1,
        from_status: NodeStatus::Offline,
        to_status: NodeStatus::Online,
        changed_at: Utc::now(),
        duration_ms: None,
    };
    assert!(change.is_recovery());
}

#[test]
fn test_status_change_is_recovery_online_to_offline() {
    let change = StatusChange {
        id: None,
        node_id: 1,
        from_status: NodeStatus::Online,
        to_status: NodeStatus::Offline,
        changed_at: Utc::now(),
        duration_ms: None,
    };
    assert!(!change.is_recovery());
}

#[test]
fn test_status_change_description() {
    let change = StatusChange {
        id: None,
        node_id: 1,
        from_status: NodeStatus::Online,
        to_status: NodeStatus::Offline,
        changed_at: Utc::now(),
        duration_ms: None,
    };
    assert_eq!(change.description(), "Online → Offline");
}

#[test]
fn test_status_change_description_recovery() {
    let change = StatusChange {
        id: None,
        node_id: 1,
        from_status: NodeStatus::Offline,
        to_status: NodeStatus::Online,
        changed_at: Utc::now(),
        duration_ms: None,
    };
    assert_eq!(change.description(), "Offline → Online");
}

#[test]
fn test_status_change_clone() {
    let original = StatusChange {
        id: Some(5),
        node_id: 10,
        from_status: NodeStatus::Online,
        to_status: NodeStatus::Offline,
        changed_at: Utc::now(),
        duration_ms: Some(5000),
    };
    let cloned = original.clone();
    assert_eq!(cloned.id, original.id);
    assert_eq!(cloned.node_id, original.node_id);
    assert_eq!(cloned.from_status, original.from_status);
    assert_eq!(cloned.to_status, original.to_status);
}

#[test]
fn test_status_change_debug() {
    let change = StatusChange {
        id: None,
        node_id: 1,
        from_status: NodeStatus::Online,
        to_status: NodeStatus::Offline,
        changed_at: Utc::now(),
        duration_ms: Some(30000),
    };
    let debug_str = format!("{:?}", change);
    assert!(debug_str.contains("StatusChange"));
    assert!(debug_str.contains("node_id"));
}

#[test]
fn test_status_change_serialization() {
    let change = StatusChange {
        id: Some(1),
        node_id: 42,
        from_status: NodeStatus::Online,
        to_status: NodeStatus::Offline,
        changed_at: Utc::now(),
        duration_ms: Some(60000),
    };
    let json = serde_json::to_string(&change).unwrap();
    let deserialized: StatusChange = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.id, change.id);
    assert_eq!(deserialized.node_id, change.node_id);
    assert_eq!(deserialized.from_status, change.from_status);
    assert_eq!(deserialized.to_status, change.to_status);
}

// ========== Node Edge Cases ==========

#[test]
fn test_node_with_tcp_detail() {
    let node = Node {
        id: Some(10),
        name: "TCP Node".to_string(),
        detail: MonitorDetail::Tcp {
            host: "192.168.1.100".to_string(),
            port: 22,
            timeout: 10,
        },
        status: NodeStatus::Online,
        last_check: None,
        response_time: None,
        monitoring_interval: 30,
        credential_id: Some("cred_123".to_string()),
    };

    assert_eq!(node.name, "TCP Node");
    assert_eq!(node.monitoring_interval, 30);
    assert_eq!(node.credential_id, Some("cred_123".to_string()));
}

#[test]
fn test_node_without_optional_fields() {
    let node = Node {
        id: None,
        name: "Minimal Node".to_string(),
        detail: MonitorDetail::Ping {
            host: "8.8.8.8".to_string(),
            count: 4,
            timeout: 5,
        },
        status: NodeStatus::Offline,
        last_check: None,
        response_time: None,
        monitoring_interval: 60,
        credential_id: None,
    };

    assert!(node.id.is_none());
    assert!(node.last_check.is_none());
    assert!(node.response_time.is_none());
    assert!(node.credential_id.is_none());
}

#[test]
fn test_node_clone() {
    let original = Node {
        id: Some(1),
        name: "Clone Test".to_string(),
        detail: MonitorDetail::Http {
            url: "https://example.com".to_string(),
            expected_status: 200,
        },
        status: NodeStatus::Online,
        last_check: Some(Utc::now()),
        response_time: Some(100),
        monitoring_interval: 60,
        credential_id: None,
    };
    let cloned = original.clone();
    assert_eq!(cloned.id, original.id);
    assert_eq!(cloned.name, original.name);
    assert_eq!(cloned.status, original.status);
}

#[test]
fn test_node_debug() {
    let node = Node {
        id: Some(1),
        name: "Debug Node".to_string(),
        detail: MonitorDetail::Http {
            url: "https://example.com".to_string(),
            expected_status: 200,
        },
        status: NodeStatus::Online,
        last_check: None,
        response_time: None,
        monitoring_interval: 60,
        credential_id: None,
    };
    let debug_str = format!("{:?}", node);
    assert!(debug_str.contains("Node"));
    assert!(debug_str.contains("Debug Node"));
}

#[test]
fn test_node_partial_eq() {
    let node1 = Node {
        id: Some(1),
        name: "Test".to_string(),
        detail: MonitorDetail::Http {
            url: "https://example.com".to_string(),
            expected_status: 200,
        },
        status: NodeStatus::Online,
        last_check: None,
        response_time: None,
        monitoring_interval: 60,
        credential_id: None,
    };

    let node2 = Node {
        id: Some(1),
        name: "Test".to_string(),
        detail: MonitorDetail::Http {
            url: "https://example.com".to_string(),
            expected_status: 200,
        },
        status: NodeStatus::Online,
        last_check: None,
        response_time: None,
        monitoring_interval: 60,
        credential_id: None,
    };

    assert_eq!(node1, node2);
}

// ========== NodeImport Edge Cases ==========

#[test]
fn test_node_import_with_tcp() {
    let import = NodeImport {
        name: "TCP Import".to_string(),
        detail: MonitorDetail::Tcp {
            host: "localhost".to_string(),
            port: 3000,
            timeout: 5,
        },
        monitoring_interval: 30,
        credential_id: Some("cred_abc".to_string()),
    };

    assert_eq!(import.name, "TCP Import");
    assert_eq!(import.monitoring_interval, 30);
}

#[test]
fn test_node_import_clone() {
    let original = NodeImport {
        name: "Clone Import".to_string(),
        detail: MonitorDetail::Ping {
            host: "8.8.8.8".to_string(),
            count: 4,
            timeout: 5,
        },
        monitoring_interval: 60,
        credential_id: None,
    };
    let cloned = original.clone();
    assert_eq!(cloned.name, original.name);
    assert_eq!(cloned.monitoring_interval, original.monitoring_interval);
}

#[test]
fn test_node_import_debug() {
    let import = NodeImport {
        name: "Debug Import".to_string(),
        detail: MonitorDetail::Http {
            url: "https://test.com".to_string(),
            expected_status: 200,
        },
        monitoring_interval: 120,
        credential_id: None,
    };
    let debug_str = format!("{:?}", import);
    assert!(debug_str.contains("NodeImport"));
    assert!(debug_str.contains("Debug Import"));
}

// ========== MonitoringResult Edge Cases ==========

#[test]
fn test_monitoring_result_without_optional_fields() {
    let result = MonitoringResult {
        id: None,
        node_id: 5,
        timestamp: Utc::now(),
        status: NodeStatus::Offline,
        response_time: None,
        details: None,
    };

    assert!(result.id.is_none());
    assert!(result.response_time.is_none());
    assert!(result.details.is_none());
}

#[test]
fn test_monitoring_result_with_details() {
    let result = MonitoringResult {
        id: Some(100),
        node_id: 1,
        timestamp: Utc::now(),
        status: NodeStatus::Online,
        response_time: Some(250),
        details: Some("Connection successful".to_string()),
    };

    assert_eq!(result.details, Some("Connection successful".to_string()));
    assert_eq!(result.response_time, Some(250));
}

#[test]
fn test_monitoring_result_clone() {
    let original = MonitoringResult {
        id: Some(1),
        node_id: 10,
        timestamp: Utc::now(),
        status: NodeStatus::Online,
        response_time: Some(100),
        details: Some("Test".to_string()),
    };
    let cloned = original.clone();
    assert_eq!(cloned.id, original.id);
    assert_eq!(cloned.node_id, original.node_id);
    assert_eq!(cloned.status, original.status);
}

#[test]
fn test_monitoring_result_debug() {
    let result = MonitoringResult {
        id: Some(1),
        node_id: 1,
        timestamp: Utc::now(),
        status: NodeStatus::Online,
        response_time: Some(150),
        details: Some("Success".to_string()),
    };
    let debug_str = format!("{:?}", result);
    assert!(debug_str.contains("MonitoringResult"));
}

#[test]
fn test_monitoring_result_serialization() {
    let result = MonitoringResult {
        id: Some(1),
        node_id: 1,
        timestamp: Utc::now(),
        status: NodeStatus::Online,
        response_time: Some(150),
        details: Some("Success".to_string()),
    };
    let json = serde_json::to_string(&result).unwrap();
    let deserialized: MonitoringResult = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.id, result.id);
    assert_eq!(deserialized.node_id, result.node_id);
}
