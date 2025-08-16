use net_monitor::database::Database;
use net_monitor::models::{MonitorDetail, Node, NodeStatus};
use tempfile::NamedTempFile;

/// Test configuration for different test scenarios
pub struct TestConfig {
    pub http_timeout: u64,
    pub ping_timeout: u64,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            http_timeout: 10,
            ping_timeout: 5,
        }
    }
}

/// Creates a temporary database for testing
pub fn create_test_database() -> (Database, NamedTempFile) {
    let temp_file = NamedTempFile::new().unwrap();
    let db = Database::new(temp_file.path()).unwrap();
    (db, temp_file)
}

/// Creates a test HTTP node with configurable parameters
pub fn create_test_http_node(
    name: &str,
    url: &str,
    expected_status: u16,
    interval: u64,
) -> Node {
    Node {
        id: None,
        name: name.to_string(),
        detail: MonitorDetail::Http {
            url: url.to_string(),
            expected_status,
        },
        status: NodeStatus::Unknown,
        last_check: None,
        response_time: None,
        monitoring_interval: interval,
    }
}

/// Creates a test ping node with configurable parameters
pub fn create_test_ping_node(
    name: &str,
    host: &str,
    count: u32,
    timeout: u64,
    interval: u64,
) -> Node {
    Node {
        id: None,
        name: name.to_string(),
        detail: MonitorDetail::Ping {
            host: host.to_string(),
            count,
            timeout,
        },
        status: NodeStatus::Unknown,
        last_check: None,
        response_time: None,
        monitoring_interval: interval,
    }
}


/// Creates a standard test HTTP node for common testing
pub fn create_standard_test_http_node() -> Node {
    create_test_http_node(
        "Standard Test HTTP Node",
        "https://httpbin.org/status/200",
        200,
        60,
    )
}

/// Creates a standard test ping node for common testing
pub fn create_standard_test_ping_node() -> Node {
    create_test_ping_node(
        "Standard Test Ping Node",
        "127.0.0.1",
        1,
        1,
        30,
    )
}

/// Asserts that a node has the expected basic properties
pub fn assert_node_basic_properties(node: &Node, expected_name: &str, expected_interval: u64) {
    assert_eq!(node.name, expected_name);
    assert_eq!(node.monitoring_interval, expected_interval);
    assert!(node.id.is_none()); // Should be None for new nodes
}

/// Asserts that a node has the expected HTTP properties
pub fn assert_http_node_properties(
    node: &Node,
    expected_url: &str,
    expected_status: u16,
) {
    if let MonitorDetail::Http { url, expected_status: status } = &node.detail {
        assert_eq!(url, expected_url);
        assert_eq!(*status, expected_status);
    } else {
        panic!("Expected HTTP monitor detail");
    }
}

/// Asserts that a node has the expected ping properties
pub fn assert_ping_node_properties(
    node: &Node,
    expected_host: &str,
    expected_count: u32,
    expected_timeout: u64,
) {
    if let MonitorDetail::Ping { host, count, timeout } = &node.detail {
        assert_eq!(host, expected_host);
        assert_eq!(*count, expected_count);
        assert_eq!(*timeout, expected_timeout);
    } else {
        panic!("Expected Ping monitor detail");
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_test_http_node() {
        let node = create_test_http_node("Test", "https://example.com", 200, 60);
        assert_node_basic_properties(&node, "Test", 60);
        assert_http_node_properties(&node, "https://example.com", 200);
    }

    #[test]
    fn test_create_test_ping_node() {
        let node = create_test_ping_node("Test", "127.0.0.1", 4, 5, 30);
        assert_node_basic_properties(&node, "Test", 30);
        assert_ping_node_properties(&node, "127.0.0.1", 4, 5);
    }


    #[test]
    fn test_create_standard_test_nodes() {
        let http_node = create_standard_test_http_node();
        assert_node_basic_properties(&http_node, "Standard Test HTTP Node", 60);
        assert_http_node_properties(&http_node, "https://httpbin.org/status/200", 200);

        let ping_node = create_standard_test_ping_node();
        assert_node_basic_properties(&ping_node, "Standard Test Ping Node", 30);
        assert_ping_node_properties(&ping_node, "127.0.0.1", 1, 1);
    }
} 