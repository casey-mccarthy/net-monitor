# Network Monitor

A simple network monitoring application written in Rust with a GUI interface. Monitor network nodes using HTTP GET requests, ping, or SNMP protocols.

## Features

- **Multiple Monitoring Types**: Support for HTTP GET, ICMP ping, and SNMP monitoring
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

### Building from Source

1. Clone the repository:
```bash
git clone https://github.com/yourusername/net-monitor.git
cd net-monitor
```

2. Build the application:
```bash
cargo build --release
```

3. Run the application:
```bash
cargo run --release
```

## Usage

### Adding Nodes

1. Click "Add Node" to open the node creation dialog
2. Enter the node details:
   - **Name**: A descriptive name for the node
   - **IP Address**: IP address or hostname to monitor
   - **Monitor Type**: Choose between HTTP GET, Ping, or SNMP

### Monitor Type Configuration

#### HTTP GET
- **HTTP Path**: URL path to check (default: "/")
- **Expected Status Code**: Expected HTTP response code (default: 200)

#### Ping
- **Ping Count**: Number of ping packets to send (default: 1)
- **Ping Timeout**: Timeout in seconds (default: 5)

#### SNMP
- **SNMP Community**: Community string (default: "public")
- **SNMP OID**: Object identifier to query (default: "1.3.6.1.2.1.1.1.0")

### Monitoring

- **Check Now**: Perform a one-time check of all nodes
- **Start Monitoring**: Begin continuous monitoring (checks every 30 seconds)
- **Stop Monitoring**: Stop continuous monitoring

### Import/Export

#### Import Nodes
- **JSON Format**: Import nodes from a JSON file
- **CSV Format**: Import nodes from a CSV file with format: `name,ip,type`

#### Export Nodes
- Export all nodes to a JSON file for backup or sharing

### Node Management

- **Edit**: Modify existing node configuration
- **Delete**: Remove nodes from monitoring
- **Status Display**: View current status, last check time, and response times

## File Formats

### JSON Import Format
```json
[
  {
    "name": "Web Server",
    "ip": "192.168.1.100",
    "monitor_type": "HttpGet",
    "config": {
      "http_path": "/health",
      "http_expected_status": 200
    }
  },
  {
    "name": "Router",
    "ip": "192.168.1.1",
    "monitor_type": "Ping",
    "config": {
      "ping_count": 3,
      "ping_timeout": 5
    }
  }
]
```

### CSV Import Format
```
Web Server,192.168.1.100,http_get
Router,192.168.1.1,ping
Switch,192.168.1.10,snmp
```

## Data Storage

The application stores all data locally in a SQLite database located at:
- **Windows**: `%LOCALAPPDATA%\net-monitor\network_monitor.db`
- **macOS**: `~/Library/Application Support/net-monitor/network_monitor.db`
- **Linux**: `~/.local/share/net-monitor/network_monitor.db`

### Database Schema

#### Nodes Table
- `id`: Primary key
- `name`: Node name
- `ip`: IP address or hostname
- `monitor_type`: Type of monitoring (http_get, ping, snmp)
- `status`: Current status (online, offline, unknown)
- `last_check`: Timestamp of last check
- `response_time`: Response time in milliseconds
- Configuration fields for each monitor type

#### Monitoring Results Table
- `id`: Primary key
- `node_id`: Foreign key to nodes table
- `timestamp`: When the check was performed
- `status`: Status at check time
- `response_time`: Response time in milliseconds
- `details`: Additional details about the check

## Dependencies

- **egui**: GUI framework
- **eframe**: Native window framework
- **rusqlite**: SQLite database
- **reqwest**: HTTP client
- **ping**: ICMP ping functionality
- **chrono**: Date and time handling
- **serde**: Serialization
- **anyhow**: Error handling

## Development

### Project Structure

```
src/
├── main.rs          # Application entry point
├── models.rs        # Data structures and types
├── database.rs      # SQLite database operations
├── monitor.rs       # Monitoring implementations
└── gui.rs          # GUI interface
```

### Adding New Monitor Types

1. Add the new monitor type to the `MonitorType` enum in `models.rs`
2. Implement the `Monitor` trait for the new type in `monitor.rs`
3. Add the monitor to the `MonitorFactory` in `monitor.rs`
4. Update the GUI to handle the new monitor type

### Building for Distribution

```bash
# Windows
cargo build --release --target x86_64-pc-windows-msvc

# macOS
cargo build --release --target x86_64-apple-darwin

# Linux
cargo build --release --target x86_64-unknown-linux-gnu
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

### Common Issues

1. **Permission Denied for Ping**: On some systems, ping requires elevated privileges
2. **SNMP Not Working**: Ensure SNMP is properly configured on target devices
3. **Database Locked**: Close other instances of the application

### Logs

The application uses the `tracing` crate for logging. Enable debug logging by setting the `RUST_LOG` environment variable:

```bash
RUST_LOG=debug cargo run
```

## Roadmap

- [ ] Email/SMS notifications
- [ ] Web dashboard
- [ ] Advanced SNMP monitoring
- [ ] Custom monitoring scripts
- [ ] Performance graphs
- [ ] Alert thresholds
- [ ] Multi-user support

