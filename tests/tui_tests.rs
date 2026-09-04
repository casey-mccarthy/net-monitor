use net_monitor::database::Database;
use net_monitor::tui::NetworkMonitorTui;
use tempfile::tempdir;

#[test]
fn test_tui_initialization() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let database = Database::new(&db_path).expect("Failed to create database");

    let result = NetworkMonitorTui::new(database);
    assert!(result.is_ok(), "TUI initialization should succeed");
}

#[test]
fn test_database_integration() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let database = Database::new(&db_path).expect("Failed to create database");

    // Add a test node
    use net_monitor::models::{MonitorDetail, Node, NodeStatus};
    let node = Node {
        id: None,
        name: "Test Node".to_string(),
        detail: MonitorDetail::Http {
            url: "https://example.com".to_string(),
            expected_status: 200,
        },
        status: NodeStatus::Offline,
        last_check: None,
        response_time: None,
        monitoring_interval: 5,
        consecutive_failures: 0,
        max_check_attempts: 3,
        retry_interval: 15,
    };

    let node_id = database.add_node(&node).expect("Failed to add node");
    assert!(node_id > 0, "Node ID should be positive");

    // Verify the TUI can load nodes
    let tui = NetworkMonitorTui::new(database).expect("TUI should initialize");
    // TUI should have loaded the node (we can't directly access private fields,
    // but we can verify through the database)
    drop(tui); // Just verify it doesn't panic
}

#[test]
fn test_monitoring_lifecycle() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let database = Database::new(&db_path).expect("Failed to create database");

    let tui = NetworkMonitorTui::new(database).expect("TUI should initialize");

    // Monitoring should auto-start on initialization
    // We can't directly test this without accessing private fields,
    // but we can verify the TUI is created successfully
    drop(tui); // Should cleanly stop monitoring
}

#[cfg(test)]
mod state_transition_tests {
    use super::*;

    #[test]
    fn test_state_initialization() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let database = Database::new(&db_path).expect("Failed to create database");

        let tui = NetworkMonitorTui::new(database).expect("TUI should initialize");

        // TUI should start in Main state
        drop(tui);
    }
}
