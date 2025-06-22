
mod gui;
mod models;
mod monitor;
mod database;

use anyhow::Result;
use eframe::egui;
use crate::gui::NetworkMonitorApp;
use crate::database::Database;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};
use directories::ProjectDirs;

/// Main entry point for the network monitor application
fn main() -> Result<()> {
    // Setup logging
    if let Some(proj_dirs) = project_dirs() {
        let log_dir = proj_dirs.data_dir();
        std::fs::create_dir_all(log_dir)?;
        let file_appender = tracing_appender::rolling::daily(log_dir, "net-monitor.log");
        let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
        
        tracing_subscriber::registry()
            .with(fmt::layer().with_writer(non_blocking))
            .with(fmt::layer().with_writer(std::io::stdout))
            .with(EnvFilter::from_default_env().add_directive("net_monitor=info".parse()?))
            .init();
    } else {
        tracing_subscriber::fmt::init();
    }

    // Setup database
    let db_path = project_dirs().expect("Could not find project directories").data_dir().join("network_monitor.db");
    let database = Database::new(&db_path)?;

    // Create and run the application
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1024.0, 768.0]),
        ..Default::default()
    };
    
    eframe::run_native(
        "Network Monitor",
        options,
        Box::new(|_cc| Box::new(NetworkMonitorApp::new(database).expect("Failed to create app"))),
    ).map_err(|e| anyhow::anyhow!("Eframe error: {}", e))
}

fn project_dirs() -> Option<ProjectDirs> {
    ProjectDirs::from("com", "casey", "net-monitor")
} 