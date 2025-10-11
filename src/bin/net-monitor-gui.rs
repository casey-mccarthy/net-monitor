#![windows_subsystem = "windows"]

use anyhow::Result;
use directories::ProjectDirs;
use eframe::egui;
use net_monitor::database::Database;
use net_monitor::gui::NetworkMonitorApp;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Main entry point for the GUI version
fn main() -> Result<()> {
    // Setup logging
    let _guard = if let Some(proj_dirs) = project_dirs() {
        let log_dir = proj_dirs.data_dir();
        std::fs::create_dir_all(log_dir)?;
        let log_file = log_dir.join("net-monitor-gui.log");

        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_file)?;

        let (non_blocking, guard) = tracing_appender::non_blocking(file);

        tracing_subscriber::registry()
            .with(
                fmt::layer()
                    .with_writer(non_blocking)
                    .with_ansi(false)
                    .with_target(false),
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

    // Create and run the GUI application
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1024.0, 768.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Network Monitor",
        options,
        Box::new(|_cc| {
            Ok(Box::new(
                NetworkMonitorApp::new(database).expect("Failed to create app"),
            ))
        }),
    )
    .map_err(|e| anyhow::anyhow!("Eframe error: {}", e))
}

fn project_dirs() -> Option<ProjectDirs> {
    ProjectDirs::from("com", "casey", "net-monitor")
}
