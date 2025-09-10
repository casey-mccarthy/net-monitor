use crate::credentials::{CredentialStore, SshCredential};
use anyhow::{anyhow, Result};
use std::net::{TcpStream, ToSocketAddrs};
use std::process::Command;
use tracing::{error, info, warn};

/// Trait defining the connection strategy interface
pub trait ConnectionStrategy: Send + Sync {
    /// Connect to the target using the appropriate method
    fn connect(&self, target: &str) -> Result<()>;

    /// Get a description of this connection strategy
    #[allow(dead_code)]
    fn description(&self) -> &str;
}

/// Enhanced trait for connection strategies that support authentication
pub trait AuthenticatedConnectionStrategy: ConnectionStrategy {
    /// Connect to the target using provided credentials
    fn connect_with_credentials(&self, target: &str, credential: &SshCredential) -> Result<()>;

    /// Test if a connection can be established (without fully connecting)
    #[allow(dead_code)]
    fn test_connection(&self, target: &str, credential: Option<&SshCredential>) -> Result<bool>;
}

/// HTTP connection strategy - opens URLs in the default web browser
pub struct HttpConnectionStrategy;

impl ConnectionStrategy for HttpConnectionStrategy {
    fn connect(&self, target: &str) -> Result<()> {
        info!("Opening URL in browser: {}", target);

        // Ensure the URL has a scheme
        let url = if !target.starts_with("http://") && !target.starts_with("https://") {
            format!("https://{}", target)
        } else {
            target.to_string()
        };

        open::that(&url).map_err(|e| {
            error!("Failed to open URL {}: {}", url, e);
            anyhow!("Failed to open URL: {}", e)
        })?;

        Ok(())
    }

    fn description(&self) -> &str {
        "Open in web browser"
    }
}

/// SSH connection strategy - opens SSH connection in terminal
pub struct SshConnectionStrategy {
    /// Optional credential store for retrieving credentials
    #[allow(dead_code)]
    pub credential_store: Option<Box<dyn CredentialStore>>,
}

impl Default for SshConnectionStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl SshConnectionStrategy {
    /// Create a new SSH connection strategy
    pub fn new() -> Self {
        Self {
            credential_store: None,
        }
    }

    /// Create a new SSH connection strategy with credential store
    pub fn with_credential_store(credential_store: Box<dyn CredentialStore>) -> Self {
        Self {
            credential_store: Some(credential_store),
        }
    }

    /// Parse the target to extract host and optional port
    fn parse_target(&self, target: &str) -> (String, u16) {
        // Check if target contains port (e.g., "hostname:2222" or "192.168.1.1:2222")
        if let Some(colon_pos) = target.rfind(':') {
            // Check if what comes after the colon is a valid port number
            if let Ok(port) = target[colon_pos + 1..].parse::<u16>() {
                let host = target[..colon_pos].to_string();
                return (host, port);
            }
        }

        // Default SSH port
        (target.to_string(), 22)
    }

    /// Build SSH command with credentials
    fn build_ssh_command(
        &self,
        host: &str,
        port: u16,
        credential: Option<&SshCredential>,
    ) -> Vec<String> {
        let mut command = Vec::new();

        match credential {
            Some(SshCredential::Default) | None => {
                // Use default SSH behavior
                command.push("ssh".to_string());
                if port != 22 {
                    command.push("-p".to_string());
                    command.push(port.to_string());
                }
                command.push(host.to_string());
            }
            Some(SshCredential::Password { username, .. }) => {
                // For password auth, we'll use ssh with username
                command.push("ssh".to_string());
                if port != 22 {
                    command.push("-p".to_string());
                    command.push(port.to_string());
                }
                command.push(format!("{}@{}", username, host));
            }
            Some(SshCredential::Key {
                username,
                private_key_path,
                ..
            }) => {
                // Use specific SSH key
                command.push("ssh".to_string());
                if port != 22 {
                    command.push("-p".to_string());
                    command.push(port.to_string());
                }
                command.push("-i".to_string());
                command.push(private_key_path.to_string_lossy().to_string());
                command.push(format!("{}@{}", username, host));
            }
            Some(SshCredential::KeyData {
                username,
                private_key_data: _,
                ..
            }) => {
                // For embedded key data, we'll write to a temp file
                // Note: This is a simplified implementation - in production you'd want better temp file security
                command.push("ssh".to_string());
                if port != 22 {
                    command.push("-p".to_string());
                    command.push(port.to_string());
                }
                // We'll need to handle the temp file creation during connection
                // For now, just connect with username and let SSH use default keys
                warn!(
                    "Using default SSH behavior for embedded key data - temp file creation needed"
                );
                command.push(format!("{}@{}", username, host));
            }
        }

        command
    }

