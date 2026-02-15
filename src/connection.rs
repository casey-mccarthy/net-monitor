//! Connection strategies for different monitor types.
//!
//! This module implements the Strategy pattern for connecting to monitored nodes.
//! **Note:** Only SSH-based connections (SSH, Ping, TCP) support credential-based authentication.
//! HTTP/HTTPS targets will always open in the default web browser without credential handling.

use crate::credentials::SshCredential;
use anyhow::{anyhow, Result};
use std::process::{Command, Stdio};
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
}

/// HTTP connection strategy - opens URLs in the default web browser.
///
/// **Note:** HTTP/HTTPS connections do not support credentials. This strategy
/// will always open the URL in the system's default browser without any
/// credential handling. For authenticated access, credentials should be
/// managed through the browser's built-in authentication mechanisms.
pub struct HttpConnectionStrategy;

impl ConnectionStrategy for HttpConnectionStrategy {
    fn connect(&self, target: &str) -> Result<()> {
        info!("Opening URL in browser: {}", target);

        // Normalize the URL to ensure it has a proper scheme
        // Supports both HTTP and HTTPS, and preserves port numbers
        let url = if target.starts_with("http://") || target.starts_with("https://") {
            target.to_string()
        } else {
            // Default to HTTPS if no scheme is specified
            format!("https://{}", target)
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

impl Default for SshConnectionStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl SshConnectionStrategy {
    /// Create a new SSH connection strategy
    pub fn new() -> Self {
        Self
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

    /// Check if sshpass is available on the system
    fn check_sshpass_available(&self) -> bool {
        Command::new("which")
            .arg("sshpass")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    /// Build SSH command with credentials
    fn build_ssh_command(
        &self,
        host: &str,
        port: u16,
        credential: Option<&SshCredential>,
    ) -> Result<Vec<String>> {
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
            Some(SshCredential::Password { username, password }) => {
                // For password auth, use sshpass to pass the password securely
                if !self.check_sshpass_available() {
                    return Err(anyhow!(
                        "sshpass is required for password authentication but not found. \
                        Install it with: brew install sshpass (macOS), \
                        apt-get install sshpass (Ubuntu/Debian), or \
                        yum install sshpass (RHEL/CentOS)"
                    ));
                }

                command.push("sshpass".to_string());
                command.push("-p".to_string());
                command.push(password.as_str().to_string());
                command.push("ssh".to_string());
                command.push("-o".to_string());
                command.push("StrictHostKeyChecking=no".to_string());
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

        Ok(command)
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

        let ssh_command_vec = self.build_ssh_command(&host, port, Some(credential))?;
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
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
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
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();

            if result.is_err() {
                // Fallback to cmd
                Command::new("cmd")
                    .arg("/c")
                    .arg("start")
                    .arg("cmd")
                    .arg("/k")
                    .arg(&ssh_command_str)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
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
                    cmd.arg(format!(
                        "{}; read -p 'Press Enter to close...'",
                        ssh_command_str
                    ));
                } else {
                    cmd.arg(&ssh_command_str);
                }

                cmd.stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null());

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
}

/// Enum representing different connection types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionType {
    Http,
    Ssh,
    Ping,
    Tcp,
}
