# Database Schema Documentation

## Overview

Net-Monitor uses SQLite for local data persistence. The database is automatically created and migrated as needed.

## Current Schema (Version 3)

### Tables

#### `nodes`
Stores monitored nodes configuration.

| Column | Type | Constraints | Description |
|--------|------|------------|-------------|
| id | INTEGER | PRIMARY KEY | Unique identifier |
| name | TEXT | NOT NULL | Display name of the node |
| connection_type | TEXT | NOT NULL | Type: 'http', 'ping', or 'ssh' |
| connection_target | TEXT | NOT NULL | URL, IP address, or hostname |
| monitoring_enabled | BOOLEAN | NOT NULL DEFAULT 0 | Auto-monitoring flag |
| monitoring_interval | INTEGER | NOT NULL DEFAULT 60 | Check interval in seconds |
| created_at | TEXT | NOT NULL | ISO 8601 timestamp |
| updated_at | TEXT | NOT NULL | ISO 8601 timestamp |
| credential_id | INTEGER | REFERENCES credentials(id) | Optional credential reference |

#### `monitoring_results`
Stores historical monitoring data.

| Column | Type | Constraints | Description |
|--------|------|------------|-------------|
| id | INTEGER | PRIMARY KEY | Unique identifier |
| node_id | INTEGER | NOT NULL, REFERENCES nodes(id) | Associated node |
| status | TEXT | NOT NULL | 'up', 'down', or 'unknown' |
| response_time_ms | INTEGER | | Response time in milliseconds |
| error_message | TEXT | | Error details if check failed |
| checked_at | TEXT | NOT NULL | ISO 8601 timestamp |

**Indexes:**
- `idx_monitoring_results_node_id` on (node_id)
- `idx_monitoring_results_checked_at` on (checked_at)

#### `status_changes`
Stores node status transition events for analytics and historical tracking.

| Column | Type | Constraints | Description |
|--------|------|------------|-------------|
| id | INTEGER | PRIMARY KEY | Unique identifier |
| node_id | INTEGER | NOT NULL, REFERENCES nodes(id) ON DELETE CASCADE | Associated node |
| from_status | TEXT | NOT NULL | Previous status ('Online', 'Offline', 'Unknown') |
| to_status | TEXT | NOT NULL | New status ('Online', 'Offline', 'Unknown') |
| changed_at | TEXT | NOT NULL | ISO 8601 timestamp of status change |
| duration_ms | INTEGER | | Time spent in previous status (milliseconds) |

**Indexes:**
- `idx_status_changes_node_id` on (node_id)
- `idx_status_changes_changed_at` on (changed_at)

**Note:** This table differs from `monitoring_results` by only recording **status transitions** (when status actually changes), not every monitoring check. This enables efficient queries for outage tracking, uptime calculations, and status history analysis.

#### `credentials`
Stores encrypted authentication credentials.

| Column | Type | Constraints | Description |
|--------|------|------------|-------------|
| id | INTEGER | PRIMARY KEY | Unique identifier |
| name | TEXT | NOT NULL UNIQUE | Credential identifier |
| credential_type | TEXT | NOT NULL | 'password' or 'ssh_key' |
| encrypted_data | BLOB | NOT NULL | Encrypted credential data |
| salt | BLOB | NOT NULL | Encryption salt |
| nonce | BLOB | NOT NULL | Encryption nonce |
| created_at | TEXT | NOT NULL | ISO 8601 timestamp |
| updated_at | TEXT | NOT NULL | ISO 8601 timestamp |

#### `migrations`
Tracks applied database migrations.

| Column | Type | Constraints | Description |
|--------|------|------------|-------------|
| version | INTEGER | PRIMARY KEY | Migration version number |
| applied_at | TEXT | NOT NULL | ISO 8601 timestamp |

## Migration History

### Version 1 - Initial Schema
- Created `nodes` table with basic fields
- Created `monitoring_results` table
- Established foreign key relationships

