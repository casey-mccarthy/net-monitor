# Network Monitor

A simple network monitoring application written in Rust with a GUI interface. Monitor network nodes using HTTP GET requests and ping.

## Features

- **Multiple Monitoring Types**: Support for HTTP GET and ICMP ping
- **GUI Interface**: Clean and intuitive interface built with egui
- **Local Data Storage**: SQLite database for persistent storage of nodes and monitoring history
- **Import/Export**: Import nodes from JSON or CSV files, export to JSON
- **Real-time Monitoring**: Continuous monitoring with configurable intervals
- **Historical Data**: Track monitoring results over time
- **Cross-platform**: Works on Windows, macOS, and Linux

## Installation

### Prerequisites

- Rust (latest stable version)
- Cargo

### Running

```bash
git clone https://github.com/yourusername/net-monitor.git
cd net-monitor
cargo run --release
```

## Usage

### Basic Usage

1. **Add Node**: Click to add a new node with name, IP address, and monitor type (HTTP GET or Ping)
2. **Monitor**: Use "Check Now" for one-time checks or "Start Monitoring" for continuous monitoring
3. **Import/Export**: Import nodes from JSON/CSV or export for backup

## Import Formats

**JSON**: Full configuration with all settings  
**CSV**: Simple format - `name,ip,type` (e.g., `Web Server,192.168.1.100,http_get`)

## Data Storage

Data is stored locally in SQLite:
- **Windows**: `%LOCALAPPDATA%\net-monitor\network_monitor.db`
- **macOS**: `~/Library/Application Support/net-monitor/network_monitor.db`
- **Linux**: `~/.local/share/net-monitor/network_monitor.db`

## Key Dependencies

- **egui/eframe**: GUI framework
- **rusqlite**: SQLite database
- **reqwest**: HTTP client
- **ping**: ICMP ping functionality

## Development

### Project Structure

```
src/
├── main.rs          # Application entry point
├── models.rs        # Data structures
├── database.rs      # SQLite operations
├── monitor.rs       # Monitoring logic
└── gui.rs          # GUI interface
```

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

## Troubleshooting

- **Ping Permission Denied**: May require elevated privileges on some systems
- **Database Locked**: Close other instances of the application
- **Debug Logs**: Run with `RUST_LOG=debug cargo run`

## Roadmap

- [ ] Email notifications
- [ ] Web dashboard
- [ ] Custom monitoring scripts
- [ ] Performance graphs
- [ ] Alert thresholds

