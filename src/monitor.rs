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
