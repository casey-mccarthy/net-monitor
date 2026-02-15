mod config;
mod connection;
mod credentials;
mod database;
mod models;
mod monitor;
mod monitoring_engine;
mod tui;

use crate::database::Database;
use crate::tui::NetworkMonitorTui;
use anyhow::Result;
use directories::ProjectDirs;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Main entry point for the network monitor application
fn main() -> Result<()> {
    // Setup logging - guard must be kept alive for the lifetime of the application
    let _guard = if let Some(proj_dirs) = project_dirs() {
        let log_dir = proj_dirs.data_dir();
        std::fs::create_dir_all(log_dir)?;
        let log_file = log_dir.join("net-monitor.log");

        // Open file in append mode to keep all logs in one file
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_file)?;

        let (non_blocking, guard) = tracing_appender::non_blocking(file);

        tracing_subscriber::registry()
            .with(
                fmt::layer()
                    .with_writer(non_blocking)
                    .with_ansi(false) // Disable ANSI color codes in log file
                    .with_target(false), // Cleaner format without module paths
            )
            .with(EnvFilter::from_default_env().add_directive("net_monitor=info".parse()?))
            .init();

        Some(guard)
    } else {
        tracing_subscriber::fmt::init();
        None
    };

    // Setup database
    let db_path = project_dirs()
        .expect("Could not find project directories")
        .data_dir()
        .join("network_monitor.db");
    let database = Database::new(&db_path)?;

    // Create and run the TUI application
    let mut app = NetworkMonitorTui::new(database)?;
    app.run()
}

fn project_dirs() -> Option<ProjectDirs> {
    ProjectDirs::from("com", "casey", "net-monitor")
}
