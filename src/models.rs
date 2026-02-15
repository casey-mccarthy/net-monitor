use crate::connection::ConnectionType;
use crate::credentials::CredentialId;
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
    Tcp {
        host: String,
        port: u16,
        timeout: u64,
    },
}

impl MonitorDetail {
    /// Get the connection target for this monitor type
    pub fn get_connection_target(&self) -> String {
        match self {
            MonitorDetail::Http { url, .. } => url.clone(),
            MonitorDetail::Ping { host, .. } => host.clone(),
            MonitorDetail::Tcp { host, port, .. } => format!("{}:{}", host, port),
        }
    }

    /// Get the appropriate connection type for this monitor
    pub fn get_connection_type(&self) -> ConnectionType {
        match self {
            MonitorDetail::Http { .. } => ConnectionType::Http,
            MonitorDetail::Ping { .. } => ConnectionType::Ping,
            MonitorDetail::Tcp { .. } => ConnectionType::Tcp,
        }
    }
}

impl fmt::Display for MonitorDetail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MonitorDetail::Http { .. } => write!(f, "HTTP"),
            MonitorDetail::Ping { .. } => write!(f, "Ping"),
            MonitorDetail::Tcp { .. } => write!(f, "TCP"),
        }
    }
}

/// Represents the current status of a node
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeStatus {
    /// Node is online and responding
    Online,
    /// Node is offline or not responding (confirmed after max_check_attempts)
    Offline,
    /// Node is failing checks but not yet confirmed down (soft state)
    Degraded,
}

impl fmt::Display for NodeStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodeStatus::Online => write!(f, "Online"),
            NodeStatus::Offline => write!(f, "Offline"),
            NodeStatus::Degraded => write!(f, "Degraded"),
        }
    }
}

/// Default number of consecutive failures before confirming offline
pub const DEFAULT_MAX_CHECK_ATTEMPTS: u32 = 3;

/// Default retry interval in seconds when in degraded state
pub const DEFAULT_RETRY_INTERVAL: u64 = 15;

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
    /// Optional credential reference for connections
    pub credential_id: Option<CredentialId>,
    /// Number of consecutive failures (resets on success)
    #[serde(default)]
    pub consecutive_failures: u32,
    /// How many consecutive failures before confirming offline (soft -> hard)
    #[serde(default = "default_max_check_attempts")]
    pub max_check_attempts: u32,
    /// Retry interval in seconds when in degraded state (shorter than monitoring_interval)
    #[serde(default = "default_retry_interval")]
    pub retry_interval: u64,
}

fn default_max_check_attempts() -> u32 {
    DEFAULT_MAX_CHECK_ATTEMPTS
}

fn default_retry_interval() -> u64 {
    DEFAULT_RETRY_INTERVAL
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
    /// Optional credential reference for connections
    pub credential_id: Option<CredentialId>,
    /// How many consecutive failures before confirming offline
    #[serde(default = "default_max_check_attempts")]
    pub max_check_attempts: u32,
    /// Retry interval in seconds when in degraded state
    #[serde(default = "default_retry_interval")]
    pub retry_interval: u64,
}

/// Represents a status change event for a node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusChange {
    /// Unique identifier for the status change
    pub id: Option<i64>,
    /// ID of the node that changed status
    pub node_id: i64,
    /// Previous status before the change
    pub from_status: NodeStatus,
    /// New status after the change
    pub to_status: NodeStatus,
    /// When the status change occurred
    pub changed_at: DateTime<Utc>,
    /// Duration in milliseconds spent in the previous status (None for first status)
    pub duration_ms: Option<i64>,
}

impl StatusChange {
    /// Calculate duration in milliseconds between two timestamps
    pub fn calculate_duration(from: DateTime<Utc>, to: DateTime<Utc>) -> i64 {
        (to - from).num_milliseconds()
    }

    /// Check if this is a transition to/from an error state
    #[allow(dead_code)] // Future feature: status change analysis
    pub fn is_degradation(&self) -> bool {
        matches!(
            (self.from_status, self.to_status),
            (NodeStatus::Online, NodeStatus::Offline)
                | (NodeStatus::Online, NodeStatus::Degraded)
                | (NodeStatus::Degraded, NodeStatus::Offline)
        )
    }

    /// Check if this is a recovery to a healthy state
    #[allow(dead_code)] // Future feature: status change analysis
    pub fn is_recovery(&self) -> bool {
        matches!(
            (self.from_status, self.to_status),
            (NodeStatus::Offline, NodeStatus::Online) | (NodeStatus::Degraded, NodeStatus::Online)
        )
    }

    /// Get a human-readable description of the status change
    #[allow(dead_code)] // Future feature: status change descriptions
    pub fn description(&self) -> String {
        format!("{} → {}", self.from_status, self.to_status)
    }
}
