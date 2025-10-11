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
                ping_timeout INTEGER
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

        // Migrate Unknown status to Offline
        self.migrate_unknown_status(&conn)?;

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

    /// Adds a new node to the database
    pub fn add_node(&self, node: &Node) -> Result<i64> {
        let conn = self.get_connection()?;
        let (monitor_type, http_url, http_expected_status, ping_host, ping_count, ping_timeout) =
            node.detail.to_db_params();

        let status_str = node.status.to_string();

        conn.execute(
            "INSERT INTO nodes (
                name, monitor_type, status, last_check, response_time, monitoring_interval,
                credential_id, http_url, http_expected_status, ping_host, ping_count, ping_timeout
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
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
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Updates an existing node in the database
    pub fn update_node(&self, node: &Node) -> Result<()> {
        let conn = self.get_connection()?;
        let (monitor_type, http_url, http_expected_status, ping_host, ping_count, ping_timeout) =
            node.detail.to_db_params();

        let status_str = node.status.to_string();

        conn.execute(
            "UPDATE nodes SET
                name = ?1, monitor_type = ?2, status = ?3, last_check = ?4, response_time = ?5,
                monitoring_interval = ?6, credential_id = ?7, http_url = ?8, http_expected_status = ?9,
                ping_host = ?10, ping_count = ?11, ping_timeout = ?12
            WHERE id = ?13",
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
                    credential_id, http_url, http_expected_status, ping_host, ping_count, ping_timeout
             FROM nodes ORDER BY name",
        )?;
        let nodes = stmt.query_map([], |row| self.row_to_node(row))?;
        nodes
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
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

    /// Calculate uptime percentage over a time period
    /// Returns percentage (0.0 - 100.0) of time the node was Online
    pub fn calculate_uptime_percentage(
        &self,
        node_id: i64,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<f64> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT from_status, to_status, changed_at, duration_ms
             FROM status_changes
             WHERE node_id = ? AND changed_at >= ? AND changed_at <= ?
             ORDER BY changed_at ASC",
        )?;

        let changes: Vec<StatusChange> = stmt
            .query_map(
                params![node_id, start_time.to_rfc3339(), end_time.to_rfc3339()],
                |row| {
                    let from_status: String = row.get(0)?;
                    let to_status: String = row.get(1)?;
                    let changed_at_str: String = row.get(2)?;
                    let duration_ms: Option<i64> = row.get(3)?;

                    let changed_at = DateTime::parse_from_rfc3339(&changed_at_str)
                        .map(|dt| dt.with_timezone(&Utc))
                        .map_err(|_| rusqlite::Error::InvalidQuery)?;

                    Ok(StatusChange {
                        id: None,
                        node_id,
                        from_status: from_status.parse().unwrap_or(NodeStatus::Offline),
                        to_status: to_status.parse().unwrap_or(NodeStatus::Offline),
                        changed_at,
                        duration_ms,
                    })
                },
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        if changes.is_empty() {
            return Ok(0.0);
        }

        let total_duration = StatusChange::calculate_duration(start_time, end_time);
        let mut online_duration = 0i64;

        for (i, change) in changes.iter().enumerate() {
            if change.from_status == NodeStatus::Online {
                if let Some(duration) = change.duration_ms {
                    online_duration += duration;
                } else if i + 1 < changes.len() {
                    // Calculate duration to next change
                    let next_change = &changes[i + 1];
                    online_duration +=
                        StatusChange::calculate_duration(change.changed_at, next_change.changed_at);
                }
            }
        }

        // Handle the last status if it was Online
        if let Some(last_change) = changes.last() {
            if last_change.to_status == NodeStatus::Online {
                online_duration +=
                    StatusChange::calculate_duration(last_change.changed_at, end_time);
            }
        }

        let percentage = (online_duration as f64 / total_duration as f64) * 100.0;
        Ok(percentage.clamp(0.0, 100.0))
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
    &'static str,
    Option<String>,
    Option<u16>,
    Option<String>,
    Option<u32>,
    Option<u64>,
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
