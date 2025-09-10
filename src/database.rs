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
                http_url, http_expected_status, ping_host, ping_count, ping_timeout
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
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
                monitoring_interval = ?6, http_url = ?7, http_expected_status = ?8,
                ping_host = ?9, ping_count = ?10, ping_timeout = ?11
            WHERE id = ?12",
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
                    http_url, http_expected_status, ping_host, ping_count, ping_timeout
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
            _ => Ok(NodeStatus::Unknown),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{MonitorDetail, MonitoringResult, Node, NodeStatus};
    use chrono::Utc;
    use std::fs;
    use tempfile::NamedTempFile;

    /// Creates a temporary database for testing
    fn create_test_database() -> (Database, NamedTempFile) {
        let temp_file = NamedTempFile::new().unwrap();
        let db = Database::new(temp_file.path()).unwrap();
        (db, temp_file)
    }

    /// Creates a test HTTP node
    fn create_test_http_node() -> Node {
        Node {
            id: None,
            name: "Test HTTP Node".to_string(),
            detail: MonitorDetail::Http {
                url: "https://example.com".to_string(),
                expected_status: 200,
            },
            status: NodeStatus::Online,
            last_check: Some(Utc::now()),
            response_time: Some(150),
            monitoring_interval: 60,
        }
    }

    /// Creates a test ping node
    fn create_test_ping_node() -> Node {
        Node {
            id: None,
            name: "Test Ping Node".to_string(),
            detail: MonitorDetail::Ping {
                host: "192.168.1.1".to_string(),
                count: 4,
                timeout: 5,
            },
            status: NodeStatus::Offline,
            last_check: None,
            response_time: None,
            monitoring_interval: 30,
        }
    }

    #[test]
    fn test_database_creation() {
        let (db, temp_file) = create_test_database();
        assert!(temp_file.path().exists());
        drop(db);
        fs::remove_file(temp_file.path()).unwrap();
    }

    #[test]
    fn test_add_and_get_http_node() {
        let (db, temp_file) = create_test_database();
        let node = create_test_http_node();

        let id = db.add_node(&node).unwrap();
        assert!(id > 0);

        let nodes = db.get_all_nodes().unwrap();
        assert_eq!(nodes.len(), 1);

        let retrieved_node = &nodes[0];
        assert_eq!(retrieved_node.id, Some(id));
        assert_eq!(retrieved_node.name, "Test HTTP Node");
        assert_eq!(retrieved_node.status, NodeStatus::Online);
        assert_eq!(retrieved_node.monitoring_interval, 60);

        if let MonitorDetail::Http {
            url,
            expected_status,
        } = &retrieved_node.detail
        {
            assert_eq!(url, "https://example.com");
            assert_eq!(*expected_status, 200);
        } else {
            panic!("Expected HTTP monitor detail");
        }

        drop(db);
        fs::remove_file(temp_file.path()).unwrap();
    }

    #[test]
    fn test_add_and_get_ping_node() {
        let (db, temp_file) = create_test_database();
        let node = create_test_ping_node();

        let id = db.add_node(&node).unwrap();
        assert!(id > 0);

        let nodes = db.get_all_nodes().unwrap();
        assert_eq!(nodes.len(), 1);

        let retrieved_node = &nodes[0];
        assert_eq!(retrieved_node.id, Some(id));
        assert_eq!(retrieved_node.name, "Test Ping Node");
        assert_eq!(retrieved_node.status, NodeStatus::Offline);
        assert_eq!(retrieved_node.monitoring_interval, 30);

        if let MonitorDetail::Ping {
            host,
            count,
            timeout,
        } = &retrieved_node.detail
        {
            assert_eq!(host, "192.168.1.1");
            assert_eq!(*count, 4);
            assert_eq!(*timeout, 5);
        } else {
            panic!("Expected Ping monitor detail");
        }

        drop(db);
        fs::remove_file(temp_file.path()).unwrap();
    }

    #[test]
    fn test_update_node() {
        let (db, temp_file) = create_test_database();
        let mut node = create_test_http_node();

        let id = db.add_node(&node).unwrap();
        node.id = Some(id);
        node.name = "Updated HTTP Node".to_string();
        node.status = NodeStatus::Offline;

        db.update_node(&node).unwrap();

        let nodes = db.get_all_nodes().unwrap();
        assert_eq!(nodes.len(), 1);

        let retrieved_node = &nodes[0];
        assert_eq!(retrieved_node.name, "Updated HTTP Node");
        assert_eq!(retrieved_node.status, NodeStatus::Offline);

        drop(db);
        fs::remove_file(temp_file.path()).unwrap();
    }

    #[test]
    fn test_delete_node() {
        let (db, temp_file) = create_test_database();
        let node = create_test_http_node();

        let id = db.add_node(&node).unwrap();

        let nodes = db.get_all_nodes().unwrap();
        assert_eq!(nodes.len(), 1);

        db.delete_node(id).unwrap();

        let nodes = db.get_all_nodes().unwrap();
        assert_eq!(nodes.len(), 0);

        drop(db);
        fs::remove_file(temp_file.path()).unwrap();
    }

    #[test]
    fn test_add_monitoring_result() {
        let (db, temp_file) = create_test_database();
        let node = create_test_http_node();
        let node_id = db.add_node(&node).unwrap();

        let result = MonitoringResult {
            id: None,
            node_id,
            timestamp: Utc::now(),
            status: NodeStatus::Online,
            response_time: Some(150),
            details: Some("Success".to_string()),
        };

        let result_id = db.add_monitoring_result(&result).unwrap();
        assert!(result_id > 0);

        drop(db);
        fs::remove_file(temp_file.path()).unwrap();
    }

    #[test]
    fn test_multiple_nodes() {
        let (db, temp_file) = create_test_database();

        let http_node = create_test_http_node();
        let ping_node = create_test_ping_node();

        db.add_node(&http_node).unwrap();
        db.add_node(&ping_node).unwrap();

        let nodes = db.get_all_nodes().unwrap();
        assert_eq!(nodes.len(), 2);

        // Check that nodes are ordered by name
        assert_eq!(nodes[0].name, "Test HTTP Node");
        assert_eq!(nodes[1].name, "Test Ping Node");

        drop(db);
        fs::remove_file(temp_file.path()).unwrap();
    }

    #[test]
    fn test_node_status_parsing() {
        assert_eq!("Online".parse::<NodeStatus>().unwrap(), NodeStatus::Online);
        assert_eq!(
            "Offline".parse::<NodeStatus>().unwrap(),
            NodeStatus::Offline
        );
        assert_eq!(
            "Unknown".parse::<NodeStatus>().unwrap(),
            NodeStatus::Unknown
        );
        assert_eq!(
            "Invalid".parse::<NodeStatus>().unwrap(),
            NodeStatus::Unknown
        );
    }

    #[test]
    fn test_monitor_detail_to_db_params() {
        let http_detail = MonitorDetail::Http {
            url: "https://example.com".to_string(),
            expected_status: 200,
        };
        let params = http_detail.to_db_params();
        assert_eq!(params.0, "http");
        assert_eq!(params.1, Some("https://example.com".to_string()));
        assert_eq!(params.2, Some(200));

        let ping_detail = MonitorDetail::Ping {
            host: "192.168.1.1".to_string(),
            count: 4,
            timeout: 5,
        };
        let params = ping_detail.to_db_params();
        assert_eq!(params.0, "ping");
        assert_eq!(params.3, Some("192.168.1.1".to_string()));
        assert_eq!(params.4, Some(4));
        assert_eq!(params.5, Some(5));
    }

    #[test]
    fn test_node_with_response_time() {
        let (db, temp_file) = create_test_database();
        let mut node = create_test_http_node();
        node.response_time = Some(250);

        let _id = db.add_node(&node).unwrap();

        let nodes = db.get_all_nodes().unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].response_time, Some(250));

        drop(db);
        fs::remove_file(temp_file.path()).unwrap();
    }

    #[test]
    fn test_node_with_last_check() {
        let (db, temp_file) = create_test_database();
        let mut node = create_test_http_node();
        let now = Utc::now();
        node.last_check = Some(now);

        let _id = db.add_node(&node).unwrap();

        let nodes = db.get_all_nodes().unwrap();
        assert_eq!(nodes.len(), 1);
        assert!(nodes[0].last_check.is_some());
        // Allow for small time differences due to database operations
        let time_diff = (nodes[0].last_check.unwrap() - now).num_seconds().abs();
        assert!(time_diff < 5);

        drop(db);
        fs::remove_file(temp_file.path()).unwrap();
    }
}
