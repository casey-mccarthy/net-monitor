use crate::models::{MonitorDetail, MonitoringResult, Node, NodeStatus, StatusChange};
use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, Row};
use std::path::{Path, PathBuf};
use tracing::info;

/// Database manager for handling SQLite operations
#[derive(Clone)]
pub struct Database {
    path: PathBuf,
}

impl Database {
    /// Creates a new database connection and initializes tables
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path_buf = path.as_ref().to_path_buf();
        let db = Self { path: path_buf };
        db.init_tables()?;
        Ok(db)
    }

    /// Creates a new connection for this database
    fn get_connection(&self) -> Result<Connection> {
        Ok(Connection::open(&self.path)?)
    }

    /// Initializes the database tables if they don't exist
    fn init_tables(&self) -> Result<()> {
        let conn = self.get_connection()?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS nodes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                monitor_type TEXT NOT NULL,
                status TEXT NOT NULL,
                last_check TEXT,
                response_time INTEGER,
                monitoring_interval INTEGER NOT NULL DEFAULT 5,
                credential_id TEXT,
                http_url TEXT,
                http_expected_status INTEGER,
                ping_host TEXT,
                ping_count INTEGER,
                ping_timeout INTEGER,
                tcp_host TEXT,
                tcp_port INTEGER,
                tcp_timeout INTEGER
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS monitoring_results (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                node_id INTEGER NOT NULL,
                timestamp TEXT NOT NULL,
                status TEXT NOT NULL,
                response_time INTEGER,
                details TEXT,
                FOREIGN KEY (node_id) REFERENCES nodes (id) ON DELETE CASCADE
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS status_changes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                node_id INTEGER NOT NULL,
                from_status TEXT NOT NULL,
                to_status TEXT NOT NULL,
                changed_at TEXT NOT NULL,
                duration_ms INTEGER,
                FOREIGN KEY (node_id) REFERENCES nodes (id) ON DELETE CASCADE
            )",
            [],
        )?;

        // Create indexes for efficient queries
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_status_changes_node_id ON status_changes(node_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_status_changes_changed_at ON status_changes(changed_at)",
            [],
        )?;

        // Add credential_id column to existing nodes table if it doesn't exist
        self.migrate_credential_column(&conn)?;

        // Add TCP columns to existing nodes table if they don't exist
        self.migrate_tcp_columns(&conn)?;

        // Migrate Unknown status to Offline
        self.migrate_unknown_status(&conn)?;

        // Add display_order column for custom node ordering
        self.migrate_display_order_column(&conn)?;

        Ok(())
    }

    /// Migrate to add credential_id column if it doesn't exist
    fn migrate_credential_column(&self, conn: &Connection) -> Result<()> {
        // Check if credential_id column already exists
        let mut stmt = conn.prepare("PRAGMA table_info(nodes)")?;
        let column_exists = stmt
            .query_map([], |row| {
                let column_name: String = row.get(1)?;
                Ok(column_name)
            })?
            .any(|name| name.unwrap_or_default() == "credential_id");

        if !column_exists {
            conn.execute("ALTER TABLE nodes ADD COLUMN credential_id TEXT", [])?;
            info!("Added credential_id column to nodes table");
        }

        Ok(())
    }

    /// Migrate to add TCP columns if they don't exist
    fn migrate_tcp_columns(&self, conn: &Connection) -> Result<()> {
        let mut stmt = conn.prepare("PRAGMA table_info(nodes)")?;
        let existing_columns: Vec<String> = stmt
            .query_map([], |row| {
                let column_name: String = row.get(1)?;
                Ok(column_name)
            })?
            .filter_map(|name| name.ok())
            .collect();

        // Add tcp_host column if it doesn't exist
        if !existing_columns.contains(&"tcp_host".to_string()) {
            conn.execute("ALTER TABLE nodes ADD COLUMN tcp_host TEXT", [])?;
            info!("Added tcp_host column to nodes table");
        }

        // Add tcp_port column if it doesn't exist
        if !existing_columns.contains(&"tcp_port".to_string()) {
            conn.execute("ALTER TABLE nodes ADD COLUMN tcp_port INTEGER", [])?;
            info!("Added tcp_port column to nodes table");
        }

        // Add tcp_timeout column if it doesn't exist
        if !existing_columns.contains(&"tcp_timeout".to_string()) {
            conn.execute("ALTER TABLE nodes ADD COLUMN tcp_timeout INTEGER", [])?;
            info!("Added tcp_timeout column to nodes table");
        }

        Ok(())
    }

    /// Migrate Unknown status to Offline in existing data
    fn migrate_unknown_status(&self, conn: &Connection) -> Result<()> {
        // Update nodes table
        let nodes_updated = conn.execute(
            "UPDATE nodes SET status = 'Offline' WHERE status = 'Unknown'",
            [],
        )?;
        if nodes_updated > 0 {
            info!(
                "Migrated {} node(s) from Unknown to Offline status",
                nodes_updated
            );
        }

        // Update monitoring_results table
        let results_updated = conn.execute(
            "UPDATE monitoring_results SET status = 'Offline' WHERE status = 'Unknown'",
            [],
        )?;
        if results_updated > 0 {
            info!(
                "Migrated {} monitoring result(s) from Unknown to Offline status",
                results_updated
            );
        }

        // Update status_changes table
        let from_updated = conn.execute(
            "UPDATE status_changes SET from_status = 'Offline' WHERE from_status = 'Unknown'",
            [],
        )?;
        let to_updated = conn.execute(
            "UPDATE status_changes SET to_status = 'Offline' WHERE to_status = 'Unknown'",
            [],
        )?;
        if from_updated > 0 || to_updated > 0 {
            info!(
                "Migrated {} status change(s) from Unknown to Offline",
                from_updated + to_updated
            );
        }

        Ok(())
    }

    /// Migrate to add display_order column if it doesn't exist
    fn migrate_display_order_column(&self, conn: &Connection) -> Result<()> {
        let mut stmt = conn.prepare("PRAGMA table_info(nodes)")?;
        let column_exists = stmt
            .query_map([], |row| {
                let column_name: String = row.get(1)?;
                Ok(column_name)
            })?
            .any(|name| name.unwrap_or_default() == "display_order");

        if !column_exists {
            conn.execute("ALTER TABLE nodes ADD COLUMN display_order INTEGER", [])?;
            // Backfill existing rows with alphabetical order
            conn.execute(
                "UPDATE nodes SET display_order = (
                    SELECT COUNT(*) FROM nodes AS n2 WHERE n2.name < nodes.name
                )",
                [],
            )?;
            info!("Added display_order column to nodes table");
        }

        Ok(())
    }

    /// Adds a new node to the database
    pub fn add_node(&self, node: &Node) -> Result<i64> {
        // Validate: HTTP nodes cannot have credentials (SSH-only feature)
        if matches!(node.detail, crate::models::MonitorDetail::Http { .. })
            && node.credential_id.is_some()
        {
            return Err(anyhow::anyhow!(
                "HTTP/HTTPS targets do not support credentials. Credentials are only supported for SSH-based connections (Ping, TCP)."
            ));
        }

        let conn = self.get_connection()?;
        let (
            monitor_type,
            http_url,
            http_expected_status,
            ping_host,
            ping_count,
            ping_timeout,
            tcp_host,
            tcp_port,
            tcp_timeout,
        ) = node.detail.to_db_params();

        let status_str = node.status.to_string();

        conn.execute(
            "INSERT INTO nodes (
                name, monitor_type, status, last_check, response_time, monitoring_interval,
                credential_id, http_url, http_expected_status, ping_host, ping_count, ping_timeout,
                tcp_host, tcp_port, tcp_timeout, display_order
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
                (SELECT COALESCE(MAX(display_order), -1) + 1 FROM nodes))",
            params![
                node.name,
                monitor_type,
                status_str,
                node.last_check.map(|dt| dt.to_rfc3339()),
                node.response_time,
                node.monitoring_interval,
                node.credential_id,
                http_url,
                http_expected_status,
                ping_host,
                ping_count,
                ping_timeout,
                tcp_host,
                tcp_port,
                tcp_timeout,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Updates an existing node in the database
    pub fn update_node(&self, node: &Node) -> Result<()> {
        // Validate: HTTP nodes cannot have credentials (SSH-only feature)
        if matches!(node.detail, crate::models::MonitorDetail::Http { .. })
            && node.credential_id.is_some()
        {
            return Err(anyhow::anyhow!(
                "HTTP/HTTPS targets do not support credentials. Credentials are only supported for SSH-based connections (Ping, TCP)."
            ));
        }

        let conn = self.get_connection()?;
        let (
            monitor_type,
            http_url,
            http_expected_status,
            ping_host,
            ping_count,
            ping_timeout,
            tcp_host,
            tcp_port,
            tcp_timeout,
        ) = node.detail.to_db_params();

        let status_str = node.status.to_string();

        conn.execute(
            "UPDATE nodes SET
                name = ?1, monitor_type = ?2, status = ?3, last_check = ?4, response_time = ?5,
                monitoring_interval = ?6, credential_id = ?7, http_url = ?8, http_expected_status = ?9,
                ping_host = ?10, ping_count = ?11, ping_timeout = ?12,
                tcp_host = ?13, tcp_port = ?14, tcp_timeout = ?15
            WHERE id = ?16",
            params![
                node.name,
                monitor_type,
                status_str,
                node.last_check.map(|dt| dt.to_rfc3339()),
                node.response_time,
                node.monitoring_interval,
                node.credential_id,
                http_url,
                http_expected_status,
                ping_host,
                ping_count,
                ping_timeout,
                tcp_host,
                tcp_port,
                tcp_timeout,
                node.id,
            ],
        )?;
        Ok(())
    }

    /// Retrieves all nodes from the database
    pub fn get_all_nodes(&self) -> Result<Vec<Node>> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, monitor_type, status, last_check, response_time, monitoring_interval,
                    credential_id, http_url, http_expected_status, ping_host, ping_count, ping_timeout,
                    tcp_host, tcp_port, tcp_timeout
             FROM nodes ORDER BY display_order, name",
        )?;
        let nodes = stmt.query_map([], |row| self.row_to_node(row))?;
        nodes
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// Updates display_order for multiple nodes atomically
    pub fn update_node_display_orders(&self, order: &[(i64, i64)]) -> Result<()> {
        let conn = self.get_connection()?;
        let tx = conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare("UPDATE nodes SET display_order = ?1 WHERE id = ?2")?;
            for &(node_id, new_order) in order {
                stmt.execute(params![new_order, node_id])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Deletes a node from the database
    pub fn delete_node(&self, id: i64) -> Result<()> {
        let conn = self.get_connection()?;
        conn.execute("DELETE FROM nodes WHERE id = ?", [id])?;
        Ok(())
    }

    /// Adds a monitoring result to the database
    pub fn add_monitoring_result(&self, result: &MonitoringResult) -> Result<i64> {
        let conn = self.get_connection()?;
        let status_str = result.status.to_string();
        conn.execute(
            "INSERT INTO monitoring_results (node_id, timestamp, status, response_time, details)
             VALUES (?, ?, ?, ?, ?)",
            params![
                result.node_id,
                result.timestamp.to_rfc3339(),
                status_str,
                result.response_time,
                result.details,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Gets the most recent monitoring result for a node
    pub fn get_latest_monitoring_result(&self, node_id: i64) -> Result<Option<MonitoringResult>> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, node_id, timestamp, status, response_time, details
             FROM monitoring_results
             WHERE node_id = ?
             ORDER BY timestamp DESC
             LIMIT 1",
        )?;

        let mut results = stmt.query_map([node_id], |row| {
            let status_str: String = row.get("status")?;
            let timestamp_str: String = row.get("timestamp")?;

            let timestamp = DateTime::parse_from_rfc3339(&timestamp_str)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|_| rusqlite::Error::InvalidQuery)?;

            Ok(MonitoringResult {
                id: row.get("id")?,
                node_id: row.get("node_id")?,
                timestamp,
                status: status_str.parse().unwrap_or(NodeStatus::Offline),
                response_time: row.get("response_time")?,
                details: row.get("details")?,
            })
        })?;

        match results.next() {
            Some(Ok(result)) => Ok(Some(result)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    /// Adds a status change event to the database
    pub fn add_status_change(&self, change: &StatusChange) -> Result<i64> {
        let conn = self.get_connection()?;
        conn.execute(
            "INSERT INTO status_changes (node_id, from_status, to_status, changed_at, duration_ms)
             VALUES (?, ?, ?, ?, ?)",
            params![
                change.node_id,
                change.from_status.to_string(),
                change.to_status.to_string(),
                change.changed_at.to_rfc3339(),
                change.duration_ms,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Retrieves status changes for a node, ordered by most recent first
    pub fn get_status_changes(
        &self,
        node_id: i64,
        limit: Option<usize>,
    ) -> Result<Vec<StatusChange>> {
        let conn = self.get_connection()?;
        let query = if let Some(limit) = limit {
            format!(
                "SELECT id, node_id, from_status, to_status, changed_at, duration_ms
                 FROM status_changes
                 WHERE node_id = ?
                 ORDER BY changed_at DESC
                 LIMIT {}",
                limit
            )
        } else {
            "SELECT id, node_id, from_status, to_status, changed_at, duration_ms
             FROM status_changes
             WHERE node_id = ?
             ORDER BY changed_at DESC"
                .to_string()
        };

        let mut stmt = conn.prepare(&query)?;
        let changes = stmt.query_map([node_id], |row| self.row_to_status_change(row))?;
        changes
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// Gets the most recent status change for a node
    pub fn get_latest_status_change(&self, node_id: i64) -> Result<Option<StatusChange>> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, node_id, from_status, to_status, changed_at, duration_ms
             FROM status_changes
             WHERE node_id = ?
             ORDER BY changed_at DESC
             LIMIT 1",
        )?;

        let mut changes = stmt.query_map([node_id], |row| self.row_to_status_change(row))?;
        match changes.next() {
            Some(Ok(change)) => Ok(Some(change)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    /// Calculate how long the node has been in its current status
    pub fn get_current_status_duration(&self, node_id: i64) -> Result<Option<i64>> {
        if let Some(latest_change) = self.get_latest_status_change(node_id)? {
            let duration_ms =
                StatusChange::calculate_duration(latest_change.changed_at, Utc::now());
            Ok(Some(duration_ms))
        } else {
            Ok(None)
        }
    }

    /// Gets the status of a node at a specific point in time
    /// by finding the most recent status change before that time
    ///
    /// Returns None if there are no status changes before the given time
    /// (in which case the node should be assumed to be in its default/current state)
    #[allow(dead_code)] // Future feature: historical status queries
    pub fn get_status_at_time(
        &self,
        node_id: i64,
        at_time: DateTime<Utc>,
    ) -> Result<Option<NodeStatus>> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT to_status
             FROM status_changes
             WHERE node_id = ? AND changed_at < ?
             ORDER BY changed_at DESC
             LIMIT 1",
        )?;

        let result = stmt.query_row(params![node_id, at_time.to_rfc3339()], |row| {
            let status_str: String = row.get(0)?;
            Ok(status_str)
        });

        match result {
            Ok(status_str) => Ok(Some(status_str.parse().unwrap_or(NodeStatus::Offline))),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Calculate uptime percentage over a time period
    /// Returns percentage (0.0 - 100.0) of time the node was Online
    ///
    /// Starts at 100% and decrements based on time spent offline.
    /// This provides a more realistic representation for newly added nodes:
    /// - No status changes = 100% uptime (assumed online)
    /// - With outages = 100% - (offline_time / total_period * 100%)
    pub fn calculate_uptime_percentage(
        &self,
        node_id: i64,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<f64> {
        // Validate time window
        if start_time >= end_time {
            return Err(anyhow::anyhow!(
                "Invalid time window: start_time must be before end_time"
            ));
        }

        let total_duration = StatusChange::calculate_duration(start_time, end_time);
        if total_duration == 0 {
            return Err(anyhow::anyhow!("Invalid time window: zero duration"));
        }

        // Get all status changes that could affect this time window
        // This includes changes within the window AND the last change before the window
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT from_status, to_status, changed_at
             FROM status_changes
             WHERE node_id = ? AND changed_at <= ?
             ORDER BY changed_at ASC",
        )?;

        let changes: Vec<StatusChange> = stmt
            .query_map(params![node_id, end_time.to_rfc3339()], |row| {
                let from_status: String = row.get(0)?;
                let to_status: String = row.get(1)?;
                let changed_at_str: String = row.get(2)?;

                let changed_at = DateTime::parse_from_rfc3339(&changed_at_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(|_| rusqlite::Error::InvalidQuery)?;

                Ok(StatusChange {
                    id: None,
                    node_id,
                    from_status: from_status.parse().unwrap_or(NodeStatus::Offline),
                    to_status: to_status.parse().unwrap_or(NodeStatus::Offline),
                    changed_at,
                    duration_ms: None,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        // If no status changes at all, assume 100% uptime (node is online)
        if changes.is_empty() {
            return Ok(100.0);
        }

        let mut offline_duration = 0i64;
        let mut current_status = NodeStatus::Online; // Default assumption
        let mut current_time = start_time;

        // Determine initial status at start_time
        for change in &changes {
            if change.changed_at < start_time {
                current_status = change.to_status;
            } else {
                break;
            }
        }

        // Process each status change within or after the window
        for change in &changes {
            if change.changed_at >= end_time {
                break;
            }

            if change.changed_at > start_time {
                // Calculate duration from current_time to this change
                // (but only count time within the window)
                let period_start = current_time.max(start_time);
                let period_end = change.changed_at.min(end_time);

                if period_start < period_end && current_status == NodeStatus::Offline {
                    offline_duration += StatusChange::calculate_duration(period_start, period_end);
                }

                current_time = change.changed_at;
                current_status = change.to_status;
            } else if change.changed_at == start_time {
                // Change happens exactly at start_time
                current_time = start_time;
                current_status = change.to_status;
            }
        }

        // Handle the remaining time from the last change to end_time
        if current_time < end_time && current_status == NodeStatus::Offline {
            offline_duration += StatusChange::calculate_duration(current_time, end_time);
        }

        // Calculate uptime as 100% minus the percentage of time spent offline
        let offline_percentage = (offline_duration as f64 / total_duration as f64) * 100.0;
        let uptime_percentage = 100.0 - offline_percentage;
        Ok(uptime_percentage.clamp(0.0, 100.0))
    }

    /// Converts a database row to a Node struct
    fn row_to_node(&self, row: &Row) -> std::result::Result<Node, rusqlite::Error> {
        let detail = MonitorDetail::from_row(row)?;
        let status: String = row.get("status")?;

        let last_check_str: Option<String> = row.get("last_check")?;
        let last_check = last_check_str
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.with_timezone(&Utc));

        Ok(Node {
            id: row.get("id")?,
            name: row.get("name")?,
            detail,
            status: status.parse().unwrap_or(NodeStatus::Offline),
            last_check,
            response_time: row.get("response_time")?,
            monitoring_interval: row.get("monitoring_interval")?,
            credential_id: row.get("credential_id")?,
        })
    }

    /// Converts a database row to a StatusChange struct
    fn row_to_status_change(
        &self,
        row: &Row,
    ) -> std::result::Result<StatusChange, rusqlite::Error> {
        let from_status: String = row.get("from_status")?;
        let to_status: String = row.get("to_status")?;
        let changed_at_str: String = row.get("changed_at")?;

        let changed_at = DateTime::parse_from_rfc3339(&changed_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|_| rusqlite::Error::InvalidQuery)?;

        Ok(StatusChange {
            id: row.get("id")?,
            node_id: row.get("node_id")?,
            from_status: from_status.parse().unwrap_or(NodeStatus::Offline),
            to_status: to_status.parse().unwrap_or(NodeStatus::Offline),
            changed_at,
            duration_ms: row.get("duration_ms")?,
        })
    }
}

type DbParams = (
    &'static str,   // monitor_type
    Option<String>, // http_url
    Option<u16>,    // http_expected_status
    Option<String>, // ping_host
    Option<u32>,    // ping_count
    Option<u64>,    // ping_timeout
    Option<String>, // tcp_host
    Option<u16>,    // tcp_port
    Option<u64>,    // tcp_timeout
);

impl MonitorDetail {
    fn to_db_params(&self) -> DbParams {
        match self {
            MonitorDetail::Http {
                url,
                expected_status,
            } => (
                "http",
                Some(url.clone()),
                Some(*expected_status),
                None,
                None,
                None,
                None,
                None,
                None,
            ),
            MonitorDetail::Ping {
                host,
                count,
                timeout,
            } => (
                "ping",
                None,
                None,
                Some(host.clone()),
                Some(*count),
                Some(*timeout),
                None,
                None,
                None,
            ),
            MonitorDetail::Tcp {
                host,
                port,
                timeout,
            } => (
                "tcp",
                None,
                None,
                None,
                None,
                None,
                Some(host.clone()),
                Some(*port),
                Some(*timeout),
            ),
        }
    }

    fn from_row(row: &Row) -> std::result::Result<MonitorDetail, rusqlite::Error> {
        let monitor_type: String = row.get("monitor_type")?;
        match monitor_type.as_str() {
            "http" => Ok(MonitorDetail::Http {
                url: row.get("http_url")?,
                expected_status: row.get("http_expected_status")?,
            }),
            "ping" => Ok(MonitorDetail::Ping {
                host: row.get("ping_host")?,
                count: row.get("ping_count")?,
                timeout: row.get("ping_timeout")?,
            }),
            "tcp" => Ok(MonitorDetail::Tcp {
                host: row.get("tcp_host")?,
                port: row.get("tcp_port")?,
                timeout: row.get("tcp_timeout")?,
            }),
            _ => Err(rusqlite::Error::InvalidColumnType(
                0,
                "monitor_type".to_string(),
                rusqlite::types::Type::Text,
            )),
        }
    }
}

impl std::str::FromStr for NodeStatus {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Online" => Ok(NodeStatus::Online),
            "Offline" => Ok(NodeStatus::Offline),
            _ => Ok(NodeStatus::Offline), // Default to Offline for unknown strings
        }
    }
}
