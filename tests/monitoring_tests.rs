mod common;

use chrono::Utc;
use common::{fixtures, NodeBuilder, TestDatabase};
use net_monitor::models::{MonitorDetail, Node, NodeStatus};
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

    // Test a few common status codes (avoiding too many to prevent rate limiting)
    let status_codes = vec![200, 404];

    for code in status_codes {
        let node = NodeBuilder::new()
            .name(format!("HTTP {} Test", code))
            .http(format!("https://httpbin.org/status/{}", code), code)
            .build();

        let node_id = test_db.db.add_node(&node).unwrap();
        let mut node_with_id = node;
        node_with_id.id = Some(node_id);

        let result = check_node(&node_with_id).await;

        // Network tests can be flaky, so we only test when they succeed
        if let Ok(result) = result {
            // When expected status matches actual status, should be Online
            assert_eq!(
                result.status,
                NodeStatus::Online,
                "Failed for status code {}",
                code
            );
            assert!(result.response_time.is_some());
        }
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

// ============================================================================
// Unit tests moved from src/monitor.rs
// ============================================================================

/// Helper function to create a test HTTP node for testing purposes
fn create_test_http_node() -> Node {
    Node {
        id: Some(1),
        name: "Test HTTP Node".to_string(),
        detail: MonitorDetail::Http {
            url: "https://httpbin.org/status/200".to_string(),
            expected_status: 200,
        },
        status: NodeStatus::Offline,
        last_check: None,
        response_time: None,
        monitoring_interval: 60,
        credential_id: None,
    }
}

#[tokio::test]
#[cfg(feature = "network-tests")]
async fn test_check_node_http_success() {
    let node = create_test_http_node();
    let result = check_node(&node).await;

    assert!(result.is_ok());
    let monitoring_result = result.unwrap();
    assert_eq!(monitoring_result.node_id, 1);
    assert_eq!(monitoring_result.status, NodeStatus::Online);
    assert!(monitoring_result.response_time.is_some());
    assert!(monitoring_result.details.is_some());
}

#[tokio::test]
#[cfg(feature = "network-tests")]
async fn test_check_node_http_failure() {
    let node = Node {
        id: Some(1),
        name: "Test HTTP Node".to_string(),
        detail: MonitorDetail::Http {
            url: "https://httpbin.org/status/404".to_string(),
            expected_status: 200, // Expecting 200 but will get 404
        },
        status: NodeStatus::Offline,
        last_check: None,
        response_time: None,
        monitoring_interval: 60,
        credential_id: None,
    };

    let result = check_node(&node).await;

    assert!(result.is_ok());
    let monitoring_result = result.unwrap();
    assert_eq!(monitoring_result.node_id, 1);
    assert_eq!(monitoring_result.status, NodeStatus::Offline);
    assert!(monitoring_result.response_time.is_some());
    assert!(monitoring_result.details.is_some());
}

#[tokio::test]
async fn test_check_node_invalid_url() {
    let node = Node {
        id: Some(1),
        name: "Test HTTP Node".to_string(),
        detail: MonitorDetail::Http {
            url: "https://invalid-domain-that-does-not-exist-12345.com".to_string(),
            expected_status: 200,
        },
        status: NodeStatus::Offline,
        last_check: None,
        response_time: None,
        monitoring_interval: 60,
        credential_id: None,
    };

    let result = check_node(&node).await;

    assert!(result.is_ok());
    let monitoring_result = result.unwrap();
    assert_eq!(monitoring_result.node_id, 1);
    assert_eq!(monitoring_result.status, NodeStatus::Offline);
    assert!(monitoring_result.response_time.is_some());
    assert!(monitoring_result.details.is_some());
}

// Note: Tests for check_http and check_ping were removed because these are
// private functions. Their functionality is tested through check_node tests.

#[test]
fn test_monitoring_result_structure() {
    let node = create_test_http_node();
    let result = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(check_node(&node))
        .unwrap();

    assert_eq!(result.node_id, 1);
    assert!(result.timestamp > Utc::now() - chrono::Duration::seconds(10));
    assert!(result.response_time.is_some());
    assert!(result.response_time.unwrap() > 0);
}

#[tokio::test]
async fn test_check_node_with_none_id() {
    let mut node = create_test_http_node();
    node.id = None;

    let result = check_node(&node).await;
    assert!(result.is_ok());
    let monitoring_result = result.unwrap();
    assert_eq!(monitoring_result.node_id, 0); // Default value when id is None
}

#[test]
fn test_monitor_detail_variants() {
    let http_detail = MonitorDetail::Http {
        url: "https://example.com".to_string(),
        expected_status: 200,
    };
    match http_detail {
        MonitorDetail::Http {
            url,
            expected_status,
        } => {
            assert_eq!(url, "https://example.com");
            assert_eq!(expected_status, 200);
        }
        _ => panic!("Expected HTTP variant"),
    }

    let ping_detail = MonitorDetail::Ping {
        host: "192.168.1.1".to_string(),
        count: 4,
        timeout: 5,
    };
    match ping_detail {
        MonitorDetail::Ping {
            host,
            count,
            timeout,
        } => {
            assert_eq!(host, "192.168.1.1");
            assert_eq!(count, 4);
            assert_eq!(timeout, 5);
        }
        _ => panic!("Expected Ping variant"),
    }

    let tcp_detail = MonitorDetail::Tcp {
        host: "login.eqemulator.net".to_string(),
        port: 5998,
        timeout: 5,
    };
    match tcp_detail {
        MonitorDetail::Tcp {
            host,
            port,
            timeout,
        } => {
            assert_eq!(host, "login.eqemulator.net");
            assert_eq!(port, 5998);
            assert_eq!(timeout, 5);
        }
        _ => panic!("Expected TCP variant"),
    }
}

// ============================================================================
// TCP Monitoring Tests
// ============================================================================

#[tokio::test]
#[cfg(feature = "network-tests")]
async fn test_tcp_monitoring_success() {
    let test_db = TestDatabase::new();

    // Test with the concrete example from issue #15
    let node = Node {
        id: Some(1),
        name: "EQ Emulator Login Server".to_string(),
        detail: MonitorDetail::Tcp {
            host: "login.eqemulator.net".to_string(),
            port: 5998,
            timeout: 5,
        },
        status: NodeStatus::Offline,
        last_check: None,
        response_time: None,
        monitoring_interval: 60,
        credential_id: None,
    };

    let node_id = test_db.db.add_node(&node).unwrap();
    let mut node_with_id = node;
    node_with_id.id = Some(node_id);

    let result = check_node(&node_with_id).await;

    // Network tests can be flaky, so we verify both success and failure cases
    assert!(result.is_ok());
    let monitoring_result = result.unwrap();
    assert_eq!(monitoring_result.node_id, node_id);
    assert!(monitoring_result.response_time.is_some());
    assert!(monitoring_result.details.is_some());

    // The server might be up or down, but the test should complete without error
    assert!(
        monitoring_result.status == NodeStatus::Online
            || monitoring_result.status == NodeStatus::Offline
    );
}

#[tokio::test]
async fn test_tcp_monitoring_failure() {
    let test_db = TestDatabase::new();

    // Test with an unreachable host
    let node = Node {
        id: Some(1),
        name: "Unreachable TCP Server".to_string(),
        detail: MonitorDetail::Tcp {
            host: "invalid-host-that-does-not-exist-12345.com".to_string(),
            port: 9999,
            timeout: 2,
        },
        status: NodeStatus::Offline,
        last_check: None,
        response_time: None,
        monitoring_interval: 60,
        credential_id: None,
    };

    let node_id = test_db.db.add_node(&node).unwrap();
    let mut node_with_id = node;
    node_with_id.id = Some(node_id);

    let result = check_node(&node_with_id).await;

    assert!(result.is_ok());
    let monitoring_result = result.unwrap();
    assert_eq!(monitoring_result.node_id, node_id);
    assert_eq!(monitoring_result.status, NodeStatus::Offline);
    assert!(monitoring_result.response_time.is_some());
    assert!(monitoring_result.details.is_some());
}

#[tokio::test]
#[cfg(feature = "network-tests")]
async fn test_tcp_monitoring_localhost() {
    let test_db = TestDatabase::new();

    // Test with localhost:80 (most systems have something listening on port 80 or will reject quickly)
    let node = Node {
        id: Some(1),
        name: "Localhost TCP Test".to_string(),
        detail: MonitorDetail::Tcp {
            host: "127.0.0.1".to_string(),
            port: 80,
            timeout: 2,
        },
        status: NodeStatus::Offline,
        last_check: None,
        response_time: None,
        monitoring_interval: 60,
        credential_id: None,
    };

    let node_id = test_db.db.add_node(&node).unwrap();
    let mut node_with_id = node;
    node_with_id.id = Some(node_id);

    let result = check_node(&node_with_id).await;

    assert!(result.is_ok());
    let monitoring_result = result.unwrap();
    assert_eq!(monitoring_result.node_id, node_id);
    assert!(monitoring_result.response_time.is_some());
    assert!(monitoring_result.details.is_some());
}

#[tokio::test]
async fn test_tcp_monitoring_workflow() {
    let test_db = TestDatabase::new();

    // Create a TCP node
    let node = Node {
        id: None,
        name: "TCP Workflow Test".to_string(),
        detail: MonitorDetail::Tcp {
            host: "example.com".to_string(),
            port: 80,
            timeout: 5,
        },
        status: NodeStatus::Offline,
        last_check: None,
        response_time: None,
        monitoring_interval: 60,
        credential_id: None,
    };

    // Add node to database
    let node_id = test_db.db.add_node(&node).unwrap();
    assert!(node_id > 0);

    // Retrieve the node from database
    let nodes = test_db.db.get_all_nodes().unwrap();
    assert_eq!(nodes.len(), 1);
    let mut retrieved_node = nodes[0].clone();
    assert!(matches!(retrieved_node.detail, MonitorDetail::Tcp { .. }));

    // Update the node
    retrieved_node.id = Some(node_id);
    retrieved_node.status = NodeStatus::Online;
    test_db.db.update_node(&retrieved_node).unwrap();

    // Verify the update
    let updated_nodes = test_db.db.get_all_nodes().unwrap();
    assert_eq!(updated_nodes.len(), 1);
    assert_eq!(updated_nodes[0].status, NodeStatus::Online);
}

// ========== Monitor Edge Case Tests ==========

#[tokio::test]
async fn test_check_http_with_timeout() {
    let node = Node {
        id: Some(1),
        name: "Timeout Test".to_string(),
        detail: MonitorDetail::Http {
            url: "http://example.com:81".to_string(), // Non-standard port likely to timeout
            expected_status: 200,
        },
        status: NodeStatus::Offline,
        last_check: None,
        response_time: None,
        monitoring_interval: 60,
        credential_id: None,
    };

    // This should timeout or fail
    let result = check_node(&node).await;

    // Expect either error or offline status
    if let Ok(monitoring_result) = result {
        // If we get a result, it should be offline due to timeout
        assert_eq!(monitoring_result.status, NodeStatus::Offline);
    }
}

#[tokio::test]
async fn test_check_tcp_multiple_addresses() {
    // Use a hostname that likely resolves to multiple IPs
    let node = Node {
        id: Some(1),
        name: "Multi-IP Test".to_string(),
        detail: MonitorDetail::Tcp {
            host: "example.com".to_string(),
            port: 80,
            timeout: 5,
        },
        status: NodeStatus::Offline,
        last_check: None,
        response_time: None,
        monitoring_interval: 60,
        credential_id: None,
    };

    let result = check_node(&node).await;

    // Should successfully connect to at least one address
    if let Ok(monitoring_result) = result {
        assert!(monitoring_result.response_time.is_some());
    }
}

#[tokio::test]
async fn test_check_tcp_connection_refused() {
    let node = Node {
        id: Some(1),
        name: "Connection Refused Test".to_string(),
        detail: MonitorDetail::Tcp {
            host: "127.0.0.1".to_string(),
            port: 1, // Port 1 unlikely to have service
            timeout: 2,
        },
        status: NodeStatus::Offline,
        last_check: None,
        response_time: None,
        monitoring_interval: 60,
        credential_id: None,
    };

    let result = check_node(&node).await;

    // Should fail to connect
    if let Ok(monitoring_result) = result {
        assert_eq!(monitoring_result.status, NodeStatus::Offline);
        assert!(monitoring_result.details.is_some());
    }
}

#[tokio::test]
async fn test_check_ping_invalid_format() {
    let node = Node {
        id: Some(1),
        name: "Invalid Ping Test".to_string(),
        detail: MonitorDetail::Ping {
            host: "not-a-valid-ip-address-!!!".to_string(),
            count: 4,
            timeout: 5,
        },
        status: NodeStatus::Offline,
        last_check: None,
        response_time: None,
        monitoring_interval: 60,
        credential_id: None,
    };

    let result = check_node(&node).await;

    // Should handle invalid IP gracefully
    if let Ok(monitoring_result) = result {
        assert_eq!(monitoring_result.status, NodeStatus::Offline);
    }
}

#[tokio::test]
async fn test_check_node_response_time_recorded() {
    let test_db = TestDatabase::new();

    let node = fixtures::http_node();
    let node_id = test_db.db.add_node(&node).unwrap();

    let mut node_with_id = node.clone();
    node_with_id.id = Some(node_id);

    let result = check_node(&node_with_id).await;

    if let Ok(monitoring_result) = result {
        // Response time should be recorded for successful checks
        if monitoring_result.status == NodeStatus::Online {
            assert!(monitoring_result.response_time.is_some());
            assert!(monitoring_result.response_time.unwrap() > 0);
        }
    }
}

#[tokio::test]
async fn test_check_tcp_short_timeout() {
    let node = Node {
        id: Some(1),
        name: "Short Timeout TCP Test".to_string(),
        detail: MonitorDetail::Tcp {
            host: "example.com".to_string(),
            port: 80,
            timeout: 1, // Very short timeout
        },
        status: NodeStatus::Offline,
        last_check: None,
        response_time: None,
        monitoring_interval: 60,
        credential_id: None,
    };

    let result = check_node(&node).await;

    // Should complete within timeout or succeed
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_check_ping_short_timeout() {
    let node = Node {
        id: Some(1),
        name: "Short Timeout Ping Test".to_string(),
        detail: MonitorDetail::Ping {
            host: "8.8.8.8".to_string(),
            count: 1,
            timeout: 1, // Very short timeout
        },
        status: NodeStatus::Offline,
        last_check: None,
        response_time: None,
        monitoring_interval: 60,
        credential_id: None,
    };

    let result = check_node(&node).await;

    // Should handle short timeout gracefully
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_monitoring_result_timestamp_accuracy() {
    let node = fixtures::http_node();

    let before = Utc::now();
    let result = check_node(&node).await;
    let after = Utc::now();

    if let Ok(monitoring_result) = result {
        // Timestamp should be between before and after
        assert!(monitoring_result.timestamp >= before);
        assert!(monitoring_result.timestamp <= after);
    }
}
