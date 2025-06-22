use crate::models::{MonitorDetail, MonitoringResult, Node, NodeStatus};
use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, Row};
use std::path::{Path, PathBuf};

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
                monitoring_interval INTEGER NOT NULL DEFAULT 60,
                http_url TEXT,
                http_expected_status INTEGER,
                ping_host TEXT,
                ping_count INTEGER,
                ping_timeout INTEGER,
                snmp_target TEXT,
                snmp_community TEXT,
                snmp_oid TEXT
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
        Ok(())
    }

    /// Adds a new node to the database
    pub fn add_node(&self, node: &Node) -> Result<i64> {
        let conn = self.get_connection()?;
        let (
            monitor_type,
            http_url,
            http_expected_status,
            ping_host,
            ping_count,
            ping_timeout,
            snmp_target,
            snmp_community,
            snmp_oid,
        ) = node.detail.to_db_params();

        let status_str = node.status.to_string();

        conn.execute(
            "INSERT INTO nodes (
                name, monitor_type, status, last_check, response_time, monitoring_interval,
                http_url, http_expected_status, ping_host, ping_count, ping_timeout,
                snmp_target, snmp_community, snmp_oid
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                node.name,
                monitor_type,
                status_str,
                node.last_check.map(|dt| dt.to_rfc3339()),
                node.response_time,
                node.monitoring_interval,
                http_url,
                http_expected_status,
                ping_host,
                ping_count,
                ping_timeout,
                snmp_target,
                snmp_community,
                snmp_oid,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Updates an existing node in the database
    pub fn update_node(&self, node: &Node) -> Result<()> {
        let conn = self.get_connection()?;
        let (
            monitor_type,
            http_url,
            http_expected_status,
            ping_host,
            ping_count,
            ping_timeout,
            snmp_target,
            snmp_community,
            snmp_oid,
        ) = node.detail.to_db_params();
        
        let status_str = node.status.to_string();

        conn.execute(
            "UPDATE nodes SET
                name = ?1, monitor_type = ?2, status = ?3, last_check = ?4, response_time = ?5,
                monitoring_interval = ?6, http_url = ?7, http_expected_status = ?8,
                ping_host = ?9, ping_count = ?10, ping_timeout = ?11, snmp_target = ?12,
                snmp_community = ?13, snmp_oid = ?14
            WHERE id = ?15",
            params![
                node.name,
                monitor_type,
                status_str,
                node.last_check.map(|dt| dt.to_rfc3339()),
                node.response_time,
                node.monitoring_interval,
                http_url,
                http_expected_status,
                ping_host,
                ping_count,
                ping_timeout,
                snmp_target,
                snmp_community,
                snmp_oid,
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
                    http_url, http_expected_status, ping_host, ping_count, ping_timeout,
                    snmp_target, snmp_community, snmp_oid
             FROM nodes ORDER BY name",
        )?;
        let nodes = stmt.query_map([], |row| self.row_to_node(row))?;
        nodes.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
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
            status: status.parse().unwrap_or(NodeStatus::Unknown),
            last_check,
            response_time: row.get("response_time")?,
            monitoring_interval: row.get("monitoring_interval")?,
        })
    }
}

impl MonitorDetail {
    fn to_db_params(&self) -> (&'static str, Option<String>, Option<u16>, Option<String>, Option<u32>, Option<u64>, Option<String>, Option<String>, Option<String>) {
        match self {
            MonitorDetail::Http { url, expected_status } => (
                "http", Some(url.clone()), Some(*expected_status), None, None, None, None, None, None
            ),
            MonitorDetail::Ping { host, count, timeout } => (
                "ping", None, None, Some(host.clone()), Some(*count), Some(*timeout), None, None, None
            ),
            MonitorDetail::Snmp { target, community, oid } => (
                "snmp", None, None, None, None, None, Some(target.clone()), Some(community.clone()), Some(oid.clone())
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
            "snmp" => Ok(MonitorDetail::Snmp {
                target: row.get("snmp_target")?,
                community: row.get("snmp_community")?,
                oid: row.get("snmp_oid")?,
            }),
            _ => Err(rusqlite::Error::InvalidColumnType(0, "monitor_type".to_string(), rusqlite::types::Type::Text)),
        }
    }
}

impl std::str::FromStr for NodeStatus {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Online" => Ok(NodeStatus::Online),
            "Offline" => Ok(NodeStatus::Offline),
            _ => Ok(NodeStatus::Unknown),
        }
    }
} 