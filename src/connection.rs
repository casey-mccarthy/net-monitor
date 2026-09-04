//! Connection strategies for different monitor types.
//!
//! This module implements the Strategy pattern for connecting to monitored nodes.
//! HTTP/HTTPS targets open in the default web browser; Ping and TCP targets open
//! an SSH session in a terminal using the system's default SSH configuration.

use anyhow::{anyhow, Result};
use std::process::{Command, Stdio};
use tracing::{error, info};

/// Trait defining the connection strategy interface
pub trait ConnectionStrategy: Send + Sync {
    /// Connect to the target using the appropriate method
    fn connect(&self, target: &str) -> Result<()>;

    /// Get a description of this connection strategy
    #[allow(dead_code)]
    fn description(&self) -> &str;
}

/// HTTP connection strategy - opens URLs in the default web browser.
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

    /// Build the SSH command for the given host and port
    fn build_ssh_command(&self, host: &str, port: u16) -> Vec<String> {
        let mut command = vec!["ssh".to_string()];
        if port != 22 {
            command.push("-p".to_string());
            command.push(port.to_string());
        }
        command.push(host.to_string());
        command
    }
}

impl ConnectionStrategy for SshConnectionStrategy {
    fn connect(&self, target: &str) -> Result<()> {
        let (host, port) = self.parse_target(target);
        info!("Opening SSH connection to {}:{}", host, port);

        let ssh_command_str = self.build_ssh_command(&host, port).join(" ");

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

    fn description(&self) -> &str {
        "Open SSH connection in terminal"
    }
}

/// Enum representing different connection types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ConnectionType {
    Http,
    Ssh,
    Ping,
    Tcp,
}