    /// Test TCP connection to SSH port
    fn test_tcp_connection(&self, host: &str, port: u16) -> Result<bool> {
        let addr = format!("{}:{}", host, port);
        match addr.to_socket_addrs() {
            Ok(mut addrs) => {
                if let Some(addr) = addrs.next() {
                    match TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(5)) {
                        Ok(_) => Ok(true),
                        Err(_) => Ok(false),
                    }
                } else {
                    Ok(false)
                }
            }
            Err(_) => Ok(false),
        }
    }
}

impl ConnectionStrategy for SshConnectionStrategy {
    fn connect(&self, target: &str) -> Result<()> {
        self.connect_with_credentials(target, &SshCredential::Default)
    }

    fn description(&self) -> &str {
        "Open SSH connection in terminal"
    }
}

impl AuthenticatedConnectionStrategy for SshConnectionStrategy {
    fn connect_with_credentials(&self, target: &str, credential: &SshCredential) -> Result<()> {
        let (host, port) = self.parse_target(target);
        info!(
            "Opening SSH connection to {}:{} with credentials",
            host, port
        );

        let ssh_command_vec = self.build_ssh_command(&host, port, Some(credential));
        let ssh_command_str = ssh_command_vec.join(" ");

        #[cfg(target_os = "macos")]
        {
            // On macOS, use Terminal.app with osascript
            let script = format!(
                r#"tell application "Terminal"
                    activate
                    do script "{}"
                end tell"#,
                ssh_command_str
            );

            Command::new("osascript")
                .arg("-e")
                .arg(&script)
                .spawn()
                .map_err(|e| {
                    error!("Failed to open Terminal for SSH: {}", e);
                    anyhow!("Failed to open Terminal: {}", e)
                })?;
        }

        #[cfg(target_os = "windows")]
        {
            // Try Windows Terminal first
            let result = Command::new("wt")
                .arg("new-tab")
                .arg("--")
                .arg("cmd")
                .arg("/k")
                .arg(&ssh_command_str)
                .spawn();

            if result.is_err() {
                // Fallback to cmd
                Command::new("cmd")
                    .arg("/c")
                    .arg("start")
                    .arg("cmd")
                    .arg("/k")
                    .arg(&ssh_command_str)
                    .spawn()
                    .map_err(|e| {
                        error!("Failed to open terminal for SSH: {}", e);
                        anyhow!("Failed to open terminal: {}", e)
                    })?;
            }
        }

        #[cfg(target_os = "linux")]
        {
            // Try different terminal emulators in order of preference
            let terminals = [
                ("gnome-terminal", vec!["--", "bash", "-c"]),
                ("konsole", vec!["-e"]),
                ("xfce4-terminal", vec!["-e"]),
                ("xterm", vec!["-e"]),
            ];

            let mut success = false;
            for (terminal, args) in terminals.iter() {
                let mut cmd = Command::new(terminal);
                for arg in args {
                    cmd.arg(arg);
                }

                if args.contains(&"bash") {
                    // For terminals that use bash -c, we need to keep the terminal open
                    cmd.arg(&format!(
                        "{}; read -p 'Press Enter to close...'",
                        ssh_command_str
                    ));
                } else {
                    cmd.arg(&ssh_command_str);
                }

                if cmd.spawn().is_ok() {
                    success = true;
                    break;
                }
            }

            if !success {
                return Err(anyhow!("No suitable terminal emulator found"));
            }
        }

        Ok(())
    }

