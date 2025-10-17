use crate::models::{MonitorDetail, MonitoringResult, Node, NodeStatus};
use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use std::net::{TcpStream, ToSocketAddrs};
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
        MonitorDetail::Tcp {
            host,
            port,
            timeout,
        } => check_tcp(host, *port, *timeout).await,
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

    // Normalize the URL to ensure it has a proper scheme
    let normalized_url = normalize_http_url(url);
    info!("Normalized URL: {}", normalized_url);

    // Build client that accepts self-signed certificates
    // This is necessary for monitoring internal services (e.g., Proxmox on private IPs)
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(Duration::from_secs(30))
        .build()?;

    let res = client.get(&normalized_url).send().await?;
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

/// Normalize HTTP URL to ensure it has a proper scheme
/// Supports both HTTP and HTTPS, and preserves port numbers
pub fn normalize_http_url(url: &str) -> String {
    // If the URL already has a scheme, use it as-is
    if url.starts_with("http://") || url.starts_with("https://") {
        url.to_string()
    } else {
        // Default to HTTPS if no scheme is specified
        format!("https://{}", url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_http_url_with_https() {
        assert_eq!(
            normalize_http_url("https://example.com"),
            "https://example.com"
        );
    }

    #[test]
    fn test_normalize_http_url_with_http() {
        assert_eq!(
            normalize_http_url("http://example.com"),
            "http://example.com"
        );
    }

    #[test]
    fn test_normalize_http_url_without_scheme() {
        assert_eq!(normalize_http_url("example.com"), "https://example.com");
    }

    #[test]
    fn test_normalize_http_url_with_port_https() {
        assert_eq!(
            normalize_http_url("https://example.com:8006"),
            "https://example.com:8006"
        );
    }

    #[test]
    fn test_normalize_http_url_with_port_http() {
        assert_eq!(
            normalize_http_url("http://example.com:8123"),
            "http://example.com:8123"
        );
    }

    #[test]
    fn test_normalize_http_url_with_port_no_scheme() {
        assert_eq!(
            normalize_http_url("example.com:8006"),
            "https://example.com:8006"
        );
    }

    #[test]
    fn test_normalize_http_url_proxmox_example() {
        assert_eq!(
            normalize_http_url("proxmox.local:8006"),
            "https://proxmox.local:8006"
        );
    }

    #[test]
    fn test_normalize_http_url_homeassistant_example() {
        assert_eq!(
            normalize_http_url("homeassistant:8123"),
            "https://homeassistant:8123"
        );
    }

    #[test]
    fn test_normalize_http_url_ip_with_port() {
        assert_eq!(
            normalize_http_url("192.168.1.100:8080"),
            "https://192.168.1.100:8080"
        );
    }

    #[test]
    fn test_normalize_http_url_http_ip_with_port() {
        assert_eq!(
            normalize_http_url("http://192.168.1.100:8080"),
            "http://192.168.1.100:8080"
        );
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

async fn check_tcp(host: &str, port: u16, timeout: u64) -> Result<String> {
    info!("Checking TCP connection to {}:{}", host, port);

    // Format the address and resolve DNS
    let addr_str = format!("{}:{}", host, port);
    let socket_addrs: Vec<_> = addr_str
        .to_socket_addrs()
        .context(format!("Failed to resolve hostname: {}", host))?
        .collect();

    if socket_addrs.is_empty() {
        return Err(anyhow!("No addresses found for {}:{}", host, port));
    }

    // Try connecting to each resolved address
    let timeout_duration = Duration::from_secs(timeout);
    let mut last_error = None;

    for socket_addr in socket_addrs {
        match TcpStream::connect_timeout(&socket_addr, timeout_duration) {
            Ok(_stream) => {
                return Ok(format!(
                    "TCP connection successful to {}:{} ({})",
                    host, port, socket_addr
                ));
            }
            Err(e) => {
                last_error = Some(e);
                continue;
            }
        }
    }

    // If we get here, all connection attempts failed
    if let Some(err) = last_error {
        Err(anyhow!("Failed to connect to {}:{} - {}", host, port, err))
    } else {
        Err(anyhow!("Failed to connect to {}:{}", host, port))
    }
}
