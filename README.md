# Network Monitor

A robust network monitoring application built with Rust for monitoring HTTP endpoints, network hosts, and TCP connections. Features a terminal user interface (TUI).

## Features

- **Terminal User Interface (TUI)**
  - Clean, responsive terminal interface built with ratatui

- **Multiple Connection Types**
  - HTTP/HTTPS endpoint monitoring with status code validation
  - TCP port connectivity checking
  - ICMP ping for network host availability

- **Data Management**
  - SQLite database for persistent storage
  - Import/Export functionality (JSON, CSV)
  - Automatic data migration between versions
  - Configurable monitoring intervals
  - Historical monitoring data with status change tracking

- **Cross-Platform Support**
  - Native binaries for Windows, macOS, and Linux
  - Consistent experience across platforms

## Installation

### Download Pre-built Binaries

Download the latest release for your platform from the [Releases page](https://github.com/casey-mccarthy/net-monitor/releases):

- **Windows**: `net-monitor-windows-x64.exe`
- **macOS Intel**: `net-monitor-macos-x64`
- **macOS Apple Silicon**: `net-monitor-macos-arm64`
- **Linux**: `net-monitor-linux-x64`

### Build from Source

**Prerequisites:**
- Rust 1.70+ (latest stable recommended)
- Linux only: `libssl-dev`

**Quick start:**
```bash
git clone https://github.com/casey-mccarthy/net-monitor.git
cd net-monitor
cargo build --release
./target/release/net-monitor
```

For detailed build commands and development workflows, see [CLAUDE.md](CLAUDE.md).

## Usage

### Basic Usage

```bash
net-monitor
```

1. Launch the application
2. Use arrow keys and Enter to navigate
3. Press 'a' to add a new node, 's' to start monitoring
4. Press 'q' to quit

### Connection Types

- **HTTP/HTTPS**: Monitor web endpoints, APIs, and services (e.g., `https://example.com`)
- **TCP**: Check TCP port connectivity (e.g., `192.168.1.100:3306` for MySQL)
- **Ping (ICMP)**: Check network host availability (e.g., `192.168.1.1`)

### Import/Export

Import and export your monitoring configuration as JSON files.

**Format:**
```json
[
  {
    "name": "Google",
    "monitoring_interval": 10,
    "detail": {
      "type": "Http",
      "url": "https://www.google.com",
      "expected_status": 200
    },
    "credential_id": null
  },
  {
    "name": "MySQL Server",
    "monitoring_interval": 30,
    "detail": {
      "type": "Tcp",
      "host": "192.168.1.100",
      "port": 3306,
      "timeout": 5
    },
    "credential_id": null
  },
  {
    "name": "Gateway",
    "monitoring_interval": 5,
    "detail": {
      "type": "Ping",
      "host": "192.168.1.1",
      "count": 3,
      "timeout": 5
    },
    "credential_id": null
  }
]
```

**Usage:**
- Press 'i' to import or 'e' to export, then enter file path
- Files must use `.json` extension
- Export creates pretty-printed JSON for easy editing

See [sample_nodes.json](sample_nodes.json) for more examples.

### Data Storage

Application data is stored locally:
- **Windows**: `%LOCALAPPDATA%\net-monitor\`
- **macOS**: `~/Library/Application Support/net-monitor/`
- **Linux**: `~/.local/share/net-monitor/`

## Development

### Quick Start

```bash
# Development build
cargo build

# Run tests
cargo test

# Run with debug logging
RUST_LOG=debug cargo run

# Production build
cargo build --release
```

For comprehensive development workflows, build commands, and testing guidelines, see [CLAUDE.md](CLAUDE.md).

## Contributing

We welcome contributions! Here's how to get started:

1. **Report bugs**: Use [GitHub Issues](https://github.com/casey-mccarthy/net-monitor/issues) with our bug report template
2. **Suggest features**: Open an issue with the feature request template
3. **Submit code**: Fork the repository, create a feature branch, and submit a pull request

Please see [CONTRIBUTING.md](CONTRIBUTING.md) for detailed guidelines on:
- Development workflow and branch naming
- Commit message conventions
- Code style and testing requirements
- Pull request process

For contributors using Claude Code, see [CLAUDE.md](CLAUDE.md) for automated workflow commands.

## Support

- **Bug Reports**: Use our [bug report template](https://github.com/casey-mccarthy/net-monitor/issues/new?template=bug_report.yml)
- **Feature Requests**: Use our [feature request template](https://github.com/casey-mccarthy/net-monitor/issues/new?template=feature_request.yml)
- **Releases**: [GitHub Releases](https://github.com/casey-mccarthy/net-monitor/releases)

## License

MIT License - see [LICENSE](LICENSE) file for details.