### Version 2 - Add Credentials
- Added `credentials` table
- Added encryption fields for secure storage

### Version 3 - Link Nodes to Credentials
- Added `credential_id` column to `nodes` table
- Created foreign key reference to credentials

## Data Types Mapping

| Rust Type | SQLite Type | Notes |
|-----------|------------|-------|
| i32, i64 | INTEGER | Auto-incrementing for PRIMARY KEY |
| String | TEXT | UTF-8 encoded |
| bool | BOOLEAN | Stored as 0/1 |
| Vec<u8> | BLOB | Binary data |
| DateTime<Utc> | TEXT | ISO 8601 format |

## Query Patterns

### Common Queries

```sql
-- Get all nodes with their latest status
SELECT n.*, mr.status, mr.checked_at
FROM nodes n
LEFT JOIN monitoring_results mr ON n.id = mr.node_id
WHERE mr.id = (
    SELECT id FROM monitoring_results
    WHERE node_id = n.id
    ORDER BY checked_at DESC
    LIMIT 1
);

-- Get monitoring history for a node
SELECT * FROM monitoring_results
WHERE node_id = ?
ORDER BY checked_at DESC
LIMIT 100;

-- Get nodes needing monitoring
SELECT * FROM nodes
WHERE monitoring_enabled = 1
AND datetime('now') > datetime(
    (SELECT checked_at FROM monitoring_results
     WHERE node_id = nodes.id
     ORDER BY checked_at DESC LIMIT 1),
    '+' || monitoring_interval || ' seconds'
);

-- Get status change history for a node
SELECT * FROM status_changes
WHERE node_id = ?
ORDER BY changed_at DESC
LIMIT 50;

-- Get latest status change for a node
SELECT * FROM status_changes
WHERE node_id = ?
ORDER BY changed_at DESC
LIMIT 1;

-- Calculate uptime percentage over time period
SELECT
    node_id,
    SUM(CASE WHEN from_status = 'Online' THEN duration_ms ELSE 0 END) as online_ms,
    SUM(duration_ms) as total_ms,
    (SUM(CASE WHEN from_status = 'Online' THEN duration_ms ELSE 0 END) * 100.0 / SUM(duration_ms)) as uptime_pct
FROM status_changes
WHERE node_id = ?
    AND changed_at >= ?
    AND changed_at <= ?
GROUP BY node_id;

-- Get all outages (transitions to Offline)
SELECT * FROM status_changes
WHERE node_id = ? AND to_status = 'Offline'
ORDER BY changed_at DESC;

-- Get recovery times (time between Offline and Online)
SELECT
    sc1.changed_at as outage_start,
    sc2.changed_at as recovery_time,
    (julianday(sc2.changed_at) - julianday(sc1.changed_at)) * 86400000 as downtime_ms
FROM status_changes sc1
JOIN status_changes sc2 ON sc1.node_id = sc2.node_id
WHERE sc1.to_status = 'Offline'
    AND sc2.from_status = 'Offline'
    AND sc2.to_status = 'Online'
    AND sc2.changed_at > sc1.changed_at
ORDER BY sc1.changed_at DESC;
```

## Database Maintenance

### Size Management
- Old monitoring results are automatically pruned after 30 days
- Indexes are automatically maintained by SQLite
- VACUUM is run monthly to reclaim space

### Backup Strategy
- Database file can be directly copied for backup
- Export functionality creates JSON backups
- Consider periodic automated backups

### Performance Optimization
- Indexes on frequently queried columns
- Prepared statements for repeated queries
- Connection pooling for concurrent access

## Future Considerations

### Potential Schema Changes
- Add `tags` table for node categorization
- Add `alerts` table for notification history
- Add `users` table for multi-user support
- Consider partitioning monitoring_results by date

### Migration Strategy
- All changes via numbered migrations
- Backward compatibility maintained
- Automatic migration on startup
- Rollback capability for safety