# Net-Monitor Architecture Overview

## System Architecture

Net-Monitor follows a modular architecture with clear separation of concerns:

```
┌─────────────────────────────────────────┐
│            GUI Layer (egui)             │
│  - User Interface                       │
│  - Event Handling                       │
│  - Real-time Updates                    │
└─────────────────────────────────────────┘
                    │
┌─────────────────────────────────────────┐
│         Application Core                │
│  - State Management                     │
│  - Business Logic                       │
│  - Monitoring Orchestration             │
└─────────────────────────────────────────┘
                    │
     ┌──────────────┼──────────────┐
     │              │              │
┌─────────┐ ┌──────────┐ ┌──────────────┐
│Database │ │Monitoring│ │  Connection  │
│  Layer  │ │  Engine  │ │  Strategies  │
└─────────┘ └──────────┘ └──────────────┘
     │              │              │
┌─────────────────────────────────────────┐
│         External Services               │
│  - HTTP/HTTPS Endpoints                 │
│  - Network Hosts (ICMP)                 │
│  - SSH Servers                          │
└─────────────────────────────────────────┘
```

## Component Responsibilities

### GUI Layer (`gui.rs`)
- Renders the user interface using egui
- Handles user input and events
- Displays real-time monitoring status
- Manages import/export operations

### Application Core (`main.rs`)
- Initializes and coordinates all components
- Manages application lifecycle
- Handles configuration and settings
- Orchestrates background monitoring tasks

### Database Layer (`database.rs`)
- SQLite database management
- Schema migrations
- CRUD operations for nodes and monitoring data
- Query optimization and indexing

### Monitoring Engine (`monitor.rs`)
- Executes monitoring checks
- Manages monitoring schedules
- Collects and processes results
- Handles retry logic and error recovery

### Connection Strategies (`connection.rs`)
- Strategy pattern for different connection types
- HTTP/HTTPS connection implementation
- ICMP ping implementation
- SSH connection implementation
- Extensible for future connection types

### Credential Management (`credentials.rs`)
- Secure storage of authentication data
- Encryption/decryption using ring
- Key management
- Credential association with nodes

## Data Flow

1. **User Interaction** → GUI receives input
2. **Command Processing** → Application core processes request
3. **Data Operation** → Database read/write as needed
4. **Monitoring Execution** → Connection strategy executes check
5. **Result Processing** → Monitor engine processes results
6. **State Update** → Database stores results
7. **UI Update** → GUI reflects new state

## Concurrency Model

- **Main Thread**: GUI rendering and user interaction
- **Tokio Runtime**: Async monitoring operations
- **Background Tasks**: Periodic monitoring checks
- **Database Access**: Thread-safe connection pooling

## Security Considerations

### Credential Storage
- Credentials encrypted at rest using AES-256-GCM
- Keys derived using PBKDF2
- No plaintext storage of sensitive data

### Network Security
- TLS/SSL for HTTPS connections
- SSH key validation
- No credential logging

### Data Protection
- Local-only data storage
- No cloud synchronization by default
- User-controlled export operations

## Extension Points

### Adding New Connection Types
1. Implement `ConnectionStrategy` trait
2. Add variant to `ConnectionType` enum
3. Update GUI to support configuration
4. Add database migration if needed

### Adding New Features
1. Extend models in `models.rs`
2. Add database migrations
3. Implement business logic
4. Update GUI components

## Performance Characteristics

- **Startup Time**: < 500ms typical
- **Memory Usage**: ~50MB baseline
- **Monitoring Overhead**: < 1% CPU per check
- **Database Size**: ~1MB per 10,000 records
- **Concurrent Checks**: Up to 100 simultaneous

## Technology Stack

- **Language**: Rust 2021 Edition
- **GUI Framework**: egui/eframe
- **Async Runtime**: Tokio
- **Database**: SQLite with rusqlite
- **HTTP Client**: reqwest
- **SSH Library**: ssh2
- **Cryptography**: ring