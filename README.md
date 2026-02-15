# Net Monitor

A terminal-based network monitoring tool built with Rust. Monitors HTTP endpoints, TCP ports, and ICMP ping targets through an interactive TUI.

## Features

- **HTTP/HTTPS monitoring** — validate endpoints by expected status code
- **TCP port checks** — verify connectivity to any host and port
- **ICMP ping** — monitor network host availability
- **Soft/hard state model** — reduces false positives by requiring consecutive failures before marking a node offline
- **Persistent storage** — SQLite database with automatic schema migrations
- **Import/Export** — JSON-based node configuration for portability
- **Credential management** — AES-256-GCM encrypted storage for SSH credentials
- **Cross-platform** — runs on Linux, macOS, and Windows

## Installation

### Pre-built Binaries

Download from the [Releases page](https://github.com/casey-mccarthy/net-monitor/releases):

| Platform | Binary |
|---|---|
| Linux x64 | `net-monitor-linux-x64` |
| macOS Intel | `net-monitor-macos-x64` |
| macOS Apple Silicon | `net-monitor-macos-arm64` |
| Windows x64 | `net-monitor-windows-x64.exe` |

### Build from Source

Requires Rust 1.70+ and `libssl-dev` on Linux.

```bash
git clone https://github.com/casey-mccarthy/net-monitor.git
cd net-monitor
cargo build --release
./target/release/net-monitor
```

## Usage

Launch the application:

```bash
net-monitor
```

### Keyboard Shortcuts

| Key | Action |
|---|---|
| `q` | Quit |
| `a` | Add node |
| `e` | Edit selected node |
| `d` | Delete selected node |
| `m` | Start/stop monitoring |
| `h` | View status history |
| `c` | Manage credentials |
| `r` | Reorder nodes |
| `i` | Import nodes from JSON |
| `x` | Export nodes to JSON |
| `Enter` | Connect to selected node |
| `?` | Help |
| `Up/Down` | Navigate |

### Monitor Types

**HTTP/HTTPS** — monitor web endpoints and APIs with expected status code validation.

**TCP** — check port connectivity on any host (e.g., database ports, service ports).

**Ping** — ICMP availability checks with configurable count and timeout.

### Node States

| State | Meaning |
|---|---|
| Online | Responding normally |
| Degraded | Failed check, not yet confirmed down (soft state) |
| Offline | Failed consecutive checks (hard state, default: 3 attempts) |

### Import/Export

Nodes can be imported and exported as JSON. See [sample_nodes.json](sample_nodes.json) for the format.

```json
[
  {
    "name": "GitHub",
    "monitoring_interval": 15,
    "detail": {
      "type": "Http",
      "url": "https://github.com",
      "expected_status": 200
    }
  }
]
```

### Data Storage

Data is stored locally in a SQLite database:

| Platform | Path |
|---|---|
| Linux | `~/.local/share/net-monitor/` |
| macOS | `~/Library/Application Support/net-monitor/` |
| Windows | `%LOCALAPPDATA%\net-monitor\` |

## Development

```bash
cargo build                # development build
cargo test                 # run tests (excludes network tests)
cargo fmt                  # format code
cargo clippy               # lint
cargo build --release      # release build
```

See [CLAUDE.md](CLAUDE.md) for the full development workflow and [CONTRIBUTING.md](CONTRIBUTING.md) for contribution guidelines.

## License

MIT OR Apache-2.0
