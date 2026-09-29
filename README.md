# Net Monitor

A terminal-based network monitor written in Rust. It watches HTTP endpoints, TCP ports, and ICMP ping targets, records every status change in SQLite, and lets you jump from a node straight into a browser tab or an SSH session.

## Features

- **HTTP/HTTPS** — request a URL and compare the status code to the one you expect. Self-signed certificates are accepted, so internal services work out of the box.
- **TCP** — open a connection to a host and port within a timeout.
- **Ping** — ICMP echo with a configurable count and timeout. Accepts hostnames as well as IP addresses, and works without root on Linux and macOS.
- **Soft/hard state model** — one failed check marks a node *Degraded*; only consecutive failures mark it *Offline*. Fewer false alarms.
- **Concurrent checks** — every node is checked on its own schedule, in parallel. A host that times out never delays the others.
- **Status history** — every status transition is stored with how long the previous state lasted, so the history view shows uptime and outage lengths.
- **Connect** — press Enter on a node to open it: HTTP nodes open in your browser, ping and TCP nodes open an SSH session in a new terminal window.
- **Import/Export** — node configuration as JSON.
- **Cross-platform** — Linux, macOS, and Windows.

## Installation

### Pre-built binaries

Every release on the [Releases page](https://github.com/casey-mccarthy/net-monitor/releases) ships one archive per platform:

| Platform | Archive |
|---|---|
| Linux x64 | `net-monitor-vX.Y.Z-linux-x64.tar.gz` |
| macOS Intel | `net-monitor-vX.Y.Z-macos-x64.tar.gz` |
| macOS Apple Silicon | `net-monitor-vX.Y.Z-macos-arm64.tar.gz` |
| Windows x64 | `net-monitor-vX.Y.Z-windows-x64.zip` |

Extract the archive and run the `net-monitor` binary inside it. SHA256 checksums for every archive are in `checksums.txt`.

### Build from source

You need a current stable Rust toolchain. On Linux you also need GTK 3 (native file dialogs) and the OpenSSL headers:

```bash
sudo apt-get install libgtk-3-dev libssl-dev pkg-config   # Debian/Ubuntu
```

```bash
git clone https://github.com/casey-mccarthy/net-monitor.git
cd net-monitor
cargo build --release
./target/release/net-monitor
```

## Usage

```bash
net-monitor
```

Monitoring starts as soon as the app launches. Press `?` in any view for context-sensitive help.

### Keys

| Key | Action |
|---|---|
| `↑` / `↓` | Select a node |
| `Enter` | Connect to the selected node (browser for HTTP, SSH for ping and TCP) |
| `m` | Start / stop monitoring |
| `a` | Add a node |
| `e` | Edit the selected node |
| `d` | Delete the selected node (asks for confirmation) |
| `h` | Status history for the selected node |
| `r` | Reorder nodes: `↑` / `↓` to move, `r` to save, `Esc` to cancel |
| `i` | Import nodes from a JSON file |
| `x` | Export nodes to a JSON file |
| `b` | About |
| `?` | Help |
| `q` | Quit |

In the node form, `Tab` and `Shift+Tab` move between fields, `←` / `→` or `Space` change the monitor type, `Enter` saves, and `Esc` cancels.

### Monitor types

Every node has a name, a monitoring interval in seconds, and one of these checks:

| Type | Fields | Online when |
|---|---|---|
| `Http` | `url`, `expected_status` | the response status equals `expected_status` |
| `Tcp` | `host`, `port`, `timeout` | a TCP connection succeeds within `timeout` seconds |
| `Ping` | `host`, `count`, `timeout` | any one of `count` echo requests is answered within `timeout` seconds |

A URL without a scheme is treated as `https://`.

### Node states

| State | Meaning |
|---|---|
| Online | The last check succeeded |
| Degraded | A check failed, but not enough in a row to call the node down (soft state). It is rechecked every `retry_interval` seconds, 15 by default. |
| Offline | `max_check_attempts` consecutive checks failed, 3 by default (hard state). |

One successful check returns a node to Online from either state. Every transition between these states is recorded in the status history.

### Import/Export

Files are a JSON array of nodes. `max_check_attempts` and `retry_interval` are optional and default to 3 and 15. [sample_nodes.json](sample_nodes.json) has an example of each type.

```json
[
  {
    "name": "GitHub",
    "monitoring_interval": 15,
    "detail": { "type": "Http", "url": "https://github.com", "expected_status": 200 }
  },
  {
    "name": "Postgres",
    "monitoring_interval": 30,
    "detail": { "type": "Tcp", "host": "db.internal", "port": 5432, "timeout": 5 }
  },
  {
    "name": "Router",
    "monitoring_interval": 5,
    "detail": { "type": "Ping", "host": "192.168.1.1", "count": 3, "timeout": 5 }
  }
]
```

Importing offers two modes. **Import & Skip Conflicts** keeps your existing nodes and skips any imported node whose name already exists. **Clear & Import All** replaces every node with the file's contents. The file is validated before anything is deleted.

### Data storage

The SQLite database (`network_monitor.db`) and the log file (`net-monitor.log`) live in the platform data directory:

| Platform | Path |
|---|---|
| Linux | `~/.local/share/net-monitor/` |
| macOS | `~/Library/Application Support/com.casey.net-monitor/` |
| Windows | `%LOCALAPPDATA%\casey\net-monitor\data\` |

The schema is migrated automatically on startup. Set `RUST_LOG=debug` for verbose logging.

## Development

```bash
cargo build
RUSTFLAGS="-A dead_code" cargo test
cargo fmt
RUSTFLAGS="-A dead_code" cargo clippy --all-targets --all-features -- -D warnings
```

Tests that reach the network are behind the `network-tests` feature and stay off in CI: `cargo test --features network-tests`.

[CLAUDE.md](CLAUDE.md) has the full workflow, [CONTRIBUTING.md](CONTRIBUTING.md) the contribution guidelines, and [tests/README.md](tests/README.md) the test layout.

## Releases

Every merge to `main` with a conventional commit triggers the release workflow. It bumps the version in `Cargo.toml`, tags the commit, builds the four archives, and publishes a GitHub release. The release notes are generated from the commits since the previous tag by `scripts/release-notes.sh`, grouped into breaking changes, features, fixes, performance, documentation, and other changes. Preview the notes for the next release locally:

```bash
scripts/release-notes.sh 1.5.0 v1.4.10
```

## License

MIT OR Apache-2.0
