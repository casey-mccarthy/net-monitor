// Unit tests for connection module

use net_monitor::connection::{
    ConnectionStrategy, ConnectionType, HttpConnectionStrategy, PingConnectionStrategy,
    SshConnectionStrategy,
};

#[test]
fn test_http_connection_strategy_description() {
    let strategy = HttpConnectionStrategy;
    assert_eq!(strategy.description(), "Open in web browser");
}

#[test]
fn test_ssh_connection_strategy_new() {
    let strategy = SshConnectionStrategy::new();
    assert_eq!(strategy.description(), "Open SSH connection in terminal");
}

#[test]
fn test_ssh_connection_strategy_default() {
    let strategy = SshConnectionStrategy::default();
    assert_eq!(strategy.description(), "Open SSH connection in terminal");
}

#[test]
fn test_ping_connection_strategy_new() {
    let strategy = PingConnectionStrategy::new();
    assert_eq!(
        strategy.description(),
        "Connect via SSH (default for ping targets)"
    );
}

#[test]
fn test_ping_connection_strategy_default() {
    let strategy = PingConnectionStrategy::default();
    assert_eq!(
        strategy.description(),
        "Connect via SSH (default for ping targets)"
    );
}

#[test]
fn test_connection_type_equality() {
    assert_eq!(ConnectionType::Http, ConnectionType::Http);
    assert_eq!(ConnectionType::Ssh, ConnectionType::Ssh);
    assert_eq!(ConnectionType::Ping, ConnectionType::Ping);
    assert_eq!(ConnectionType::Tcp, ConnectionType::Tcp);

    assert_ne!(ConnectionType::Http, ConnectionType::Ssh);
    assert_ne!(ConnectionType::Ping, ConnectionType::Tcp);
}

#[test]
fn test_connection_type_debug() {
    // Test that ConnectionType can be formatted with Debug
    let conn_type = ConnectionType::Http;
    let debug_str = format!("{:?}", conn_type);
    assert!(debug_str.contains("Http"));
}

#[test]
fn test_connection_type_clone() {
    let original = ConnectionType::Ssh;
    let cloned = original;
    assert_eq!(original, cloned);
}

#[test]
fn test_connection_type_copy() {
    let original = ConnectionType::Tcp;
    let copied = original;
    // Both should be equal and original should still be valid
    assert_eq!(original, ConnectionType::Tcp);
    assert_eq!(copied, ConnectionType::Tcp);
}
