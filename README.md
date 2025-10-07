# Network Monitor

A robust network monitoring application built with Rust and egui for monitoring HTTP endpoints, network hosts, and SSH connections.

## Features

- **Multiple Connection Types**
  - HTTP/HTTPS endpoint monitoring with status code validation
  - ICMP ping for network host availability
  - SSH connection testing with key-based authentication
  
- **Secure Credential Management**
  - Encrypted storage for SSH keys and passwords
  - Support for multiple authentication methods
  - Secure credential association with monitored nodes

- **User Interface**
  - Clean, responsive GUI built with egui
  - Real-time status updates
  - Historical monitoring data visualization
  - System tray integration for background monitoring

- **Data Management**
  - SQLite database for persistent storage
  - Import/Export functionality (JSON, CSV)
  - Automatic data migration between versions
  - Configurable monitoring intervals

- **Cross-Platform Support**
  - Native binaries for Windows, macOS, and Linux
  - Consistent experience across platforms
  - Platform-specific optimizations

## Installation

### Download Pre-built Binaries

Download the latest release for your platform from the [Releases page](https://github.com/casey-mccarthy/net-monitor/releases):

- **Windows**: `net-monitor-windows-x64.exe`
- **macOS Intel**: `net-monitor-macos-x64`
- **macOS Apple Silicon**: `net-monitor-macos-arm64`
- **Linux**: `net-monitor-linux-x64`

### Build from Source

Prerequisites:
- Rust 1.70+ (latest stable recommended)
- Platform-specific dependencies (see below)

```bash
git clone https://github.com/casey-mccarthy/net-monitor.git
cd net-monitor
cargo build --release
./target/release/net-monitor
```

#### Platform-Specific Dependencies

**Linux:**
```bash
sudo apt-get install libgtk-3-dev libssl-dev
```

**macOS:**
No additional dependencies required.

**Windows:**
No additional dependencies required.

## Usage

### Getting Started

1. **Launch the application** - Double-click the executable or run from terminal
2. **Add a node to monitor**:
   - Click "Add Node" 
   - Enter node details (name, address/URL)
   - Select connection type (HTTP, Ping, or SSH)
   - Configure credentials if needed (for SSH)
3. **Start monitoring**:
   - Click "Check Now" for single check
   - Click "Start Monitoring" for continuous monitoring
   - View real-time status in the main window

### Connection Types

- **HTTP/HTTPS**: Monitor web endpoints, APIs, and services
- **Ping (ICMP)**: Check network host availability
- **SSH**: Test SSH server connectivity and authentication

### Import/Export

**Supported formats:**
- JSON: Full configuration with all settings
- CSV: Simplified format for bulk imports

**CSV Format:**
```csv
name,address,type
Web Server,https://example.com,http
Database,192.168.1.100,ping
SSH Server,10.0.0.5,ssh
```

### Data Storage

Application data is stored locally:
- **Windows**: `%LOCALAPPDATA%\net-monitor\`
- **macOS**: `~/Library/Application Support/net-monitor/`
- **Linux**: `~/.local/share/net-monitor/`

## Architecture

Built with modern Rust practices:
- **egui/eframe**: Native GUI framework
- **tokio**: Async runtime for concurrent monitoring
- **rusqlite**: Embedded database with migrations
- **reqwest**: HTTP/HTTPS client
- **ssh2**: SSH protocol support
- **ring**: Cryptographic operations for credential encryption

## Development

### Project Structure

```
src/
├── main.rs          # Application entry point
├── gui.rs           # User interface implementation
├── models.rs        # Data models and types
├── database.rs      # Database operations and migrations
├── monitor.rs       # Core monitoring logic
├── connection.rs    # Connection strategies (HTTP, Ping, SSH)
└── credentials.rs   # Secure credential management
```

### Building and Testing

```bash
# Development build with debug symbols
cargo build

# Run tests
cargo test

# Run with debug logging
RUST_LOG=debug cargo run

# Production build with optimizations
cargo build --release
```

## Contributing

Please see [CONTRIBUTING.md](CONTRIBUTING.md) for development workflow and guidelines.

### Claude Commands

This project includes custom Claude Code commands to streamline development workflows:

#### Core Workflow Commands

- **`/quick-pr`** - Complete PR workflow
  - Creates feature branch
  - Guides through commits
  - Runs pre-push quality checks (format, lint, tests)
  - Rebases on main
  - Creates pull request with changelog

- **`/pre-commit-checks`** - Run all quality checks
  - Check only (default): Reports issues without changes
  - `--fix`: Auto-fixes formatting and lint issues
  - `--fix-commit`: Fixes and creates conventional commit
  - Includes: format, build, clippy, tests, security audit

#### Standalone Utility Commands

- **`/create-feature-branch`** - Create and switch to feature branch
- **`/commit-feature`** - Create conventional commit
- **`/sync-main`** - Sync feature branch with main (rebase)
- **`/release`** - Trigger release with automated versioning

#### Quick Reference

```bash
# Start working on a new feature (all-in-one)
/quick-pr feature-name

# Just run quality checks before committing
/pre-commit-checks

# Run checks and auto-fix issues
/pre-commit-checks --fix

# Run checks, fix, and commit
/pre-commit-checks --fix-commit

# Create a feature branch only
/create-feature-branch

# Sync your branch with latest main
/sync-main

# Create a conventional commit
/commit-feature
```

See [CLAUDE.md](CLAUDE.md) for detailed workflow documentation.

## License

MIT License - see [LICENSE](LICENSE) file for details.

## Support

- **Issues**: [GitHub Issues](https://github.com/casey-mccarthy/net-monitor/issues)
- **Documentation**: See `.claude/` folder for detailed documentation
- **Releases**: [GitHub Releases](https://github.com/casey-mccarthy/net-monitor/releases)

