#![windows_subsystem = "windows"]

mod config;
mod connection;
mod credentials;
mod database;
mod gui;
mod models;
mod monitor;
mod tui;

use crate::config::{AppConfig, UiMode};
use crate::database::Database;
use crate::gui::NetworkMonitorApp;
use crate::tui::NetworkMonitorTui;
use anyhow::Result;
use clap::Parser;
use directories::ProjectDirs;
use eframe::egui;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[derive(Parser, Debug)]
#[command(name = "net-monitor")]
#[command(about = "Network monitoring tool with GUI and TUI modes", long_about = None)]
struct Args {
    /// UI mode: gui or tui
    #[arg(short, long, value_name = "MODE")]
    mode: Option<UiMode>,

    /// Save the selected mode as the default
    #[arg(short, long)]
    save_default: bool,
}

/// Main entry point for the network monitor application
fn main() -> Result<()> {
    let args = Args::parse();
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

    // Load or create config
    let mut config = AppConfig::load().unwrap_or_default();

    // Determine which UI mode to use
    let ui_mode = if let Some(mode) = args.mode {
        // Command-line argument takes precedence
        if args.save_default {
            config.ui_mode = mode;
            if let Err(e) = config.save() {
                eprintln!("Warning: Failed to save config: {}", e);
            }
        }
        mode
    } else {
        // Use saved preference
        config.ui_mode
    };

    // Setup database
    let db_path = project_dirs()
        .expect("Could not find project directories")
        .data_dir()
        .join("network_monitor.db");
    let database = Database::new(&db_path)?;

    // Run the appropriate UI mode
    match ui_mode {
        UiMode::Gui => run_gui(database),
        UiMode::Tui => run_tui(database),
    }
}

/// Run the graphical user interface
fn run_gui(database: Database) -> Result<()> {
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

/// Run the terminal user interface
fn run_tui(database: Database) -> Result<()> {
    let mut app = NetworkMonitorTui::new(database)?;
    app.run()
}

fn project_dirs() -> Option<ProjectDirs> {
    ProjectDirs::from("com", "casey", "net-monitor")
}
