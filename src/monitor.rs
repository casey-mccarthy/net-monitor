use crate::models::{MonitorDetail, MonitoringResult, Node, NodeStatus};
use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use std::io::ErrorKind;
use std::net::{IpAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;
use tracing::{info, warn};

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
            count,
            timeout,
        } => check_ping(host, *count, *timeout).await,
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

/// Resolves a ping target to an IP address, accepting either a literal IP or a hostname.
pub fn resolve_ping_host(host: &str) -> Result<IpAddr> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(ip);
    }

    // `to_socket_addrs` needs a port; it is irrelevant for ICMP.
    (host, 0)
        .to_socket_addrs()
        .with_context(|| format!("Failed to resolve hostname: {}", host))?
        .map(|addr| addr.ip())
        .next()
        .ok_or_else(|| anyhow!("No addresses found for {}", host))
}

/// True if a ping error means the host simply did not answer in time
/// (as opposed to the socket itself being unusable).
fn is_ping_timeout(error: &ping::Error) -> bool {
    matches!(
        error,
        ping::Error::IoError { error }
            if matches!(error.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock)
    )
}

/// Sends a single ICMP echo request and returns the round-trip time.
///
/// Starts with the platform's default socket type (datagram on Linux/macOS,
/// raw on Windows). Raw sockets need root or CAP_NET_RAW, and datagram ICMP
/// sockets are disabled on some Linux distributions, so if the socket could
/// not be used at all we retry once with the other kind. A timeout is never
/// retried: that already means the host did not answer.
fn send_ping(addr: IpAddr, timeout: Duration) -> std::result::Result<Duration, ping::Error> {
    let mut first = ping::new(addr);
    first.timeout(timeout);
    let first_error = match first.send() {
        Ok(reply) => return Ok(reply.rtt),
        Err(e) if is_ping_timeout(&e) => return Err(e),
        Err(e) => e,
    };

    let fallback_type = if std::env::consts::OS == "windows" {
        ping::SocketType::DGRAM
    } else {
        ping::SocketType::RAW
    };
    warn!(
        "Ping to {} could not use the default socket type ({}); retrying with {:?}",
        addr, first_error, fallback_type
    );

    let mut second = ping::new(addr);
    second.socket_type(fallback_type).timeout(timeout);
    match second.send() {
        Ok(reply) => Ok(reply.rtt),
        // Report the original error: it describes why the normal path failed.
        Err(e) if !is_ping_timeout(&e) => Err(first_error),
        Err(e) => Err(e),
    }
}

async fn check_ping(host: &str, count: u32, timeout: u64) -> Result<String> {
    info!("Checking Ping for {}", host);
    let addr = resolve_ping_host(host)?;

    // A zero timeout would fail every attempt; a zero count would never try.
    let timeout = Duration::from_secs(timeout.max(1));
    let attempts = count.max(1);

    let mut last_error = None;
    for attempt in 1..=attempts {
        match send_ping(addr, timeout) {
            Ok(rtt) => {
                return Ok(format!(
                    "Ping reply from {} in {}ms (attempt {} of {})",
                    addr,
                    rtt.as_millis(),
                    attempt,
                    attempts
                ));
            }
            Err(e) => {
                let message = if is_ping_timeout(&e) {
                    format!("no reply within {}s", timeout.as_secs())
                } else {
                    e.to_string()
                };
                last_error = Some(message);
            }
        }
    }

    Err(anyhow!(
        "Ping to {} failed after {} attempt(s): {}",
        host,
        attempts,
        last_error.unwrap_or_else(|| "unknown error".to_string())
    ))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_ping_host_accepts_ipv4_literal() {
        assert_eq!(
            resolve_ping_host("192.0.2.10").unwrap(),
            "192.0.2.10".parse::<IpAddr>().unwrap()
        );
    }

    #[test]
    fn test_resolve_ping_host_accepts_ipv6_literal() {
        assert_eq!(
            resolve_ping_host("::1").unwrap(),
            "::1".parse::<IpAddr>().unwrap()
        );
    }

    #[test]
    fn test_resolve_ping_host_resolves_localhost() {
        let ip = resolve_ping_host("localhost").unwrap();
        assert!(ip.is_loopback());
    }

    #[test]
    fn test_resolve_ping_host_rejects_garbage() {
        let err = resolve_ping_host("not-a-valid-ip-address-!!!").unwrap_err();
        assert!(err.to_string().contains("Failed to resolve hostname"));
    }

    #[test]
    fn test_is_ping_timeout_matches_timeout_kinds() {
        let timed_out = ping::Error::IoError {
            error: std::io::Error::new(ErrorKind::TimedOut, "t"),
        };
        let would_block = ping::Error::IoError {
            error: std::io::Error::new(ErrorKind::WouldBlock, "w"),
        };
        let denied = ping::Error::IoError {
            error: std::io::Error::new(ErrorKind::PermissionDenied, "p"),
        };
        assert!(is_ping_timeout(&timed_out));
        assert!(is_ping_timeout(&would_block));
        assert!(!is_ping_timeout(&denied));
        assert!(!is_ping_timeout(&ping::Error::InternalError));
    }

    /// Requires ICMP access (unprivileged datagram sockets or CAP_NET_RAW),
    /// so it only runs with the network-tests feature.
    #[cfg(feature = "network-tests")]
    #[tokio::test]
    async fn test_check_ping_loopback_succeeds_unprivileged() {
        let result = check_ping("127.0.0.1", 1, 2).await.unwrap();
        assert!(
            result.starts_with("Ping reply from 127.0.0.1"),
            "{}",
            result
        );
    }

    #[cfg(feature = "network-tests")]
    #[tokio::test]
    async fn test_check_ping_unreachable_reports_no_reply() {
        // 192.0.2.0/24 (TEST-NET-1) is reserved and never routed
        let err = check_ping("192.0.2.1", 2, 1).await.unwrap_err().to_string();
        assert!(err.contains("after 2 attempt(s)"), "{}", err);
        assert!(err.contains("no reply within 1s"), "{}", err);
    }

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
