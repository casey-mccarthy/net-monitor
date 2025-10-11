use net_monitor::database::Database;
use net_monitor::models::{MonitorDetail, Node, NodeStatus};
use std::fs;
use std::path::PathBuf;
use tempfile::NamedTempFile;

/// RAII test database fixture that automatically cleans up on drop
pub struct TestDatabase {
    pub db: Database,
    temp_file: NamedTempFile,
}

impl TestDatabase {
    /// Creates a new test database with automatic cleanup
    pub fn new() -> Self {
        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        let db = Database::new(temp_file.path()).expect("Failed to create database");
        Self { db, temp_file }
    }

    /// Returns the path to the database file
    pub fn path(&self) -> &std::path::Path {
        self.temp_file.path()
    }
}

impl Drop for TestDatabase {
    fn drop(&mut self) {
        // Database connection is dropped automatically
        // Remove the temporary file
        let _ = fs::remove_file(self.temp_file.path());
    }
}

/// Builder for creating test nodes with a fluent API
pub struct NodeBuilder {
    name: String,
    detail: Option<MonitorDetail>,
    status: NodeStatus,
    last_check: Option<chrono::DateTime<chrono::Utc>>,
    response_time: Option<u64>,
    monitoring_interval: u64,
    credential_id: Option<String>,
    id: Option<i64>,
}

impl NodeBuilder {
    /// Creates a new node builder with default values
    pub fn new() -> Self {
        Self {
            name: "Test Node".to_string(),
            detail: None,
            status: NodeStatus::Offline,
            last_check: None,
            response_time: None,
            monitoring_interval: 60,
            credential_id: None,
            id: None,
        }
    }

    /// Sets the node name
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Configures as an HTTP node
    pub fn http(mut self, url: impl Into<String>, expected_status: u16) -> Self {
        self.detail = Some(MonitorDetail::Http {
            url: url.into(),
            expected_status,
        });
        self
    }

    /// Configures as a Ping node
    pub fn ping(mut self, host: impl Into<String>, count: u32, timeout: u64) -> Self {
        self.detail = Some(MonitorDetail::Ping {
            host: host.into(),
            count,
            timeout,
        });
        self
    }

    /// Sets the monitoring interval
    pub fn monitoring_interval(mut self, seconds: u64) -> Self {
        self.monitoring_interval = seconds;
        self
    }

    /// Builds the node
    pub fn build(self) -> Node {
        Node {
            id: self.id,
            name: self.name,
            detail: self
                .detail
                .expect("Node detail must be set (use .http() or .ping())"),
            status: self.status,
            last_check: self.last_check,
            response_time: self.response_time,
            monitoring_interval: self.monitoring_interval,
            credential_id: self.credential_id,
        }
    }
}

impl Default for NodeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenient fixtures for common test scenarios
pub mod fixtures {
    use super::*;
    use chrono::Utc;

    /// Creates a standard HTTP test node
    pub fn http_node() -> Node {
        NodeBuilder::new()
            .name("Test HTTP Node")
            .http("https://httpbin.org/status/200", 200)
            .build()
    }

    /// Creates an HTTP node that will fail (404)
    /// Used in network tests which are ignored by default
    #[allow(dead_code)]
    pub fn http_failure_node() -> Node {
        NodeBuilder::new()
            .name("Test HTTP Failure Node")
            .http("https://httpbin.org/status/404", 200)
            .build()
    }

    /// Creates a standard Ping test node
    pub fn ping_node() -> Node {
        NodeBuilder::new()
            .name("Test Ping Node")
            .ping("127.0.0.1", 1, 1)
            .monitoring_interval(30)
            .build()
    }

    /// Creates a test HTTP node with example.com URL for unit tests
    /// Similar to http_node() but uses example.com for non-network tests
    #[allow(dead_code)]
    pub fn unit_test_http_node() -> Node {
        Node {
            id: None,
            name: "Test HTTP Node".to_string(),
            detail: MonitorDetail::Http {
                url: "https://example.com".to_string(),
                expected_status: 200,
            },
            status: NodeStatus::Online,
            last_check: Some(Utc::now()),
            response_time: Some(150),
            monitoring_interval: 60,
            credential_id: None,
        }
    }

    /// Creates a test ping node for unit tests
    #[allow(dead_code)]
    pub fn unit_test_ping_node() -> Node {
        Node {
            id: None,
            name: "Test Ping Node".to_string(),
            detail: MonitorDetail::Ping {
                host: "192.168.1.1".to_string(),
                count: 4,
                timeout: 5,
            },
            status: NodeStatus::Offline,
            last_check: None,
            response_time: None,
            monitoring_interval: 30,
            credential_id: None,
        }
    }
}

/// Test assertions for nodes
pub mod assertions {
    use super::*;

    /// Asserts that a node has the expected HTTP properties
    pub fn assert_http_node(node: &Node, expected_url: &str, expected_status: u16) {
        match &node.detail {
            MonitorDetail::Http {
                url,
                expected_status: status,
            } => {
                assert_eq!(url, expected_url);
                assert_eq!(*status, expected_status);
            }
            _ => panic!("Expected HTTP monitor detail, got {:?}", node.detail),
        }
    }

    /// Asserts that a node has the expected Ping properties
    pub fn assert_ping_node(
        node: &Node,
        expected_host: &str,
        expected_count: u32,
        expected_timeout: u64,
    ) {
        match &node.detail {
            MonitorDetail::Ping {
                host,
                count,
                timeout,
            } => {
                assert_eq!(host, expected_host);
                assert_eq!(*count, expected_count);
                assert_eq!(*timeout, expected_timeout);
            }
            _ => panic!("Expected Ping monitor detail, got {:?}", node.detail),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_fixture_creates_and_cleans_up() {
        let temp_path: PathBuf;
        {
            let test_db = TestDatabase::new();
            temp_path = test_db.path().to_path_buf();
            assert!(temp_path.exists());
        }
        // After drop, file should be cleaned up
        assert!(!temp_path.exists());
    }

    #[test]
    fn test_node_builder_http() {
        let node = NodeBuilder::new()
            .name("Test")
            .http("https://example.com", 200)
            .monitoring_interval(30)
            .build();

        assert_eq!(node.name, "Test");
        assert_eq!(node.monitoring_interval, 30);
        assertions::assert_http_node(&node, "https://example.com", 200);
    }

    #[test]
    fn test_node_builder_ping() {
        let node = NodeBuilder::new()
            .name("Test Ping")
            .ping("192.168.1.1", 4, 5)
            .monitoring_interval(45)
            .build();

        assert_eq!(node.name, "Test Ping");
        assert_eq!(node.monitoring_interval, 45);
        assertions::assert_ping_node(&node, "192.168.1.1", 4, 5);
    }

    #[test]
    fn test_fixtures_http_node() {
        let node = fixtures::http_node();
        assert_eq!(node.name, "Test HTTP Node");
        assertions::assert_http_node(&node, "https://httpbin.org/status/200", 200);
    }

    #[test]
    fn test_fixtures_ping_node() {
        let node = fixtures::ping_node();
        assert_eq!(node.name, "Test Ping Node");
        assertions::assert_ping_node(&node, "127.0.0.1", 1, 1);
    }
}
