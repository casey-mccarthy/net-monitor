use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Represents the details for each monitoring type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum MonitorDetail {
    Http {
        url: String,
        expected_status: u16,
    },
    Ping {
        host: String,
        count: u32,
        timeout: u64,
    },
    Snmp {
        target: String,
        community: String,
        oid: String,
    },
}

impl fmt::Display for MonitorDetail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MonitorDetail::Http { .. } => write!(f, "HTTP"),
            MonitorDetail::Ping { .. } => write!(f, "Ping"),
            MonitorDetail::Snmp { .. } => write!(f, "SNMP"),
        }
    }
}

/// Represents the current status of a node
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeStatus {
    /// Node is online and responding
    Online,
    /// Node is offline or not responding
    Offline,
    /// Status is unknown (e.g., monitoring disabled)
    Unknown,
}

impl fmt::Display for NodeStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodeStatus::Online => write!(f, "Online"),
            NodeStatus::Offline => write!(f, "Offline"),
            NodeStatus::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Represents a network node to be monitored
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Node {
    /// Unique identifier for the node
    pub id: Option<i64>,
    /// Human-readable name for the node
    pub name: String,
    /// Specific details for the monitor type
    pub detail: MonitorDetail,
    /// Current status of the node
    pub status: NodeStatus,
    /// Last time the node was checked
    pub last_check: Option<DateTime<Utc>>,
    /// Response time in milliseconds (if available)
    pub response_time: Option<u64>,
    /// Monitoring interval in seconds
    pub monitoring_interval: u64,
}

/// Represents a historical monitoring result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringResult {
    /// Unique identifier for the result
    pub id: Option<i64>,
    /// ID of the node being monitored
    pub node_id: i64,
    /// Timestamp of the check
    pub timestamp: DateTime<Utc>,
    /// Status at the time of check
    pub status: NodeStatus,
    /// Response time in milliseconds
    pub response_time: Option<u64>,
    /// Additional details about the check
    pub details: Option<String>,
}

/// Represents a node for import/export operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeImport {
    /// Human-readable name for the node
    pub name: String,
    /// Specific details for the monitor type
    pub detail: MonitorDetail,
    /// Monitoring interval in seconds
    pub monitoring_interval: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

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

        let snmp_detail = MonitorDetail::Snmp {
            target: "192.168.1.1".to_string(),
            community: "public".to_string(),
            oid: "1.3.6.1.2.1.1.1.0".to_string(),
        };
        assert_eq!(snmp_detail.to_string(), "SNMP");
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
        assert_eq!(NodeStatus::Unknown.to_string(), "Unknown");
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
        };

        assert_eq!(node.id, Some(1));
        assert_eq!(node.name, "Test Node");
        assert_eq!(node.status, NodeStatus::Online);
        assert_eq!(node.response_time, Some(150));
        assert_eq!(node.monitoring_interval, 60);
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
            detail: MonitorDetail::Snmp {
                target: "192.168.1.1".to_string(),
                community: "public".to_string(),
                oid: "1.3.6.1.2.1.1.1.0".to_string(),
            },
            monitoring_interval: 60,
        };

        assert_eq!(node_import.name, "Test Node");
        assert_eq!(node_import.monitoring_interval, 60);
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
        };

        let serialized = serde_json::to_string(&node_import).unwrap();
        let deserialized: NodeImport = serde_json::from_str(&serialized).unwrap();
        assert_eq!(node_import.name, deserialized.name);
        assert_eq!(node_import.monitoring_interval, deserialized.monitoring_interval);
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
        assert_ne!(NodeStatus::Online, NodeStatus::Unknown);
    }
} 