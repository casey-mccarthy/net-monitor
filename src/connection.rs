use anyhow::{anyhow, Result};
use std::process::Command;
use tracing::{error, info};

/// Trait defining the connection strategy interface
pub trait ConnectionStrategy: Send + Sync {
    /// Connect to the target using the appropriate method
    fn connect(&self, target: &str) -> Result<()>;

    /// Get a description of this connection strategy
    #[allow(dead_code)]
    fn description(&self) -> &str;
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
pub struct SshConnectionStrategy;

impl SshConnectionStrategy {
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
}

impl ConnectionStrategy for SshConnectionStrategy {
    fn connect(&self, target: &str) -> Result<()> {
        let (host, port) = self.parse_target(target);
        info!("Opening SSH connection to {}:{}", host, port);

        #[cfg(target_os = "macos")]
        {
            // On macOS, use Terminal.app with osascript
            let ssh_command = if port != 22 {
                format!("ssh -p {} {}", port, host)
            } else {
                format!("ssh {}", host)
            };

            let script = format!(
                r#"tell application "Terminal"
                    activate
                    do script "{}"
                end tell"#,
                ssh_command
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
            // On Windows, use Windows Terminal if available, otherwise cmd
            let ssh_command = if port != 22 {
                format!("ssh -p {} {}", port, host)
            } else {
                format!("ssh {}", host)
            };

            // Try Windows Terminal first
            let result = Command::new("wt")
                .arg("new-tab")
                .arg("--")
                .arg("cmd")
                .arg("/k")
                .arg(&ssh_command)
                .spawn();

            if result.is_err() {
                // Fallback to cmd
                Command::new("cmd")
                    .arg("/c")
                    .arg("start")
                    .arg("cmd")
                    .arg("/k")
                    .arg(&ssh_command)
                    .spawn()
                    .map_err(|e| {
                        error!("Failed to open terminal for SSH: {}", e);
                        anyhow!("Failed to open terminal: {}", e)
                    })?;
            }
        }

        #[cfg(target_os = "linux")]
        {
            // On Linux, try common terminal emulators
            let ssh_command = if port != 22 {
                vec!["ssh", "-p", &port.to_string(), &host]
            } else {
                vec!["ssh", &host]
            };

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

                // Build the SSH command string
                let ssh_cmd_str = ssh_command.join(" ");
                if args.contains(&"bash") {
                    // For terminals that use bash -c, we need to keep the terminal open
                    cmd.arg(&format!(
                        "{}; read -p 'Press Enter to close...'",
                        ssh_cmd_str
                    ));
                } else {
                    cmd.arg(&ssh_cmd_str);
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

    fn description(&self) -> &str {
        "Open SSH connection in terminal"
    }
}

/// Ping connection strategy - for nodes that use ping monitoring
/// This defaults to SSH since ping targets are typically network devices
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
            ssh_strategy: SshConnectionStrategy,
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
        ConnectionType::Ssh => Box::new(SshConnectionStrategy),
        ConnectionType::Ping => Box::new(PingConnectionStrategy::new()),
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
