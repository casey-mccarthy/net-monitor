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