    fn test_connection(&self, target: &str, _credential: Option<&SshCredential>) -> Result<bool> {
        let (host, port) = self.parse_target(target);

        // First, test basic TCP connectivity
        if !self.test_tcp_connection(&host, port)? {
            return Ok(false);
        }

        // For now, if TCP connection works, assume SSH will work
        // In a full implementation, we could use the ssh2 crate to test actual SSH auth
        Ok(true)
    }
}

/// Ping connection strategy - for nodes that use ping monitoring
/// This defaults to SSH since ping targets are typically network devices
#[allow(dead_code)]
pub struct PingConnectionStrategy {
    ssh_strategy: SshConnectionStrategy,
}

impl Default for PingConnectionStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl PingConnectionStrategy {
    pub fn new() -> Self {
        Self {
            ssh_strategy: SshConnectionStrategy::new(),
        }
    }

    pub fn with_credential_store(credential_store: Box<dyn CredentialStore>) -> Self {
        Self {
            ssh_strategy: SshConnectionStrategy::with_credential_store(credential_store),
        }
    }
}

impl ConnectionStrategy for PingConnectionStrategy {
    fn connect(&self, target: &str) -> Result<()> {
        // For ping targets, we assume SSH is the desired connection method
        self.ssh_strategy.connect(target)
    }

    fn description(&self) -> &str {
        "Connect via SSH (default for ping targets)"
    }
}

impl AuthenticatedConnectionStrategy for PingConnectionStrategy {
    fn connect_with_credentials(&self, target: &str, credential: &SshCredential) -> Result<()> {
        // Delegate to SSH strategy
        self.ssh_strategy
            .connect_with_credentials(target, credential)
    }

    fn test_connection(&self, target: &str, _credential: Option<&SshCredential>) -> Result<bool> {
        // Delegate to SSH strategy
        self.ssh_strategy.test_connection(target, _credential)
    }
}

/// Context for managing connection strategies
pub struct ConnectionContext {
    strategy: Box<dyn ConnectionStrategy>,
}

impl ConnectionContext {
    /// Create a new connection context with the given strategy
    pub fn new(strategy: Box<dyn ConnectionStrategy>) -> Self {
        Self { strategy }
    }

    /// Execute the connection using the configured strategy
    pub fn connect(&self, target: &str) -> Result<()> {
        self.strategy.connect(target)
    }

    /// Get the description of the current strategy
    #[allow(dead_code)]
    pub fn description(&self) -> &str {
        self.strategy.description()
    }
}

/// Factory function to create appropriate connection strategy based on connection type
pub fn create_connection_strategy(connection_type: ConnectionType) -> Box<dyn ConnectionStrategy> {
    match connection_type {
        ConnectionType::Http => Box::new(HttpConnectionStrategy),
        ConnectionType::Ssh => Box::new(SshConnectionStrategy::new()),
        ConnectionType::Ping => Box::new(PingConnectionStrategy::new()),
    }
}

/// Factory function to create authenticated connection strategy
pub fn create_authenticated_connection_strategy(
    connection_type: ConnectionType,
    credential_store: Option<Box<dyn CredentialStore>>,
) -> Box<dyn AuthenticatedConnectionStrategy> {
    match connection_type {
        ConnectionType::Http => {
            // HTTP doesn't need authentication for our use case
            // We could create an AuthenticatedHttpConnectionStrategy if needed
            Box::new(SshConnectionStrategy::new()) // Placeholder
        }
        ConnectionType::Ssh => {
            if let Some(store) = credential_store {
                Box::new(SshConnectionStrategy::with_credential_store(store))
            } else {
                Box::new(SshConnectionStrategy::new())
            }
        }
        ConnectionType::Ping => {
            if let Some(store) = credential_store {
                Box::new(PingConnectionStrategy::with_credential_store(store))
            } else {
                Box::new(PingConnectionStrategy::new())
            }
        }
    }
}

/// Enum representing different connection types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ConnectionType {
    Http,
    Ssh,
    Ping,
}
