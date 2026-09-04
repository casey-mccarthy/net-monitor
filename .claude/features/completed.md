# Completed Features

This document tracks features that have been successfully implemented and released.

## Version 0.3.0 (Latest)

### SSH Connection Support
- **Released**: 2024-08-30
- **Description**: Added SSH connection support using the system's default SSH configuration
- Integration with existing monitoring framework
- The encrypted credential store that originally shipped alongside this
  feature has since been removed; SSH sessions rely on the user's own SSH
  agent, keys, and config.

## Version 0.2.0

### GUI Improvements
- **Released**: 2024-08-30
- **Description**: Enhanced user interface
- About dialog with version information
- Improved window sizing and layout
- Better error messaging

### Database Migrations
- **Released**: 2024-08-30
- **Description**: Automatic database schema updates
- Seamless upgrades between versions
- Data integrity preservation
- Rollback capability

## Version 0.1.0 (Initial Release)

### Core Monitoring
- **Released**: 2024-08-01
- **Description**: Basic monitoring functionality
- HTTP/HTTPS endpoint monitoring
- ICMP ping support
- Real-time status updates

### TUI Application
- **Released**: 2024-08-01
- **Description**: Terminal-based application
- Cross-platform support (Windows, macOS, Linux)
- Clean, intuitive terminal interface

### Data Persistence
- **Released**: 2024-08-01
- **Description**: Local data storage
- SQLite database integration
- Historical monitoring data
- Configuration persistence

### Import/Export
- **Released**: 2024-08-01
- **Description**: Data portability
- JSON format support
- CSV import capability
- Bulk node management

## Feature Metrics

- **Total Features Shipped**: 12
- **Average Development Time**: 2 weeks
- **Most Requested**: SSH support (implemented ✓)
- **Next Priority**: Email notifications (see planned.md)