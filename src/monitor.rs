use crate::models::{MonitorDetail, MonitoringResult, Node, NodeStatus};
use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use std::time::Duration;
use tracing::info;

#[tokio::main]
pub async fn check_node(node: &Node) -> Result<MonitoringResult> {
    info!("Checking node: {}", node.name);
    let start_time = std::time::Instant::now();

    let check_result = match &node.detail {
        MonitorDetail::Http { url, expected_status } => {
            check_http(url, *expected_status).await
        }
        MonitorDetail::Ping { host, count: _, timeout } => {
            // The `ping` crate doesn't support a `count` parameter in this function signature
            check_ping(host, *timeout).await
        }
        MonitorDetail::Snmp { target, community, oid } => {
            check_snmp(target, community, oid).await
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
        Err(anyhow!("Expected status {} but got {}", expected_status, status))
    }
}

async fn check_ping(host: &str, timeout: u64) -> Result<String> {
    info!("Checking Ping for {}", host);
    let addr = host.parse::<std::net::IpAddr>().context("Invalid IP address")?;

    // Use the `ping` function which is simpler and matches the older API.
    // The `count` parameter is not supported in this version's `ping` function.
    match ping::ping(addr, Some(Duration::from_secs(timeout)), None, None, None, None) {
        Ok(_) => Ok("Ping successful".to_string()),
        Err(e) => Err(anyhow!("Ping failed: {}", e)),
    }
}

async fn check_snmp(target: &str, community: &str, oid: &str) -> Result<String> {
    info!("Checking SNMP for {}", target);
    // The `snmp` crate's `get` method takes an OID as a slice of u32 integers.
    let oid_parts: std::result::Result<Vec<u32>, _> = oid.split('.').map(|s| s.parse::<u32>()).collect();
    let oid_vec = match oid_parts {
        Ok(vec) => vec,
        Err(e) => return Err(anyhow!("Invalid OID '{}': {}", oid, e)),
    };

    // The session must be mutable to be used.
    let mut session = snmp::SyncSession::new(target, community.as_bytes(), Some(Duration::from_secs(2)), 0)
        .map_err(|e| anyhow!("SNMP Session error: {:?}", e))?;

    let mut response = session.get(&oid_vec).map_err(|e| anyhow!("SNMP GET error: {:?}", e))?;
    
    if let Some((_oid, val)) = response.varbinds.next() {
        Ok(format!("{:?}", val))
    } else {
        Err(anyhow!("No SNMP response"))
    }
} 