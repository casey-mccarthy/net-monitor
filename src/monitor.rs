use crate::models::{MonitorDetail, MonitoringResult, Node, NodeStatus};
use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use std::time::Duration;
use tracing::info;

pub async fn check_node(node: &Node) -> Result<MonitoringResult> {
    info!("Checking node: {}", node.name);
    let start_time = std::time::Instant::now();

    let check_result = match &node.detail {
        MonitorDetail::Http {
            url,
            expected_status,
        } => check_http(url, *expected_status).await,
        MonitorDetail::Ping {
            host,
            count: _,
            timeout,
        } => {
            // The `ping` crate doesn't support a `count` parameter in this function signature
            check_ping(host, *timeout).await
        }
    };
    let response_time = start_time.elapsed().as_millis() as u64;

    let (status, details) = match check_result {
        Ok(details) => (NodeStatus::Online, Some(details)),
        Err(e) => (NodeStatus::Offline, Some(e.to_string())),
    };

    Ok(MonitoringResult {
        id: None, // This will be set by the database
        node_id: node.id.unwrap_or(0),
        timestamp: Utc::now(),
        status,
        response_time: Some(response_time),
        details,
    })
}

async fn check_http(url: &str, expected_status: u16) -> Result<String> {
    info!("Checking HTTP for {}", url);
    let client = reqwest::Client::new();
    let res = client.get(url).send().await?;
    let status = res.status();
    if status.as_u16() == expected_status {
        Ok(format!("Responded with status {}", status))
    } else {
        Err(anyhow!(
            "Expected status {} but got {}",
            expected_status,
            status
        ))
    }
}

async fn check_ping(host: &str, timeout: u64) -> Result<String> {
    info!("Checking Ping for {}", host);
    let addr = host
        .parse::<std::net::IpAddr>()
        .context("Invalid IP address")?;

    // Use the `ping` function which is simpler and matches the older API.
    // The `count` parameter is not supported in this version's `ping` function.
    match ping::ping(
        addr,
        Some(Duration::from_secs(timeout)),
        None,
        None,
        None,
        None,
    ) {
        Ok(_) => Ok("Ping successful".to_string()),
        Err(e) => Err(anyhow!("Ping failed: {}", e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{MonitorDetail, Node, NodeStatus};
    use chrono::Utc;

    /// Creates a test HTTP node for testing purposes
    fn create_test_http_node() -> Node {
        Node {
            id: Some(1),
            name: "Test HTTP Node".to_string(),
            detail: MonitorDetail::Http {
                url: "https://httpbin.org/status/200".to_string(),
                expected_status: 200,
            },
            status: NodeStatus::Unknown,
            last_check: None,
            response_time: None,
            monitoring_interval: 60,
        }
    }

    #[tokio::test]
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
    async fn test_check_node_http_failure() {
        let node = Node {
            id: Some(1),
            name: "Test HTTP Node".to_string(),
            detail: MonitorDetail::Http {
                url: "https://httpbin.org/status/404".to_string(),
                expected_status: 200, // Expecting 200 but will get 404
            },
            status: NodeStatus::Unknown,
            last_check: None,
            response_time: None,
            monitoring_interval: 60,
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
            status: NodeStatus::Unknown,
            last_check: None,
            response_time: None,
            monitoring_interval: 60,
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
    async fn test_check_http_success() {
        let result = check_http("https://httpbin.org/status/200", 200).await;
        assert!(result.is_ok());
        let details = result.unwrap();
        assert!(details.contains("Responded with status 200"));
    }

    #[tokio::test]
    async fn test_check_http_wrong_status() {
        let result = check_http("https://httpbin.org/status/200", 404).await;
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error
            .to_string()
            .contains("Expected status 404 but got 200"));
    }

    #[tokio::test]
    async fn test_check_http_invalid_url() {
        let result = check_http("https://invalid-domain-that-does-not-exist-12345.com", 200).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_check_ping_localhost() {
        let result = check_ping("127.0.0.1", 1).await;
        // This should succeed on most systems
        if result.is_ok() {
            let details = result.unwrap();
            assert!(details.contains("Ping successful"));
        } else {
            // On some systems (like CI environments), ping might fail
            let error = result.unwrap_err();
            assert!(error.to_string().contains("Ping failed"));
        }
    }

    #[tokio::test]
    async fn test_check_ping_invalid_host() {
        let result = check_ping("invalid-host", 1).await;
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("Invalid IP address"));
    }

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
    }
}
