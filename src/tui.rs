use crate::connection::ConnectionStrategy;
use crate::database::Database;
use crate::models::{MonitorDetail, Node, NodeImport, NodeStatus, StatusChange};
use crate::monitoring_engine::{self, MonitoringHandle, NodeConfigUpdate};
use anyhow::Result;
use chrono::Utc;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, TableState, Wrap},
    Frame, Terminal,
};
use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use tracing::{error, info};

#[derive(Clone, Copy, PartialEq, Debug)]
enum MonitorTypeForm {
    Http,
    Ping,
    Tcp,
}

impl std::fmt::Display for MonitorTypeForm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MonitorTypeForm::Http => write!(f, "HTTP"),
            MonitorTypeForm::Ping => write!(f, "Ping"),
            MonitorTypeForm::Tcp => write!(f, "TCP"),
        }
    }
}

/// Form data for adding/editing nodes
#[derive(Clone)]
struct NodeForm {
    name: String,
    monitor_type: MonitorTypeForm,
    monitoring_interval: String,
    // HTTP
    http_url: String,
    http_expected_status: String,
    // Ping
    ping_host: String,
    ping_count: String,
    ping_timeout: String,
    // TCP
    tcp_host: String,
    tcp_port: String,
    tcp_timeout: String,
    // Form state
    current_field: usize,
}

impl Default for NodeForm {
    fn default() -> Self {
        Self {
            name: String::new(),
            monitor_type: MonitorTypeForm::Http,
            monitoring_interval: "5".to_string(),
            http_url: "https://".to_string(),
            http_expected_status: "200".to_string(),
            ping_host: String::new(),
            ping_count: "4".to_string(),
            ping_timeout: "5".to_string(),
            tcp_host: String::new(),
            tcp_port: String::new(),
            tcp_timeout: "5".to_string(),
            current_field: 0,
        }
    }
}

impl NodeForm {
    fn to_node_detail(&self) -> Result<MonitorDetail> {
        match self.monitor_type {
            MonitorTypeForm::Http => Ok(MonitorDetail::Http {
                url: self.http_url.clone(),
                expected_status: self.http_expected_status.parse()?,
            }),
            MonitorTypeForm::Ping => Ok(MonitorDetail::Ping {
                host: self.ping_host.clone(),
                count: self.ping_count.parse()?,
                timeout: self.ping_timeout.parse()?,
            }),
            MonitorTypeForm::Tcp => Ok(MonitorDetail::Tcp {
                host: self.tcp_host.clone(),
                port: self.tcp_port.parse()?,
                timeout: self.tcp_timeout.parse()?,
            }),
        }
    }

    fn from_node(node: &Node) -> Self {
        let mut form = Self {
            name: node.name.clone(),
            monitoring_interval: node.monitoring_interval.to_string(),
            ..Default::default()
        };

        match &node.detail {
            MonitorDetail::Http {
                url,
                expected_status,
            } => {
                form.monitor_type = MonitorTypeForm::Http;
                form.http_url = url.clone();
                form.http_expected_status = expected_status.to_string();
            }
            MonitorDetail::Ping {
                host,
                count,
                timeout,
            } => {
                form.monitor_type = MonitorTypeForm::Ping;
                form.ping_host = host.clone();
                form.ping_count = count.to_string();
                form.ping_timeout = timeout.to_string();
            }
            MonitorDetail::Tcp {
                host,
                port,
                timeout,
            } => {
                form.monitor_type = MonitorTypeForm::Tcp;
                form.tcp_host = host.clone();
                form.tcp_port = port.to_string();
                form.tcp_timeout = timeout.to_string();
            }
        }
        form
    }

    fn get_field_count(&self) -> usize {
        // name, monitoring_interval, monitor_type + type-specific fields
        match self.monitor_type {
            MonitorTypeForm::Http => 5, // name, interval, type, url, status
            MonitorTypeForm::Ping => 6, // name, interval, type, host, count, timeout
            MonitorTypeForm::Tcp => 6,  // name, interval, type, host, port, timeout
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum AppState {
    Main,
    AddNode,
    EditNode,
    ViewHistory,
    Help,
    ConfirmDelete,
    ImportModeSelect,
    Reorder,
    About,
}

enum DeferredAction {
    ShowImportDialog,
    ShowExportDialog,
}

enum FileDialogKind {
    Import,
    Export,
}

/// Returns the index to select when moving down a list of `len` items, wrapping
/// from the last item back to the first.
///
/// Returns `None` for an empty list so callers leave the selection untouched.
/// A stale selection at or past the end wraps to the first item rather than
/// indexing off the end.
fn next_selection(selected: Option<usize>, len: usize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    Some(match selected {
        Some(i) if i + 1 < len => i + 1,
        Some(_) => 0,
        None => 0,
    })
}

/// Returns the index to select when moving up a list of `len` items, wrapping
/// from the first item to the last.
///
/// Returns `None` for an empty list so callers leave the selection untouched.
/// A stale selection past the end is clamped into range.
fn previous_selection(selected: Option<usize>, len: usize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    Some(match selected {
        Some(0) => len - 1,
        Some(i) => (i - 1).min(len - 1),
        None => 0,
    })
}

pub struct NetworkMonitorTui {
    database: Database,
    nodes: Vec<Node>,
    table_state: TableState,
    state: AppState,
    status_message: Option<(String, Instant)>,
    monitoring_handle: Option<MonitoringHandle>,
    update_rx: mpsc::Receiver<Node>,
    update_tx: mpsc::Sender<Node>,
    updated_nodes: HashMap<i64, Instant>,
    // Node form
    node_form: NodeForm,
    editing_node_id: Option<i64>,
    // Status history
    viewing_history_node_id: Option<i64>,
    status_changes: Vec<StatusChange>,
    history_table_state: TableState,
    // Delete confirmation
    delete_node_index: Option<usize>,
    // Import/Export
    import_file_path: Option<PathBuf>,
    import_mode_selected: usize,
    deferred_action: Option<DeferredAction>,
    // Auto-hide selection
    last_input_time: Option<Instant>,
    // Cursor blink state for empty fields
    cursor_blink_state: bool,
    last_blink_time: Instant,
    // Help context
    previous_state: Option<AppState>,
    // Reorder mode
    reorder_original_index: Option<usize>,
    /// Node ids in their order before reorder mode started, used to undo on cancel
    reorder_original_order: Option<Vec<Option<i64>>>,
}

impl NetworkMonitorTui {
    pub fn new(database: Database) -> Result<Self> {
        let nodes = database.get_all_nodes()?;
        let (update_tx, update_rx) = mpsc::channel();

        let mut app = Self {
            database,
            nodes,
            table_state: TableState::default(),
            state: AppState::Main,
            status_message: None,
            monitoring_handle: None,
            update_rx,
            update_tx,
            updated_nodes: HashMap::new(),
            node_form: NodeForm::default(),
            editing_node_id: None,
            viewing_history_node_id: None,
            status_changes: Vec::new(),
            history_table_state: TableState::default(),
            delete_node_index: None,
            import_file_path: None,
            import_mode_selected: 0,
            deferred_action: None,
            last_input_time: Some(Instant::now()),
            cursor_blink_state: true,
            last_blink_time: Instant::now(),
            previous_state: None,
            reorder_original_index: None,
            reorder_original_order: None,
        };

        // Select first node if any exist
        if !app.nodes.is_empty() {
            app.table_state.select(Some(0));
        }

        // Start monitoring automatically
        app.start_monitoring();
        info!("TUI: Monitoring started automatically on application launch");

        Ok(app)
    }

    pub fn run(&mut self) -> Result<()> {
        // If anything panics while the terminal is in raw mode and on the
        // alternate screen, the user's shell is left unusable (no echo, no
        // line editing) until they run `reset`. Restore the terminal first,
        // then let the default hook print the panic message normally.
        let original_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            Self::restore_terminal();
            original_hook(info);
        }));

        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        if let Err(e) = execute!(stdout, EnterAlternateScreen, EnableMouseCapture) {
            let _ = disable_raw_mode();
            return Err(e.into());
        }
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = match Terminal::new(backend) {
            Ok(terminal) => terminal,
            Err(e) => {
                Self::restore_terminal();
                return Err(e.into());
            }
        };

        let result = self.run_app(&mut terminal);

        // Restore terminal. Try every step even if an earlier one fails so a
        // single error cannot leave the shell half-restored.
        Self::restore_terminal();
        let _ = terminal.show_cursor();

        // Stop monitoring
        if let Err(e) = self.stop_monitoring() {
            error!("Failed to stop monitoring: {}", e);
        }

        result
    }

    /// Copies the monitoring runtime state (status, last check, response time,
    /// consecutive failures) from `source` onto `target`, leaving `target`'s
    /// configuration untouched.
    fn apply_runtime_state(target: &mut Node, source: &Node) {
        target.status = source.status;
        target.last_check = source.last_check;
        target.response_time = source.response_time;
        target.consecutive_failures = source.consecutive_failures;
    }

    /// Puts the terminal back into its normal state. Safe to call more than
    /// once and from a panic hook; failures are ignored because there is
    /// nothing better to do with them at that point.
    fn restore_terminal() {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
    }

    fn run_app<B: ratatui::backend::Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        loop {
            terminal.draw(|f| self.ui(f))?;

            // Check for node updates
            while let Ok(updated_node) = self.update_rx.try_recv() {
                if let Some(node) = self.nodes.iter_mut().find(|n| n.id == updated_node.id) {
                    if let Some(node_id) = updated_node.id {
                        self.updated_nodes.insert(node_id, Instant::now());
                    }
                    // Only take the runtime state from the engine. The engine's copy of
                    // the configuration may predate an edit the user just saved, so
                    // replacing the whole node would revert that edit on screen.
                    Self::apply_runtime_state(node, &updated_node);
                }
            }

            // Clean up old flash animations
            let now = Instant::now();
            self.updated_nodes
                .retain(|_, timestamp| now.duration_since(*timestamp).as_millis() < 1000);

            // Clear old status messages
            if let Some((_, timestamp)) = self.status_message {
                if now.duration_since(timestamp).as_secs() > 5 {
                    self.status_message = None;
                }
            }

            // Auto-hide selection highlight after 5 seconds of inactivity
            if let Some(last_input) = self.last_input_time {
                if now.duration_since(last_input).as_secs() >= 5 && self.state != AppState::Reorder
                {
                    self.last_input_time = None;
                }
            }

            // Toggle cursor blink state every 530ms (standard terminal blink rate)
            if now.duration_since(self.last_blink_time).as_millis() >= 530 {
                self.cursor_blink_state = !self.cursor_blink_state;
                self.last_blink_time = now;
            }

            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    // On Windows, crossterm reports both KeyPress and KeyRelease events.
                    // We only want to handle KeyPress to avoid double-processing each keystroke.
                    // On macOS/Linux, only KeyPress events are generated.
                    if key.kind == KeyEventKind::Press {
                        match self.state {
                            AppState::Main => {
                                if self.handle_main_input(key.code, key.modifiers)? {
                                    break;
                                }
                            }
                            AppState::AddNode => {
                                if self.handle_node_form_input(key.code, key.modifiers) {
                                    self.state = AppState::Main;
                                }
                            }
                            AppState::EditNode => {
                                if self.handle_node_form_input(key.code, key.modifiers) {
                                    self.state = AppState::Main;
                                    self.editing_node_id = None;
                                }
                            }
                            AppState::ViewHistory => match key.code {
                                KeyCode::Esc | KeyCode::Char('q') => {
                                    self.state = AppState::Main;
                                    self.viewing_history_node_id = None;
                                    self.status_changes.clear();
                                    self.history_table_state.select(None);
                                }
                                KeyCode::Char('?') => {
                                    self.previous_state = Some(AppState::ViewHistory);
                                    self.state = AppState::Help;
                                }
                                KeyCode::Down => {
                                    let rows = self.history_row_count();
                                    select_next_row(&mut self.history_table_state, rows);
                                }
                                KeyCode::Up => {
                                    let rows = self.history_row_count();
                                    select_previous_row(&mut self.history_table_state, rows);
                                }
                                _ => {}
                            },
                            AppState::Help => {
                                if matches!(
                                    key.code,
                                    KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?')
                                ) {
                                    self.state = self.previous_state.unwrap_or(AppState::Main);
                                    self.previous_state = None;
                                }
                            }
                            AppState::About => {
                                if matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) {
                                    self.state = AppState::Main;
                                }
                            }
                            AppState::ConfirmDelete => {
                                if self.handle_confirm_delete_input(key.code) {
                                    self.state = AppState::Main;
                                }
                            }
                            AppState::ImportModeSelect => {
                                if self.handle_import_mode_input(key.code) {
                                    self.state = AppState::Main;
                                }
                            }
                            AppState::Reorder => {
                                if self.handle_reorder_input(key.code) {
                                    self.state = AppState::Main;
                                }
                            }
                        }
                    }
                }
            }

            // Handle deferred actions (file dialogs) outside the key event match
            if let Some(action) = self.deferred_action.take() {
                match action {
                    DeferredAction::ShowImportDialog => {
                        match self.show_file_dialog(FileDialogKind::Import, terminal)? {
                            Some(path) => {
                                self.import_file_path = Some(path);
                                self.import_mode_selected = 0;
                                self.state = AppState::ImportModeSelect;
                            }
                            None => self.set_status_message(
                                "Import cancelled (no file chosen or no file dialog available)",
                            ),
                        }
                    }
                    DeferredAction::ShowExportDialog => {
                        match self.show_file_dialog(FileDialogKind::Export, terminal)? {
                            Some(path) => self.export_nodes_to_path(&path),
                            None => self.set_status_message(
                                "Export cancelled (no file chosen or no file dialog available)",
                            ),
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn show_file_dialog<B: ratatui::backend::Backend>(
        &self,
        kind: FileDialogKind,
        terminal: &mut Terminal<B>,
    ) -> Result<Option<PathBuf>> {
        // Temporarily leave TUI mode so the OS file dialog can appear. Mouse
        // capture must go too, otherwise clicks while the dialog is open are
        // echoed into the cooked-mode terminal as escape sequences.
        disable_raw_mode()?;
        execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;

        let result = match kind {
            FileDialogKind::Import => rfd::FileDialog::new()
                .add_filter("JSON", &["json"])
                .pick_file(),
            FileDialogKind::Export => rfd::FileDialog::new()
                .add_filter("JSON", &["json"])
                .set_file_name("nodes.json")
                .save_file(),
        };

        // Re-enter TUI mode
        execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
        enable_raw_mode()?;
        terminal.clear()?;

        Ok(result)
    }

    fn ui(&mut self, f: &mut Frame) {
        match self.state {
            AppState::Main | AppState::Reorder => self.render_main_view(f),
            AppState::AddNode | AppState::EditNode => self.render_node_form(f),
            AppState::ViewHistory => self.render_history_view(f),
            AppState::Help => self.render_help_view(f),
            AppState::ConfirmDelete => self.render_confirm_delete(f),
            AppState::ImportModeSelect => self.render_import_mode_select(f),
            AppState::About => self.render_about_view(f),
        }
    }

    fn render_main_view(&mut self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Title
                Constraint::Min(0),    // Content
                Constraint::Length(3), // Status bar
            ])
            .split(f.area());

        // Title
        let title = Paragraph::new("Network Monitor (TUI)")
            .style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(title, chunks[0]);

        // Menu bar
        let menu_text = if self.state == AppState::Reorder {
            vec![
                Span::styled(
                    "REORDER MODE",
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" - ["),
                Span::styled("Up/Down", Style::default().fg(Color::Yellow)),
                Span::raw("] Move ["),
                Span::styled("R", Style::default().fg(Color::Yellow)),
                Span::raw("] Confirm ["),
                Span::styled("Esc", Style::default().fg(Color::Yellow)),
                Span::raw("] Cancel"),
            ]
        } else {
            vec![
                Span::raw("["),
                Span::styled("M", Style::default().fg(Color::Yellow)),
                Span::raw("]onitor "),
                Span::raw("["),
                Span::styled("A", Style::default().fg(Color::Yellow)),
                Span::raw("]dd "),
                Span::raw("["),
                Span::styled("E", Style::default().fg(Color::Yellow)),
                Span::raw("]dit "),
                Span::raw("["),
                Span::styled("D", Style::default().fg(Color::Yellow)),
                Span::raw("]elete "),
                Span::raw("["),
                Span::styled("H", Style::default().fg(Color::Yellow)),
                Span::raw("]istory "),
                Span::raw("["),
                Span::styled("R", Style::default().fg(Color::Yellow)),
                Span::raw("]eorder "),
                Span::raw("["),
                Span::styled("I", Style::default().fg(Color::Yellow)),
                Span::raw("]mport\u{2026} "),
                Span::raw("["),
                Span::styled("X", Style::default().fg(Color::Yellow)),
                Span::raw("]export\u{2026} "),
                Span::raw("["),
                Span::styled("B", Style::default().fg(Color::Yellow)),
                Span::raw("]About "),
                Span::raw("["),
                Span::styled("?", Style::default().fg(Color::Yellow)),
                Span::raw("]Help "),
                Span::raw("["),
                Span::styled("Q", Style::default().fg(Color::Yellow)),
                Span::raw("]uit"),
            ]
        };

        let content_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(0)])
            .split(chunks[1]);

        let menu = Paragraph::new(Line::from(menu_text));
        f.render_widget(menu, content_chunks[0]);

        // Node table
        let header = Row::new(vec![
            "Name",
            "Target",
            "Type",
            "Status",
            "Latency",
            "Uptime/Downtime",
            "Last Check",
        ])
        .style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .bottom_margin(1);

        let rows: Vec<Row> = self
            .nodes
            .iter()
            .map(|node| {
                // Determine if this node was recently updated (for pulsing effect)
                let flash_intensity = if let Some(node_id) = node.id {
                    if let Some(update_time) = self.updated_nodes.get(&node_id) {
                        let elapsed = Instant::now().duration_since(*update_time).as_millis();
                        if elapsed < 1000 {
                            // Fade from 1.0 to 0.0 over 1 second
                            1.0 - (elapsed as f32 / 1000.0)
                        } else {
                            0.0
                        }
                    } else {
                        0.0
                    }
                } else {
                    0.0
                };

                // Color-code the status
                let status_color = match node.status {
                    NodeStatus::Online => Color::Green,
                    NodeStatus::Offline => Color::Red,
                    NodeStatus::Degraded => Color::Yellow,
                };

                // Add visual indicator for status
                let status_str = match node.status {
                    NodeStatus::Online => "● Online",
                    NodeStatus::Offline => "● Offline",
                    NodeStatus::Degraded => "◐ Degraded",
                };

                let last_check = node
                    .last_check
                    .map(|t| {
                        t.with_timezone(&chrono::Local)
                            .format("%H:%M:%S")
                            .to_string()
                    })
                    .unwrap_or_else(|| "Never".to_string());

                // Add pulsing indicator when check just occurred
                let last_check_display = if flash_intensity > 0.0 {
                    format!("{} ⟳", last_check)
                } else {
                    last_check
                };

                // Calculate text color for Last Check cell based on status and flash intensity
                let last_check_color = if flash_intensity > 0.0 {
                    // Use status color for text during flash
                    match node.status {
                        NodeStatus::Online => Color::Green,
                        NodeStatus::Offline => Color::Red,
                        NodeStatus::Degraded => Color::Yellow,
                    }
                } else {
                    Color::White
                };

                // Get uptime/downtime
                let uptime_downtime = if let Some(node_id) = node.id {
                    match self.database.get_current_status_duration(node_id) {
                        Ok(Some(duration_ms)) => format_duration(duration_ms),
                        _ => "N/A".to_string(),
                    }
                } else {
                    "N/A".to_string()
                };

                // Create cells with individual styling using Span::styled
                // to embed color directly in text content for reliable style updates
                let cells = vec![
                    Cell::from(Span::styled(
                        node.name.clone(),
                        Style::default().fg(Color::White),
                    )),
                    Cell::from(Span::styled(
                        node.detail.get_connection_target(),
                        Style::default().fg(Color::Cyan),
                    )),
                    Cell::from(Span::styled(
                        node.detail.to_string(),
                        Style::default().fg(Color::Yellow),
                    )),
                    Cell::from(Span::styled(
                        status_str,
                        Style::default()
                            .fg(status_color)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Cell::from(Span::styled(
                        latency_text(node),
                        Style::default().fg(latency_color(node)),
                    )),
                    Cell::from(Span::styled(
                        uptime_downtime,
                        Style::default().fg(Color::White),
                    )),
                    Cell::from(Span::styled(
                        last_check_display,
                        Style::default().fg(last_check_color).add_modifier(
                            if flash_intensity > 0.0 {
                                Modifier::BOLD
                            } else {
                                Modifier::empty()
                            },
                        ),
                    )),
                ];

                Row::new(cells)
            })
            .collect();

        // Conditionally apply highlight based on mode and input activity
        let (highlight_style, highlight_symbol) = if self.state == AppState::Reorder {
            (
                Style::default()
                    .bg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
                "<> ",
            )
        } else if self.last_input_time.is_some() {
            // Show gray background when there has been recent input
            (
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
                ">> ",
            )
        } else {
            // Hide gray background after 5 seconds of inactivity, but keep >> symbol
            (Style::default(), ">> ")
        };

        let table = Table::new(
            rows,
            [
                Constraint::Percentage(16),
                Constraint::Percentage(20),
                Constraint::Percentage(8),
                Constraint::Percentage(12),
                Constraint::Percentage(10),
                Constraint::Percentage(16),
                Constraint::Percentage(18),
            ],
        )
        .header(header)
        .block(Block::default().borders(Borders::ALL).title("Nodes"))
        .row_highlight_style(highlight_style)
        .highlight_symbol(highlight_symbol);

        f.render_stateful_widget(table, content_chunks[1], &mut self.table_state);

        // Status bar
        let monitoring_status = if self.monitoring_handle.is_some() {
            Span::styled("Monitoring: ON", Style::default().fg(Color::Green))
        } else {
            Span::styled("Monitoring: OFF", Style::default().fg(Color::Red))
        };

        let degraded_count = self
            .nodes
            .iter()
            .filter(|n| n.status == NodeStatus::Degraded)
            .count();
        let node_stats = if degraded_count > 0 {
            format!(
                " | {} nodes | {} online, {} degraded, {} offline",
                self.nodes.len(),
                self.nodes
                    .iter()
                    .filter(|n| n.status == NodeStatus::Online)
                    .count(),
                degraded_count,
                self.nodes
                    .iter()
                    .filter(|n| n.status == NodeStatus::Offline)
                    .count()
            )
        } else {
            format!(
                " | {} nodes | {} online, {} offline",
                self.nodes.len(),
                self.nodes
                    .iter()
                    .filter(|n| n.status == NodeStatus::Online)
                    .count(),
                self.nodes
                    .iter()
                    .filter(|n| n.status == NodeStatus::Offline)
                    .count()
            )
        };

        let mut status_line = vec![monitoring_status, Span::raw(node_stats)];

        if let Some((ref msg, _)) = self.status_message {
            status_line.push(Span::raw(" | "));
            status_line.push(Span::styled(
                msg.clone(),
                Style::default().fg(Color::Yellow),
            ));
        }

        let status =
            Paragraph::new(Line::from(status_line)).block(Block::default().borders(Borders::ALL));
        f.render_widget(status, chunks[2]);
    }

    fn render_node_form(&mut self, f: &mut Frame) {
        let area = centered_rect(60, 80, f.area());
        f.render_widget(Clear, area);

        let title = if self.state == AppState::AddNode {
            "Add Node"
        } else {
            "Edit Node"
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let inner = block.inner(area);
        f.render_widget(block, area);

        let form = &self.node_form;
        let cursor = if self.cursor_blink_state { "│" } else { "" };
        let mut lines = vec![
            Line::from(vec![
                Span::raw("Name: "),
                Span::styled(
                    if form.name.is_empty() && form.current_field == 0 {
                        cursor
                    } else {
                        &form.name
                    },
                    if form.current_field == 0 {
                        Style::default().bg(Color::DarkGray)
                    } else {
                        Style::default()
                    },
                ),
            ]),
            Line::from(vec![
                Span::raw("Monitoring Interval (s): "),
                Span::styled(
                    if form.monitoring_interval.is_empty() && form.current_field == 1 {
                        cursor
                    } else {
                        &form.monitoring_interval
                    },
                    if form.current_field == 1 {
                        Style::default().bg(Color::DarkGray)
                    } else {
                        Style::default()
                    },
                ),
            ]),
            Line::from(vec![
                Span::raw("Monitor Type: "),
                Span::styled(
                    format!("{} ", form.monitor_type),
                    if form.current_field == 2 {
                        Style::default().bg(Color::DarkGray)
                    } else {
                        Style::default()
                    },
                ),
                if form.current_field == 2 {
                    Span::styled("[←/→ or Space to change]", Style::default().fg(Color::Gray))
                } else {
                    Span::raw("")
                },
            ]),
        ];

        match form.monitor_type {
            MonitorTypeForm::Http => {
                lines.push(Line::from(vec![
                    Span::raw("URL: "),
                    Span::styled(
                        if form.http_url.is_empty() && form.current_field == 3 {
                            cursor
                        } else {
                            &form.http_url
                        },
                        if form.current_field == 3 {
                            Style::default().bg(Color::DarkGray)
                        } else {
                            Style::default()
                        },
                    ),
                ]));
                lines.push(Line::from(vec![
                    Span::raw("Expected Status: "),
                    Span::styled(
                        if form.http_expected_status.is_empty() && form.current_field == 3 {
                            cursor
                        } else {
                            &form.http_expected_status
                        },
                        if form.current_field == 3 {
                            Style::default().bg(Color::DarkGray)
                        } else {
                            Style::default()
                        },
                    ),
                ]));
            }
            MonitorTypeForm::Ping => {
                lines.push(Line::from(vec![
                    Span::raw("Host: "),
                    Span::styled(
                        if form.ping_host.is_empty() && form.current_field == 3 {
                            cursor
                        } else {
                            &form.ping_host
                        },
                        if form.current_field == 3 {
                            Style::default().bg(Color::DarkGray)
                        } else {
                            Style::default()
                        },
                    ),
                ]));
                lines.push(Line::from(vec![
                    Span::raw("Count: "),
                    Span::styled(
                        if form.ping_count.is_empty() && form.current_field == 3 {
                            cursor
                        } else {
                            &form.ping_count
                        },
                        if form.current_field == 3 {
                            Style::default().bg(Color::DarkGray)
                        } else {
                            Style::default()
                        },
                    ),
                ]));
                lines.push(Line::from(vec![
                    Span::raw("Timeout (s): "),
                    Span::styled(
                        if form.ping_timeout.is_empty() && form.current_field == 3 {
                            cursor
                        } else {
                            &form.ping_timeout
                        },
                        if form.current_field == 3 {
                            Style::default().bg(Color::DarkGray)
                        } else {
                            Style::default()
                        },
                    ),
                ]));
            }
            MonitorTypeForm::Tcp => {
                lines.push(Line::from(vec![
                    Span::raw("Host: "),
                    Span::styled(
                        if form.tcp_host.is_empty() && form.current_field == 3 {
                            cursor
                        } else {
                            &form.tcp_host
                        },
                        if form.current_field == 3 {
                            Style::default().bg(Color::DarkGray)
                        } else {
                            Style::default()
                        },
                    ),
                ]));
                lines.push(Line::from(vec![
                    Span::raw("Port: "),
                    Span::styled(
                        if form.tcp_port.is_empty() && form.current_field == 3 {
                            cursor
                        } else {
                            &form.tcp_port
                        },
                        if form.current_field == 3 {
                            Style::default().bg(Color::DarkGray)
                        } else {
                            Style::default()
                        },
                    ),
                ]));
                lines.push(Line::from(vec![
                    Span::raw("Timeout (s): "),
                    Span::styled(
                        if form.tcp_timeout.is_empty() && form.current_field == 3 {
                            cursor
                        } else {
                            &form.tcp_timeout
                        },
                        if form.current_field == 3 {
                            Style::default().bg(Color::DarkGray)
                        } else {
                            Style::default()
                        },
                    ),
                ]));
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(
                "[Tab]",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Next | "),
            Span::styled(
                "[←/→/Space]",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Change | "),
            Span::styled(
                "[Enter]",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Save | "),
            Span::styled(
                "[Esc]",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Cancel"),
        ]));

        let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });
        f.render_widget(paragraph, inner);
    }

    fn render_history_view(&mut self, f: &mut Frame) {
        let area = centered_rect(80, 80, f.area());
        f.render_widget(Clear, area);

        let node_name = self
            .nodes
            .iter()
            .find(|n| n.id == self.viewing_history_node_id)
            .map(|n| n.name.clone())
            .unwrap_or_else(|| "Unknown".to_string());

        let block = Block::default()
            .title(format!("Status History - {}", node_name))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let inner = block.inner(area);
        f.render_widget(block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(6), // Uptime statistics section
                Constraint::Min(0),    // Status change history
                Constraint::Length(1), // Help text
            ])
            .split(inner);

        // Uptime Statistics Section
        if let Some(node_id) = self.viewing_history_node_id {
            let now = Utc::now();
            let periods = vec![
                ("Last 24 Hours", now - chrono::Duration::hours(24)),
                ("Last 7 Days", now - chrono::Duration::days(7)),
                ("Last 30 Days", now - chrono::Duration::days(30)),
            ];

            let mut uptime_lines = vec![Line::from(vec![Span::styled(
                "Uptime Statistics",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )])];

            // Add current status duration
            if let Ok(Some(duration_ms)) = self.database.get_current_status_duration(node_id) {
                // Get current status
                let current_status = self
                    .nodes
                    .iter()
                    .find(|n| n.id == Some(node_id))
                    .map(|n| n.status)
                    .unwrap_or(NodeStatus::Offline);

                let status_color = match current_status {
                    NodeStatus::Online => Color::Green,
                    NodeStatus::Offline => Color::Red,
                    NodeStatus::Degraded => Color::Yellow,
                };

                uptime_lines.push(Line::from(vec![
                    Span::raw("Time in Current Status ("),
                    Span::styled(
                        current_status.to_string(),
                        Style::default()
                            .fg(status_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("): "),
                    Span::styled(
                        format_duration(duration_ms),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                ]));
            }

            for (label, start_time) in periods {
                if let Ok(uptime_pct) = self
                    .database
                    .calculate_uptime_percentage(node_id, start_time, now)
                {
                    let color = if uptime_pct >= 99.0 {
                        Color::Green
                    } else if uptime_pct >= 95.0 {
                        Color::Yellow
                    } else {
                        Color::Red
                    };

                    uptime_lines.push(Line::from(vec![
                        Span::raw(format!("{}: ", label)),
                        Span::styled(
                            format!("{:.2}%", uptime_pct),
                            Style::default().fg(color).add_modifier(Modifier::BOLD),
                        ),
                    ]));
                }
            }

            let uptime_paragraph = Paragraph::new(uptime_lines).wrap(Wrap { trim: true });
            f.render_widget(uptime_paragraph, chunks[0]);
        }

        // Status Change History Section
        if self.status_changes.is_empty() && self.viewing_history_node_id.is_none() {
            let msg = Paragraph::new("No status changes recorded.")
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::Gray));
            f.render_widget(msg, chunks[1]);
        } else {
            let header = Row::new(vec!["Timestamp", "State", "Duration"])
                .style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
                .bottom_margin(1);

            let mut rows: Vec<Row> = Vec::new();

            // Add current state as the first row
            if let Some(node_id) = self.viewing_history_node_id {
                // Get current node status
                let current_status = self
                    .nodes
                    .iter()
                    .find(|n| n.id == Some(node_id))
                    .map(|n| n.status)
                    .unwrap_or(NodeStatus::Offline);

                // Get current duration
                let current_duration = self
                    .database
                    .get_current_status_duration(node_id)
                    .ok()
                    .flatten()
                    .map(format_duration)
                    .unwrap_or_else(|| "N/A".to_string());

                let status_color = match current_status {
                    NodeStatus::Online => Color::Green,
                    NodeStatus::Offline => Color::Red,
                    NodeStatus::Degraded => Color::Yellow,
                };

                let state_text = match current_status {
                    NodeStatus::Online => "Up",
                    NodeStatus::Degraded => "Degraded",
                    NodeStatus::Offline => "Down",
                };

                // Add current state row
                rows.push(Row::new(vec![
                    Cell::from(Span::styled(
                        "Current",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Cell::from(Span::styled(
                        state_text,
                        Style::default()
                            .fg(status_color)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Cell::from(Span::styled(
                        current_duration,
                        Style::default().add_modifier(Modifier::BOLD),
                    )),
                ]));
            }

            // Add historical status changes
            rows.extend(self.status_changes.iter().map(|change| {
                let timestamp = change
                    .changed_at
                    .with_timezone(&chrono::Local)
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string();

                let duration = change
                    .duration_ms
                    .map(format_duration)
                    .unwrap_or_else(|| "N/A".to_string());

                // Use to_status since changed_at represents when the node transitioned to this state
                let status_color = match change.to_status {
                    NodeStatus::Online => Color::Green,
                    NodeStatus::Offline => Color::Red,
                    NodeStatus::Degraded => Color::Yellow,
                };

                let state_text = match change.to_status {
                    NodeStatus::Online => "Up",
                    NodeStatus::Degraded => "Degraded",
                    NodeStatus::Offline => "Down",
                };

                Row::new(vec![
                    Cell::from(Span::styled(timestamp, Style::default())),
                    Cell::from(Span::styled(state_text, Style::default().fg(status_color))),
                    Cell::from(Span::styled(duration, Style::default())),
                ])
            }));

            let table = Table::new(
                rows,
                [
                    Constraint::Percentage(40),
                    Constraint::Percentage(20),
                    Constraint::Percentage(40),
                ],
            )
            .header(header)
            .row_highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");

            f.render_stateful_widget(table, chunks[1], &mut self.history_table_state);
        }

        let help = Paragraph::new(Line::from(vec![
            Span::styled("[↑/↓]", Style::default().fg(Color::Yellow)),
            Span::raw(" Scroll | "),
            Span::styled("[Esc]", Style::default().fg(Color::Yellow)),
            Span::raw(" Close"),
        ]));
        f.render_widget(help, chunks[2]);
    }

    fn render_about_view(&mut self, f: &mut Frame) {
        let area = centered_rect(50, 40, f.area());
        f.render_widget(Clear, area);

        let block = Block::default()
            .title("About")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let text = vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                "Net Monitor",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Version: ", Style::default().fg(Color::Yellow)),
                Span::raw(env!("CARGO_PKG_VERSION")),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Description: ", Style::default().fg(Color::Yellow)),
                Span::raw("A network monitoring tool with a TUI interface"),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("License: ", Style::default().fg(Color::Yellow)),
                Span::raw("MIT OR Apache-2.0"),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Repository: ", Style::default().fg(Color::Yellow)),
                Span::raw("https://github.com/casey-mccarthy/net-monitor"),
            ]),
            Line::from(""),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Press Esc or q to close",
                Style::default()
                    .fg(Color::Gray)
                    .add_modifier(Modifier::ITALIC),
            )]),
        ];

        let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: true });

        f.render_widget(paragraph, area);
    }

    fn render_help_view(&mut self, f: &mut Frame) {
        let area = centered_rect(60, 70, f.area());
        f.render_widget(Clear, area);

        let (title, help_text) = match self.previous_state {
            Some(AppState::Main) | None => (
                "Help - Main View",
                vec![
                    Line::from(vec![
                        Span::styled("m", Style::default().fg(Color::Yellow)),
                        Span::raw(" - Start/Stop monitoring"),
                    ]),
                    Line::from(vec![
                        Span::styled("a", Style::default().fg(Color::Yellow)),
                        Span::raw(" - Add new node"),
                    ]),
                    Line::from(vec![
                        Span::styled("e", Style::default().fg(Color::Yellow)),
                        Span::raw(" - Edit selected node"),
                    ]),
                    Line::from(vec![
                        Span::styled("d", Style::default().fg(Color::Yellow)),
                        Span::raw(" - Delete selected node"),
                    ]),
                    Line::from(vec![
                        Span::styled("h", Style::default().fg(Color::Yellow)),
                        Span::raw(" - View status history"),
                    ]),
                    Line::from(vec![
                        Span::styled("i", Style::default().fg(Color::Yellow)),
                        Span::raw(" - Import nodes from JSON"),
                    ]),
                    Line::from(vec![
                        Span::styled("x", Style::default().fg(Color::Yellow)),
                        Span::raw(" - Export nodes to JSON"),
                    ]),
                    Line::from(vec![
                        Span::styled("r", Style::default().fg(Color::Yellow)),
                        Span::raw(" - Reorder nodes"),
                    ]),
                    Line::from(vec![
                        Span::styled("↑/↓", Style::default().fg(Color::Yellow)),
                        Span::raw(" - Navigate nodes"),
                    ]),
                    Line::from(vec![
                        Span::styled("Enter", Style::default().fg(Color::Yellow)),
                        Span::raw(" - Connect to selected node"),
                    ]),
                    Line::from(vec![
                        Span::styled("?", Style::default().fg(Color::Yellow)),
                        Span::raw(" - Show this help"),
                    ]),
                    Line::from(vec![
                        Span::styled("q", Style::default().fg(Color::Yellow)),
                        Span::raw(" - Quit application"),
                    ]),
                ],
            ),
            Some(AppState::AddNode) | Some(AppState::EditNode) => (
                "Help - Node Form",
                vec![
                    Line::from(vec![
                        Span::styled("Tab", Style::default().fg(Color::Yellow)),
                        Span::raw(" - Move to next field"),
                    ]),
                    Line::from(vec![
                        Span::styled("Shift+Tab", Style::default().fg(Color::Yellow)),
                        Span::raw(" - Move to previous field"),
                    ]),
                    Line::from(vec![
                        Span::styled("←/→/Space", Style::default().fg(Color::Yellow)),
                        Span::raw(" - Change monitor type"),
                    ]),
                    Line::from(vec![
                        Span::styled("Enter", Style::default().fg(Color::Yellow)),
                        Span::raw(" - Save node"),
                    ]),
                    Line::from(vec![
                        Span::styled("Esc", Style::default().fg(Color::Yellow)),
                        Span::raw(" - Cancel"),
                    ]),
                ],
            ),
            Some(AppState::ViewHistory) => (
                "Help - Status History",
                vec![
                    Line::from(vec![Span::raw(
                        "View node status change history and uptime statistics.",
                    )]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("Esc/q", Style::default().fg(Color::Yellow)),
                        Span::raw(" - Return to main view"),
                    ]),
                ],
            ),
            Some(AppState::ImportModeSelect) => (
                "Help - Import Mode",
                vec![
                    Line::from(vec![Span::raw("Choose how to handle imported nodes.")]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("Up/Down", Style::default().fg(Color::Yellow)),
                        Span::raw(" - Select mode"),
                    ]),
                    Line::from(vec![
                        Span::styled("Enter", Style::default().fg(Color::Yellow)),
                        Span::raw(" - Confirm selection"),
                    ]),
                    Line::from(vec![
                        Span::styled("Esc", Style::default().fg(Color::Yellow)),
                        Span::raw(" - Cancel"),
                    ]),
                ],
            ),
            Some(AppState::ConfirmDelete) => (
                "Help - Confirm Delete",
                vec![
                    Line::from(vec![Span::raw("Confirm deletion of the selected item.")]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("Y", Style::default().fg(Color::Yellow)),
                        Span::raw(" - Confirm deletion"),
                    ]),
                    Line::from(vec![
                        Span::styled("N/Esc", Style::default().fg(Color::Yellow)),
                        Span::raw(" - Cancel"),
                    ]),
                ],
            ),
            Some(AppState::Reorder) => (
                "Help - Reorder Mode",
                vec![
                    Line::from(vec![Span::raw(
                        "Rearrange nodes by moving the selected node up or down.",
                    )]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("↑/↓", Style::default().fg(Color::Yellow)),
                        Span::raw(" - Move node up/down"),
                    ]),
                    Line::from(vec![
                        Span::styled("R", Style::default().fg(Color::Yellow)),
                        Span::raw(" - Confirm new order"),
                    ]),
                    Line::from(vec![
                        Span::styled("Esc", Style::default().fg(Color::Yellow)),
                        Span::raw(" - Cancel and restore original order"),
                    ]),
                ],
            ),
            Some(AppState::About) => (
                "Help - About",
                vec![
                    Line::from(vec![Span::raw(
                        "View application information and version details.",
                    )]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("Esc/q", Style::default().fg(Color::Yellow)),
                        Span::raw(" - Close about view"),
                    ]),
                ],
            ),
            Some(AppState::Help) => (
                "Help",
                vec![Line::from(vec![Span::raw("You're already viewing help!")])],
            ),
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let mut final_help_text = help_text;
        final_help_text.push(Line::from(""));
        final_help_text.push(Line::from(vec![Span::styled(
            "Press Esc, q, or ? to close this help",
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::ITALIC),
        )]));

        let paragraph = Paragraph::new(final_help_text)
            .block(block)
            .wrap(Wrap { trim: true });

        f.render_widget(paragraph, area);
    }

    fn render_confirm_delete(&mut self, f: &mut Frame) {
        let area = centered_rect(60, 25, f.area());
        f.render_widget(Clear, area);

        let block = Block::default()
            .title("Confirm Delete")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Red));

        // Determine what we're deleting and get its name
        let (item_type, item_name) = if let Some(index) = self.delete_node_index {
            let name = self
                .nodes
                .get(index)
                .map(|n| n.name.as_str())
                .unwrap_or("Unknown");
            ("node", name)
        } else {
            ("item", "Unknown")
        };

        let text = vec![
            Line::from(vec![
                Span::raw("Are you sure you want to delete this "),
                Span::styled(
                    item_type,
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("?"),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                item_name,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from("This action cannot be undone."),
            Line::from(""),
            Line::from(vec![
                Span::raw("Press "),
                Span::styled(
                    "Y",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" to confirm or "),
                Span::styled(
                    "N",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                Span::raw("/"),
                Span::styled(
                    "Esc",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" to cancel"),
            ]),
        ];

        let paragraph = Paragraph::new(text)
            .block(block)
            .alignment(Alignment::Center);

        f.render_widget(paragraph, area);
    }

    fn render_import_mode_select(&mut self, f: &mut Frame) {
        let area = centered_rect(60, 30, f.area());
        f.render_widget(Clear, area);

        let filename = self
            .import_file_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let block = Block::default()
            .title("Import Nodes")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let options = [
            (
                "Import & Skip Conflicts",
                "Skip nodes whose name matches an existing node",
            ),
            (
                "Clear & Import All",
                "Delete all existing nodes, then import everything",
            ),
        ];

        let mut text = vec![
            Line::from(vec![
                Span::raw("File: "),
                Span::styled(&filename, Style::default().fg(Color::Green)),
            ]),
            Line::from(""),
            Line::from("Select import mode:"),
            Line::from(""),
        ];

        for (i, (label, desc)) in options.iter().enumerate() {
            let marker = if i == self.import_mode_selected {
                ">"
            } else {
                " "
            };
            let style = if i == self.import_mode_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            text.push(Line::from(Span::styled(
                format!(" {} {}", marker, label),
                style,
            )));
            text.push(Line::from(Span::styled(
                format!("   {}", desc),
                Style::default().fg(Color::DarkGray),
            )));
            text.push(Line::from(""));
        }

        text.push(Line::from(vec![
            Span::styled("[Up/Down]", Style::default().fg(Color::Yellow)),
            Span::raw(" Select  "),
            Span::styled("[Enter]", Style::default().fg(Color::Yellow)),
            Span::raw(" Confirm  "),
            Span::styled("[Esc]", Style::default().fg(Color::Yellow)),
            Span::raw(" Cancel"),
        ]));

        let paragraph = Paragraph::new(text).block(block);
        f.render_widget(paragraph, area);
    }

    // Input handlers continue in next part...

    fn handle_main_input(&mut self, key: KeyCode, modifiers: KeyModifiers) -> Result<bool> {
        // Reset the input timer to show selection highlight
        self.last_input_time = Some(Instant::now());

        match key {
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                if modifiers.contains(KeyModifiers::CONTROL) {
                    return Ok(true); // Quit
                }
                return Ok(true); // Quit
            }
            KeyCode::Char('m') | KeyCode::Char('M') => {
                self.toggle_monitoring();
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                self.node_form = NodeForm::default();
                self.state = AppState::AddNode;
            }
            KeyCode::Char('e') | KeyCode::Char('E') => {
                if let Some(selected) = self.table_state.selected() {
                    if let Some(node) = self.nodes.get(selected).cloned() {
                        self.node_form = NodeForm::from_node(&node);
                        self.editing_node_id = node.id;
                        self.state = AppState::EditNode;
                    }
                }
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                if let Some(selected) = self.table_state.selected() {
                    self.delete_node_index = Some(selected);
                    self.state = AppState::ConfirmDelete;
                }
            }
            KeyCode::Char('h') | KeyCode::Char('H') => {
                if let Some(selected) = self.table_state.selected() {
                    if let Some(node) = self.nodes.get(selected) {
                        if let Some(node_id) = node.id {
                            self.viewing_history_node_id = Some(node_id);
                            self.load_status_history(node_id);
                            self.state = AppState::ViewHistory;
                        }
                    }
                }
            }
            KeyCode::Char('i') | KeyCode::Char('I') => {
                self.deferred_action = Some(DeferredAction::ShowImportDialog);
            }
            KeyCode::Char('x') | KeyCode::Char('X') => {
                self.deferred_action = Some(DeferredAction::ShowExportDialog);
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                if self.nodes.len() > 1 {
                    if let Some(selected) = self.table_state.selected() {
                        self.reorder_original_index = Some(selected);
                        self.reorder_original_order =
                            Some(self.nodes.iter().map(|n| n.id).collect());
                        self.state = AppState::Reorder;
                    }
                }
            }
            KeyCode::Char('b') | KeyCode::Char('B') => {
                self.state = AppState::About;
            }
            KeyCode::Char('?') => {
                self.previous_state = Some(AppState::Main);
                self.state = AppState::Help;
            }
            KeyCode::Down => {
                if let Some(i) = next_selection(self.table_state.selected(), self.nodes.len()) {
                    self.table_state.select(Some(i));
                }
            }
            KeyCode::Up => {
                if let Some(i) = previous_selection(self.table_state.selected(), self.nodes.len()) {
                    self.table_state.select(Some(i));
                }
            }
            KeyCode::Enter => {
                if let Some(selected) = self.table_state.selected() {
                    if let Some(node) = self.nodes.get(selected).cloned() {
                        self.connect_to_node(&node);
                    }
                }
            }
            _ => {}
        }
        Ok(false)
    }

    fn handle_reorder_input(&mut self, key: KeyCode) -> bool {
        self.last_input_time = Some(Instant::now());

        match key {
            KeyCode::Char('r') | KeyCode::Char('R') => {
                // Confirm: persist order and return to Main
                self.persist_display_order();
                self.reorder_original_index = None;
                self.reorder_original_order = None;
                return true;
            }
            KeyCode::Esc => {
                // Cancel: put the nodes back in their original order. Only the
                // order is restored; the nodes themselves keep the status and
                // latency updates that arrived while reorder mode was open.
                if let Some(original_order) = self.reorder_original_order.take() {
                    let current = std::mem::take(&mut self.nodes);
                    self.nodes = restore_node_order(&original_order, current);
                }
                if let Some(original_index) = self.reorder_original_index.take() {
                    self.table_state.select(Some(original_index));
                }
                return true;
            }
            KeyCode::Down => {
                if let Some(selected) = self.table_state.selected() {
                    if selected + 1 < self.nodes.len() {
                        self.nodes.swap(selected, selected + 1);
                        self.table_state.select(Some(selected + 1));
                    }
                }
            }
            KeyCode::Up => {
                if let Some(selected) = self.table_state.selected() {
                    if selected > 0 {
                        self.nodes.swap(selected, selected - 1);
                        self.table_state.select(Some(selected - 1));
                    }
                }
            }
            KeyCode::Char('?') => {
                self.previous_state = Some(AppState::Reorder);
                self.state = AppState::Help;
            }
            _ => {}
        }
        false
    }

    fn persist_display_order(&self) {
        let order: Vec<(i64, i64)> = self
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(i, node)| node.id.map(|id| (id, i as i64)))
            .collect();

        if let Err(e) = self.database.update_node_display_orders(&order) {
            error!("Failed to persist display order: {}", e);
        }
    }

    fn handle_node_form_input(&mut self, key: KeyCode, _modifiers: KeyModifiers) -> bool {
        match key {
            KeyCode::Esc => return true,
            KeyCode::Enter => {
                // Only close the form when the save succeeded; otherwise the
                // user would lose everything they typed and just see a brief
                // error message.
                let saved = if self.state == AppState::AddNode {
                    self.add_node_from_form()
                } else {
                    self.update_node_from_form()
                };
                return saved;
            }
            KeyCode::Tab => {
                self.node_form.current_field =
                    (self.node_form.current_field + 1) % self.node_form.get_field_count();
            }
            KeyCode::BackTab => {
                if self.node_form.current_field == 0 {
                    self.node_form.current_field = self.node_form.get_field_count() - 1;
                } else {
                    self.node_form.current_field -= 1;
                }
            }
            KeyCode::Left | KeyCode::Right => {
                // Handle arrow keys for Monitor Type field
                if self.node_form.current_field == 2 {
                    self.cycle_monitor_type(key == KeyCode::Right);
                }
            }
            KeyCode::Char('?') => {
                self.previous_state = Some(self.state);
                self.state = AppState::Help;
                return false;
            }
            KeyCode::Char(c) => {
                self.add_char_to_form_field(c);
            }
            KeyCode::Backspace => {
                self.remove_char_from_form_field();
            }
            _ => {}
        }
        false
    }

    fn handle_confirm_delete_input(&mut self, key: KeyCode) -> bool {
        match key {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let Some(index) = self.delete_node_index.take() {
                    self.delete_node_at_index(index);
                }
                return true;
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.delete_node_index = None;
                return true;
            }
            _ => {}
        }
        false
    }

    fn handle_import_mode_input(&mut self, key: KeyCode) -> bool {
        match key {
            KeyCode::Esc => return true,
            KeyCode::Up => {
                if self.import_mode_selected > 0 {
                    self.import_mode_selected -= 1;
                }
            }
            KeyCode::Down => {
                if self.import_mode_selected < 1 {
                    self.import_mode_selected += 1;
                }
            }
            KeyCode::Enter => {
                if let Some(path) = self.import_file_path.take() {
                    if self.import_mode_selected == 0 {
                        self.import_nodes_skip_conflicts(&path);
                    } else {
                        self.import_nodes_clear_all(&path);
                    }
                }
                return true;
            }
            KeyCode::Char('?') => {
                self.previous_state = Some(self.state);
                self.state = AppState::Help;
                return false;
            }
            _ => {}
        }
        false
    }

    // Helper methods

    fn cycle_monitor_type(&mut self, forward: bool) {
        self.node_form.monitor_type = if forward {
            match self.node_form.monitor_type {
                MonitorTypeForm::Http => MonitorTypeForm::Ping,
                MonitorTypeForm::Ping => MonitorTypeForm::Tcp,
                MonitorTypeForm::Tcp => MonitorTypeForm::Http,
            }
        } else {
            match self.node_form.monitor_type {
                MonitorTypeForm::Http => MonitorTypeForm::Tcp,
                MonitorTypeForm::Tcp => MonitorTypeForm::Ping,
                MonitorTypeForm::Ping => MonitorTypeForm::Http,
            }
        };
    }

    fn add_char_to_form_field(&mut self, c: char) {
        let field = self.node_form.current_field;
        match field {
            0 => self.node_form.name.push(c),
            1 => self.node_form.monitoring_interval.push(c),
            2 => {
                // Cycle through monitor types with Space only
                if c == ' ' {
                    self.cycle_monitor_type(true);
                }
            }
            3 => match self.node_form.monitor_type {
                MonitorTypeForm::Http => self.node_form.http_url.push(c),
                MonitorTypeForm::Ping => self.node_form.ping_host.push(c),
                MonitorTypeForm::Tcp => self.node_form.tcp_host.push(c),
            },
            4 => match self.node_form.monitor_type {
                MonitorTypeForm::Http => self.node_form.http_expected_status.push(c),
                MonitorTypeForm::Ping => self.node_form.ping_count.push(c),
                MonitorTypeForm::Tcp => self.node_form.tcp_port.push(c),
            },
            5 => match self.node_form.monitor_type {
                MonitorTypeForm::Ping => self.node_form.ping_timeout.push(c),
                MonitorTypeForm::Tcp => self.node_form.tcp_timeout.push(c),
                _ => {}
            },
            _ => {}
        }
    }

    fn remove_char_from_form_field(&mut self) {
        let field = self.node_form.current_field;
        match field {
            0 => {
                self.node_form.name.pop();
            }
            1 => {
                self.node_form.monitoring_interval.pop();
            }
            2 => {} // Monitor type
            3 => match self.node_form.monitor_type {
                MonitorTypeForm::Http => {
                    self.node_form.http_url.pop();
                }
                MonitorTypeForm::Ping => {
                    self.node_form.ping_host.pop();
                }
                MonitorTypeForm::Tcp => {
                    self.node_form.tcp_host.pop();
                }
            },
            4 => match self.node_form.monitor_type {
                MonitorTypeForm::Http => {
                    self.node_form.http_expected_status.pop();
                }
                MonitorTypeForm::Ping => {
                    self.node_form.ping_count.pop();
                }
                MonitorTypeForm::Tcp => {
                    self.node_form.tcp_port.pop();
                }
            },
            5 => match self.node_form.monitor_type {
                MonitorTypeForm::Ping => {
                    self.node_form.ping_timeout.pop();
                }
                MonitorTypeForm::Tcp => {
                    self.node_form.tcp_timeout.pop();
                }
                _ => {}
            },
            _ => {}
        }
    }

    /// Saves the add-node form. Returns `true` if the node was created.
    fn add_node_from_form(&mut self) -> bool {
        match self.node_form.to_node_detail() {
            Ok(detail) => {
                let node = Node {
                    id: None,
                    name: self.node_form.name.clone(),
                    detail,
                    status: NodeStatus::Offline,
                    last_check: None,
                    response_time: None,
                    monitoring_interval: self.node_form.monitoring_interval.parse().unwrap_or(5),
                    consecutive_failures: 0,
                    max_check_attempts: crate::models::DEFAULT_MAX_CHECK_ATTEMPTS,
                    retry_interval: crate::models::DEFAULT_RETRY_INTERVAL,
                };

                match self.database.add_node(&node) {
                    Ok(id) => {
                        let mut new_node = node;
                        new_node.id = Some(id);

                        if let Some(handle) = &self.monitoring_handle {
                            let _ = handle
                                .config_tx
                                .send(NodeConfigUpdate::Add(new_node.clone()));
                        }

                        self.nodes.push(new_node);
                        self.set_status_message("Node added successfully");
                        true
                    }
                    Err(e) => {
                        self.set_status_message(format!("Error adding node: {}", e));
                        false
                    }
                }
            }
            Err(e) => {
                self.set_status_message(format!("Invalid data: {}", e));
                false
            }
        }
    }

    /// Saves the edit-node form. Returns `true` if the node was updated.
    fn update_node_from_form(&mut self) -> bool {
        let Some(node_id) = self.editing_node_id else {
            // Nothing is being edited; there is no form to keep open
            return true;
        };

        let detail = match self.node_form.to_node_detail() {
            Ok(detail) => detail,
            Err(e) => {
                self.set_status_message(format!("Invalid data: {}", e));
                return false;
            }
        };

        let Some(node) = self.nodes.iter_mut().find(|n| n.id == Some(node_id)) else {
            self.set_status_message("Node no longer exists");
            return true;
        };

        // Apply the edit to a copy first so a rejected save does not leave the
        // on-screen node out of sync with the database
        let mut updated = node.clone();
        updated.name = self.node_form.name.clone();
        updated.detail = detail;
        updated.monitoring_interval = self.node_form.monitoring_interval.parse().unwrap_or(5);

        if let Err(e) = self.database.update_node(&updated) {
            self.set_status_message(format!("Error updating node: {}", e));
            return false;
        }

        *node = updated;
        if let Some(handle) = &self.monitoring_handle {
            let _ = handle
                .config_tx
                .send(NodeConfigUpdate::Update(node.clone()));
        }
        self.set_status_message("Node updated successfully");
        true
    }

    fn delete_node_at_index(&mut self, index: usize) {
        if let Some(node) = self.nodes.get(index).cloned() {
            if let Some(id) = node.id {
                if self.database.delete_node(id).is_ok() {
                    if let Some(handle) = &self.monitoring_handle {
                        let _ = handle.config_tx.send(NodeConfigUpdate::Delete(id));
                    }
                    self.nodes.remove(index);
                    self.set_status_message("Node deleted");

                    // Adjust selection
                    if self.nodes.is_empty() {
                        self.table_state.select(None);
                    } else if index >= self.nodes.len() {
                        self.table_state.select(Some(self.nodes.len() - 1));
                    }
                } else {
                    self.set_status_message("Failed to delete node");
                }
            }
        }
    }

    fn connect_to_node(&mut self, node: &Node) {
        let target = node.detail.get_connection_target();
        let connection_type = node.detail.get_connection_type();

        match connection_type {
            crate::connection::ConnectionType::Http => {
                let http_strategy = crate::connection::HttpConnectionStrategy;
                match http_strategy.connect(&target) {
                    Ok(_) => {
                        self.set_status_message(format!("Opening {} in browser...", target));
                    }
                    Err(e) => {
                        self.set_status_message(format!("Failed to open in browser: {}", e));
                    }
                }
            }
            _ => {
                let ssh_strategy = crate::connection::SshConnectionStrategy::new();
                match ssh_strategy.connect(&target) {
                    Ok(_) => {
                        self.set_status_message(format!("Connecting to {} via SSH...", target));
                    }
                    Err(e) => {
                        self.set_status_message(format!("Failed to connect via SSH: {}", e));
                    }
                }
            }
        }
    }

    fn toggle_monitoring(&mut self) {
        if self.monitoring_handle.is_some() {
            let _ = self.stop_monitoring();
        } else {
            self.start_monitoring();
        }
    }

    fn start_monitoring(&mut self) {
        let handle = monitoring_engine::start_monitoring(
            self.database.clone(),
            self.nodes.clone(),
            self.update_tx.clone(),
        );
        self.monitoring_handle = Some(handle);
        self.set_status_message("Monitoring started");
    }

    fn stop_monitoring(&mut self) -> Result<()> {
        if let Some(handle) = self.monitoring_handle.take() {
            handle.stop_tx.send(())?;
            self.set_status_message("Monitoring stopped");
        }
        Ok(())
    }

    /// Reads and parses an import file.
    ///
    /// This has no side effects, so callers can validate the file before
    /// touching any existing nodes.
    fn read_import_file(path: &PathBuf) -> Result<Vec<NodeImport>> {
        let data = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("Failed to read import file: {}", e))?;
        serde_json::from_str::<Vec<NodeImport>>(&data)
            .map_err(|e| anyhow::anyhow!("Failed to parse import file: {}", e))
    }

    /// Builds a fresh, never-checked node from an import record.
    fn node_from_import(import: NodeImport) -> Node {
        Node {
            id: None,
            name: import.name,
            detail: import.detail,
            status: NodeStatus::Offline,
            last_check: None,
            response_time: None,
            monitoring_interval: import.monitoring_interval,
            consecutive_failures: 0,
            max_check_attempts: import.max_check_attempts,
            retry_interval: import.retry_interval,
        }
    }

    /// Persists an imported node, registers it with the monitoring engine and
    /// adds it to the node list. Returns `false` if the database rejected it.
    fn add_imported_node(&mut self, import: NodeImport) -> bool {
        let mut node = Self::node_from_import(import);
        match self.database.add_node(&node) {
            Ok(id) => {
                node.id = Some(id);
                if let Some(handle) = &self.monitoring_handle {
                    let _ = handle.config_tx.send(NodeConfigUpdate::Add(node.clone()));
                }
                self.nodes.push(node);
                true
            }
            Err(e) => {
                error!("Failed to import node '{}': {}", node.name, e);
                false
            }
        }
    }

    fn import_nodes_skip_conflicts(&mut self, path: &PathBuf) {
        let nodes_to_import = match Self::read_import_file(path) {
            Ok(nodes) => nodes,
            Err(e) => {
                self.set_status_message(e.to_string());
                return;
            }
        };

        // Names already present, plus names seen earlier in this file so that
        // duplicates within the import itself are also treated as conflicts.
        let mut existing_names: std::collections::HashSet<String> =
            self.nodes.iter().map(|n| n.name.clone()).collect();
        let mut imported = 0;
        let mut skipped = 0;
        let mut failed = 0;
        for import in nodes_to_import {
            if !existing_names.insert(import.name.clone()) {
                skipped += 1;
                continue;
            }
            if self.add_imported_node(import) {
                imported += 1;
            } else {
                failed += 1;
            }
        }

        let mut message = format!("Imported {} nodes, skipped {} conflicts", imported, skipped);
        if failed > 0 {
            message.push_str(&format!(", {} failed", failed));
        }
        self.set_status_message(message);
    }

    fn import_nodes_clear_all(&mut self, path: &PathBuf) {
        // Validate the file before destroying anything: an unreadable or
        // malformed file must leave the existing nodes untouched.
        let nodes_to_import = match Self::read_import_file(path) {
            Ok(nodes) => nodes,
            Err(e) => {
                self.set_status_message(format!("{} (existing nodes were kept)", e));
                return;
            }
        };

        // Clear from database first; if that fails nothing else has changed yet
        if let Err(e) = self.database.delete_all_nodes() {
            self.set_status_message(format!("Failed to clear nodes: {}", e));
            return;
        }

        // Remove all existing nodes from the monitoring engine
        for node in &self.nodes {
            if let (Some(id), Some(handle)) = (node.id, &self.monitoring_handle) {
                let _ = handle.config_tx.send(NodeConfigUpdate::Delete(id));
            }
        }

        // Clear in-memory state
        self.nodes.clear();
        self.table_state.select(None);

        let mut imported = 0;
        let mut failed = 0;
        for import in nodes_to_import {
            if self.add_imported_node(import) {
                imported += 1;
            } else {
                failed += 1;
            }
        }
        if !self.nodes.is_empty() {
            self.table_state.select(Some(0));
        }

        let mut message = format!("Cleared all nodes, imported {}", imported);
        if failed > 0 {
            message.push_str(&format!(", {} failed", failed));
        }
        self.set_status_message(message);
    }

    fn export_nodes_to_path(&mut self, path: &PathBuf) {
        let nodes_to_export: Vec<NodeImport> = self
            .nodes
            .iter()
            .map(|node| NodeImport {
                name: node.name.clone(),
                detail: node.detail.clone(),
                monitoring_interval: node.monitoring_interval,
                max_check_attempts: node.max_check_attempts,
                retry_interval: node.retry_interval,
            })
            .collect();

        match serde_json::to_string_pretty(&nodes_to_export) {
            Ok(data) => {
                if let Err(e) = std::fs::write(path, data) {
                    self.set_status_message(format!("Failed to write export file: {}", e));
                } else {
                    self.set_status_message("Nodes exported successfully");
                }
            }
            Err(e) => {
                self.set_status_message(format!("Failed to serialize nodes: {}", e));
            }
        }
    }

    /// Number of rows in the history table: the "Current" state row (shown
    /// whenever a node is being viewed) plus one row per recorded change.
    /// Must match what `render_history_view` draws.
    fn history_row_count(&self) -> usize {
        usize::from(self.viewing_history_node_id.is_some()) + self.status_changes.len()
    }

    fn load_status_history(&mut self, node_id: i64) {
        match self.database.get_status_changes(node_id, Some(50)) {
            Ok(changes) => {
                self.status_changes = changes;
                // Start on the first row (the current state)
                if self.history_row_count() > 0 {
                    self.history_table_state.select(Some(0));
                } else {
                    self.history_table_state.select(None);
                }
            }
            Err(e) => {
                error!("Failed to load status history: {}", e);
                self.status_changes.clear();
                self.history_table_state.select(None);
            }
        }
    }

    fn set_status_message(&mut self, message: impl Into<String>) {
        self.status_message = Some((message.into(), Instant::now()));
    }
}

/// Latency to show for a node. Only an Online node has a current response
/// time; a Degraded or Offline node's last check failed, so any stored value
/// is stale (or, for data written before this rule, the time spent waiting
/// for the failure) and must not be shown as latency.
fn current_latency(node: &Node) -> Option<u64> {
    match node.status {
        NodeStatus::Online => node.response_time,
        NodeStatus::Degraded | NodeStatus::Offline => None,
    }
}

fn latency_text(node: &Node) -> String {
    match current_latency(node) {
        Some(ms) => format!("{}ms", ms),
        None => "—".to_string(),
    }
}

fn latency_color(node: &Node) -> Color {
    match current_latency(node) {
        Some(ms) if ms < 100 => Color::Green,
        Some(ms) if ms < 300 => Color::Yellow,
        Some(_) => Color::Red,
        None => Color::DarkGray,
    }
}

/// Moves a table selection down one row, clamped to the last of `row_count` rows.
/// With no current selection the first row is selected. Does nothing for an empty table.
fn select_next_row(state: &mut TableState, row_count: usize) {
    let Some(last) = row_count.checked_sub(1) else {
        state.select(None);
        return;
    };
    let next = match state.selected() {
        Some(i) => (i + 1).min(last),
        None => 0,
    };
    state.select(Some(next));
}

/// Moves a table selection up one row, stopping at the first row.
/// With no current selection the first row is selected. Does nothing for an empty table.
fn select_previous_row(state: &mut TableState, row_count: usize) {
    if row_count == 0 {
        state.select(None);
        return;
    }
    let previous = state.selected().map_or(0, |i| i.saturating_sub(1));
    state.select(Some(previous));
}

/// Reorders `nodes` to follow `original_order` (a list of node ids). Nodes whose
/// id is not in the list keep their relative order and are appended at the end.
fn restore_node_order(original_order: &[Option<i64>], mut nodes: Vec<Node>) -> Vec<Node> {
    let mut ordered = Vec::with_capacity(nodes.len());
    for id in original_order {
        if let Some(pos) = nodes.iter().position(|n| &n.id == id) {
            ordered.push(nodes.remove(pos));
        }
    }
    ordered.extend(nodes);
    ordered
}

fn format_duration(duration_ms: i64) -> String {
    let seconds = duration_ms / 1000;
    let minutes = seconds / 60;
    let hours = minutes / 60;
    let days = hours / 24;

    if days > 0 {
        format!("{}d {}h", days, hours % 24)
    } else if hours > 0 {
        format!("{}h {}m", hours, minutes % 60)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, seconds % 60)
    } else {
        format!("{}s", seconds)
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use crate::database::Database;
    use tempfile::tempdir;

    // ============================================================================
    // NetworkMonitorTui Integration Tests
    // ============================================================================

    /// Builds a TUI backed by a temporary database.
    fn tui_with_temp_db() -> (tempfile::TempDir, NetworkMonitorTui) {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let database = Database::new(&db_path).expect("Failed to create database");
        let tui = NetworkMonitorTui::new(database).expect("Failed to create TUI");
        (temp_dir, tui)
    }

    #[test]
    fn test_read_import_file_rejects_missing_and_malformed_files() {
        let temp_dir = tempdir().unwrap();

        let missing = temp_dir.path().join("does-not-exist.json");
        assert!(NetworkMonitorTui::read_import_file(&missing).is_err());

        let malformed = temp_dir.path().join("bad.json");
        std::fs::write(&malformed, "{ this is not json").unwrap();
        assert!(NetworkMonitorTui::read_import_file(&malformed).is_err());

        let wrong_shape = temp_dir.path().join("wrong.json");
        std::fs::write(&wrong_shape, r#"{"name": "single object, not a list"}"#).unwrap();
        assert!(NetworkMonitorTui::read_import_file(&wrong_shape).is_err());
    }

    #[test]
    fn test_clear_all_import_keeps_existing_nodes_when_file_is_invalid() {
        let (temp_dir, mut tui) = tui_with_temp_db();

        let mut node = Node {
            id: None,
            name: "Existing".to_string(),
            detail: MonitorDetail::Http {
                url: "https://example.com".to_string(),
                expected_status: 200,
            },
            status: NodeStatus::Online,
            last_check: None,
            response_time: None,
            monitoring_interval: 60,
            consecutive_failures: 0,
            max_check_attempts: 3,
            retry_interval: 15,
        };
        node.id = Some(tui.database.add_node(&node).unwrap());
        tui.nodes.push(node);
        tui.table_state.select(Some(0));

        let bad_file = temp_dir.path().join("bad.json");
        std::fs::write(&bad_file, "not json at all").unwrap();
        tui.import_nodes_clear_all(&bad_file);

        // Nothing was destroyed, on screen or on disk
        assert_eq!(tui.nodes.len(), 1);
        assert_eq!(tui.nodes[0].name, "Existing");
        assert_eq!(tui.database.get_all_nodes().unwrap().len(), 1);
        assert_eq!(tui.table_state.selected(), Some(0));

        let missing = temp_dir.path().join("missing.json");
        tui.import_nodes_clear_all(&missing);
        assert_eq!(tui.nodes.len(), 1);
        assert_eq!(tui.database.get_all_nodes().unwrap().len(), 1);
    }

    #[test]
    fn test_clear_all_import_replaces_nodes_with_valid_file() {
        let (temp_dir, mut tui) = tui_with_temp_db();

        let mut node = Node {
            id: None,
            name: "Old".to_string(),
            detail: MonitorDetail::Http {
                url: "https://old.example.com".to_string(),
                expected_status: 200,
            },
            status: NodeStatus::Online,
            last_check: None,
            response_time: None,
            monitoring_interval: 60,
            consecutive_failures: 0,
            max_check_attempts: 3,
            retry_interval: 15,
        };
        node.id = Some(tui.database.add_node(&node).unwrap());
        tui.nodes.push(node);

        let imports = vec![
            NodeImport {
                name: "New A".to_string(),
                detail: MonitorDetail::Tcp {
                    host: "a.example.com".to_string(),
                    port: 22,
                    timeout: 5,
                },
                monitoring_interval: 30,
                max_check_attempts: 3,
                retry_interval: 15,
            },
            NodeImport {
                name: "New B".to_string(),
                detail: MonitorDetail::Ping {
                    host: "10.0.0.1".to_string(),
                    count: 1,
                    timeout: 5,
                },
                monitoring_interval: 30,
                max_check_attempts: 3,
                retry_interval: 15,
            },
        ];
        let good_file = temp_dir.path().join("good.json");
        std::fs::write(&good_file, serde_json::to_string(&imports).unwrap()).unwrap();

        tui.import_nodes_clear_all(&good_file);

        let names: Vec<&str> = tui.nodes.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(names, vec!["New A", "New B"]);
        assert_eq!(tui.database.get_all_nodes().unwrap().len(), 2);
        assert_eq!(tui.table_state.selected(), Some(0));
    }

    #[test]
    fn test_skip_conflicts_import_skips_duplicates_within_file() {
        let (temp_dir, mut tui) = tui_with_temp_db();

        let make = |name: &str| NodeImport {
            name: name.to_string(),
            detail: MonitorDetail::Tcp {
                host: "host.example.com".to_string(),
                port: 22,
                timeout: 5,
            },
            monitoring_interval: 30,
            max_check_attempts: 3,
            retry_interval: 15,
        };
        let imports = vec![make("Dup"), make("Dup"), make("Unique")];
        let file = temp_dir.path().join("dups.json");
        std::fs::write(&file, serde_json::to_string(&imports).unwrap()).unwrap();

        tui.import_nodes_skip_conflicts(&file);

        let names: Vec<&str> = tui.nodes.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(names, vec!["Dup", "Unique"]);
        assert_eq!(tui.database.get_all_nodes().unwrap().len(), 2);
    }

    fn latency_test_node(status: NodeStatus, response_time: Option<u64>) -> Node {
        Node {
            id: Some(1),
            name: "n".to_string(),
            detail: MonitorDetail::Http {
                url: "https://example.com".to_string(),
                expected_status: 200,
            },
            status,
            last_check: None,
            response_time,
            monitoring_interval: 60,
            consecutive_failures: 0,
            max_check_attempts: 3,
            retry_interval: 15,
        }
    }

    fn reorder_test_node(id: i64, status: NodeStatus, response_time: Option<u64>) -> Node {
        Node {
            id: Some(id),
            name: format!("node-{}", id),
            detail: MonitorDetail::Http {
                url: format!("https://{}.example.com", id),
                expected_status: 200,
            },
            status,
            last_check: None,
            response_time,
            monitoring_interval: 60,
            consecutive_failures: 0,
            max_check_attempts: 3,
            retry_interval: 15,
        }
    }

    #[test]
    fn test_latency_shown_only_for_online_nodes() {
        let online = latency_test_node(NodeStatus::Online, Some(42));
        assert_eq!(latency_text(&online), "42ms");
        assert_eq!(latency_color(&online), Color::Green);

        // A refused connection fails in a couple of ms; that is not latency
        let offline_fast = latency_test_node(NodeStatus::Offline, Some(2));
        assert_eq!(latency_text(&offline_fast), "—");
        assert_eq!(latency_color(&offline_fast), Color::DarkGray);

        // A timeout waits the full timeout; that is not latency either
        let offline_timeout = latency_test_node(NodeStatus::Offline, Some(30000));
        assert_eq!(latency_text(&offline_timeout), "—");

        let degraded = latency_test_node(NodeStatus::Degraded, Some(150));
        assert_eq!(latency_text(&degraded), "—");
        assert_eq!(latency_color(&degraded), Color::DarkGray);

        let online_unknown = latency_test_node(NodeStatus::Online, None);
        assert_eq!(latency_text(&online_unknown), "—");
    }

    #[test]
    fn test_latency_color_thresholds() {
        assert_eq!(
            latency_color(&latency_test_node(NodeStatus::Online, Some(99))),
            Color::Green
        );
        assert_eq!(
            latency_color(&latency_test_node(NodeStatus::Online, Some(100))),
            Color::Yellow
        );
        assert_eq!(
            latency_color(&latency_test_node(NodeStatus::Online, Some(300))),
            Color::Red
        );
    }

    #[test]
    fn test_apply_runtime_state_keeps_local_configuration() {
        let mut local = Node {
            id: Some(7),
            name: "Edited name".to_string(),
            detail: MonitorDetail::Http {
                url: "https://edited.example.com".to_string(),
                expected_status: 200,
            },
            status: NodeStatus::Offline,
            last_check: None,
            response_time: None,
            monitoring_interval: 120,
            consecutive_failures: 2,
            max_check_attempts: 3,
            retry_interval: 15,
        };

        // The engine reports a check result computed from the pre-edit configuration
        let from_engine = Node {
            id: Some(7),
            name: "Old name".to_string(),
            detail: MonitorDetail::Http {
                url: "https://old.example.com".to_string(),
                expected_status: 200,
            },
            status: NodeStatus::Online,
            last_check: Some(chrono::Utc::now()),
            response_time: Some(31),
            monitoring_interval: 5,
            consecutive_failures: 0,
            max_check_attempts: 3,
            retry_interval: 15,
        };

        NetworkMonitorTui::apply_runtime_state(&mut local, &from_engine);

        assert_eq!(local.status, NodeStatus::Online);
        assert_eq!(local.last_check, from_engine.last_check);
        assert_eq!(local.response_time, Some(31));
        assert_eq!(local.consecutive_failures, 0);

        assert_eq!(local.name, "Edited name");
        assert_eq!(local.monitoring_interval, 120);
        assert_eq!(
            local.detail,
            MonitorDetail::Http {
                url: "https://edited.example.com".to_string(),
                expected_status: 200,
            }
        );
    }

    #[test]
    fn test_enter_keeps_node_form_open_when_data_is_invalid() {
        let temp_dir = tempdir().unwrap();
        let database = Database::new(temp_dir.path().join("test.db")).unwrap();
        let mut tui = NetworkMonitorTui::new(database).unwrap();

        tui.state = AppState::AddNode;
        tui.node_form = NodeForm::default();
        tui.node_form.name = "Typed a long name".to_string();
        tui.node_form.monitor_type = MonitorTypeForm::Tcp;
        tui.node_form.tcp_host = "db.example.com".to_string();
        tui.node_form.tcp_port = String::new(); // invalid: empty port

        let close = tui.handle_node_form_input(KeyCode::Enter, KeyModifiers::NONE);

        assert!(!close, "form must stay open after a validation failure");
        assert_eq!(tui.node_form.name, "Typed a long name");
        assert_eq!(tui.node_form.tcp_host, "db.example.com");
        assert!(tui.nodes.is_empty());
        assert!(tui.status_message.is_some());

        // Fixing the field and pressing Enter again saves and closes
        tui.node_form.tcp_port = "5432".to_string();
        let close = tui.handle_node_form_input(KeyCode::Enter, KeyModifiers::NONE);
        assert!(close);
        assert_eq!(tui.nodes.len(), 1);
        assert_eq!(tui.nodes[0].name, "Typed a long name");
    }

    #[test]
    fn test_select_next_row_reaches_last_row() {
        let mut state = TableState::default();
        // Table with a "Current" row plus 3 changes = 4 rows
        select_next_row(&mut state, 4);
        assert_eq!(state.selected(), Some(0));
        select_next_row(&mut state, 4);
        select_next_row(&mut state, 4);
        select_next_row(&mut state, 4);
        assert_eq!(state.selected(), Some(3), "last row must be reachable");
        select_next_row(&mut state, 4);
        assert_eq!(state.selected(), Some(3), "must clamp at the last row");
    }

    #[test]
    fn test_select_previous_row_stops_at_first_row() {
        let mut state = TableState::default();
        state.select(Some(2));
        select_previous_row(&mut state, 4);
        assert_eq!(state.selected(), Some(1));
        select_previous_row(&mut state, 4);
        select_previous_row(&mut state, 4);
        assert_eq!(state.selected(), Some(0));
    }

    #[test]
    fn test_row_navigation_on_empty_table_selects_nothing() {
        let mut state = TableState::default();
        state.select(Some(1));
        select_next_row(&mut state, 0);
        assert_eq!(state.selected(), None);
        state.select(Some(1));
        select_previous_row(&mut state, 0);
        assert_eq!(state.selected(), None);
    }

    #[test]
    fn test_row_navigation_with_only_current_row() {
        let mut state = TableState::default();
        select_next_row(&mut state, 1);
        assert_eq!(state.selected(), Some(0));
        select_next_row(&mut state, 1);
        assert_eq!(state.selected(), Some(0));
        select_previous_row(&mut state, 1);
        assert_eq!(state.selected(), Some(0));
    }

    #[test]
    fn test_restore_node_order_keeps_live_state() {
        let original_order = vec![Some(1), Some(2), Some(3)];

        // While reorder mode was open the user moved node 3 to the top and the
        // engine reported node 2 going offline
        let current = vec![
            reorder_test_node(3, NodeStatus::Online, Some(12)),
            reorder_test_node(1, NodeStatus::Online, Some(20)),
            reorder_test_node(2, NodeStatus::Offline, Some(30000)),
        ];

        let restored = restore_node_order(&original_order, current);

        let ids: Vec<Option<i64>> = restored.iter().map(|n| n.id).collect();
        assert_eq!(ids, original_order, "order must be restored");
        assert_eq!(
            restored[1].status,
            NodeStatus::Offline,
            "cancel must not revert a status update that arrived during reorder"
        );
        assert_eq!(restored[1].response_time, Some(30000));
    }

    #[test]
    fn test_restore_node_order_appends_unknown_nodes() {
        let original_order = vec![Some(2), Some(1)];
        let current = vec![
            reorder_test_node(1, NodeStatus::Online, None),
            reorder_test_node(9, NodeStatus::Online, None),
            reorder_test_node(2, NodeStatus::Online, None),
        ];

        let restored = restore_node_order(&original_order, current);
        let ids: Vec<Option<i64>> = restored.iter().map(|n| n.id).collect();
        assert_eq!(ids, vec![Some(2), Some(1), Some(9)]);
    }

    #[test]
    fn test_restore_node_order_ignores_missing_ids() {
        let original_order = vec![Some(5), Some(1)];
        let current = vec![reorder_test_node(1, NodeStatus::Online, None)];

        let restored = restore_node_order(&original_order, current);
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].id, Some(1));
    }

    #[test]
    fn test_network_monitor_tui_initialization() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let database = Database::new(&db_path).expect("Failed to create database");

        let tui = NetworkMonitorTui::new(database).expect("TUI should initialize");
        // Verify monitoring started automatically
        assert!(tui.monitoring_handle.is_some());
        assert_eq!(tui.state, AppState::Main);
        assert!(tui.nodes.is_empty());
    }

    #[test]
    fn test_network_monitor_tui_with_nodes() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test_nodes.db");
        let database = Database::new(&db_path).expect("Failed to create database");

        // Add a test node
        let node = Node {
            id: None,
            name: "Test HTTP Node".to_string(),
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

        database.add_node(&node).expect("Failed to add node");

        // Create TUI
        let tui = NetworkMonitorTui::new(database).expect("TUI should initialize");
        assert_eq!(tui.nodes.len(), 1);
        assert_eq!(tui.nodes[0].name, "Test HTTP Node");
        // Table should have first row selected
        assert_eq!(tui.table_state.selected(), Some(0));
    }

    #[test]
    fn test_set_status_message() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("status_msg.db");
        let database = Database::new(&db_path).expect("Failed to create database");

        if let Ok(mut tui) = NetworkMonitorTui::new(database) {
            // Test setting status message
            tui.set_status_message("Test message");
            assert!(tui.status_message.is_some());

            if let Some((msg, _timestamp)) = &tui.status_message {
                assert_eq!(msg, "Test message");
            }

            // Test setting another message
            tui.set_status_message(String::from("Another message"));
            assert!(tui.status_message.is_some());

            if let Some((msg, _timestamp)) = &tui.status_message {
                assert_eq!(msg, "Another message");
            }
        }
    }

    // ============================================================================
    // Utility Function Tests
    // ============================================================================

    #[test]
    fn test_format_duration_seconds() {
        assert_eq!(format_duration(0), "0s");
        assert_eq!(format_duration(1000), "1s");
        assert_eq!(format_duration(5000), "5s");
        assert_eq!(format_duration(59000), "59s");
    }

    #[test]
    fn test_format_duration_minutes() {
        assert_eq!(format_duration(60000), "1m 0s");
        assert_eq!(format_duration(90000), "1m 30s");
        assert_eq!(format_duration(150000), "2m 30s");
        assert_eq!(format_duration(3599000), "59m 59s");
    }

    #[test]
    fn test_format_duration_hours() {
        assert_eq!(format_duration(3600000), "1h 0m");
        assert_eq!(format_duration(3660000), "1h 1m");
        assert_eq!(format_duration(7200000), "2h 0m");
        assert_eq!(format_duration(5400000), "1h 30m");
        assert_eq!(format_duration(86399000), "23h 59m");
    }

    #[test]
    fn test_format_duration_days() {
        assert_eq!(format_duration(86400000), "1d 0h");
        assert_eq!(format_duration(90000000), "1d 1h");
        assert_eq!(format_duration(172800000), "2d 0h");
        assert_eq!(format_duration(176400000), "2d 1h");
    }

    #[test]
    fn test_centered_rect_basic() {
        let area = Rect::new(0, 0, 100, 100);
        let centered = centered_rect(50, 50, area);

        // Should be centered in both dimensions
        assert_eq!(centered.x, 25);
        assert_eq!(centered.y, 25);
        assert_eq!(centered.width, 50);
        assert_eq!(centered.height, 50);
    }

    #[test]
    fn test_centered_rect_full_size() {
        let area = Rect::new(0, 0, 100, 100);
        let centered = centered_rect(100, 100, area);

        // Should take full area
        assert_eq!(centered.width, 100);
        assert_eq!(centered.height, 100);
    }

    #[test]
    fn test_centered_rect_small() {
        let area = Rect::new(0, 0, 100, 100);
        let centered = centered_rect(20, 20, area);

        // Should be small and centered
        assert_eq!(centered.x, 40);
        assert_eq!(centered.y, 40);
        assert_eq!(centered.width, 20);
        assert_eq!(centered.height, 20);
    }

    // ============================================================================
    // MonitorTypeForm Tests
    // ============================================================================

    #[test]
    fn test_monitor_type_form_display_http() {
        assert_eq!(format!("{}", MonitorTypeForm::Http), "HTTP");
    }

    #[test]
    fn test_monitor_type_form_display_ping() {
        assert_eq!(format!("{}", MonitorTypeForm::Ping), "Ping");
    }

    #[test]
    fn test_monitor_type_form_display_tcp() {
        assert_eq!(format!("{}", MonitorTypeForm::Tcp), "TCP");
    }

    #[test]
    fn test_monitor_type_form_equality() {
        assert_eq!(MonitorTypeForm::Http, MonitorTypeForm::Http);
        assert_eq!(MonitorTypeForm::Ping, MonitorTypeForm::Ping);
        assert_eq!(MonitorTypeForm::Tcp, MonitorTypeForm::Tcp);
        assert_ne!(MonitorTypeForm::Http, MonitorTypeForm::Ping);
        assert_ne!(MonitorTypeForm::Ping, MonitorTypeForm::Tcp);
    }

    #[test]
    fn test_monitor_type_form_copy() {
        let http = MonitorTypeForm::Http;
        let http_copy = http;
        assert_eq!(http, http_copy);
    }

    // ============================================================================
    // NodeForm Tests
    // ============================================================================

    #[test]
    fn test_node_form_default() {
        let form = NodeForm::default();
        assert_eq!(form.name, "");
        assert_eq!(form.monitor_type, MonitorTypeForm::Http);
        assert_eq!(form.monitoring_interval, "5");
        assert_eq!(form.http_url, "https://");
        assert_eq!(form.http_expected_status, "200");
        assert_eq!(form.current_field, 0);
    }

    #[test]
    fn test_node_form_get_field_count_http() {
        let mut form = NodeForm::default();
        form.monitor_type = MonitorTypeForm::Http;
        assert_eq!(form.get_field_count(), 5);
    }

    #[test]
    fn test_node_form_get_field_count_ping() {
        let mut form = NodeForm::default();
        form.monitor_type = MonitorTypeForm::Ping;
        assert_eq!(form.get_field_count(), 6);
    }

    #[test]
    fn test_node_form_get_field_count_tcp() {
        let mut form = NodeForm::default();
        form.monitor_type = MonitorTypeForm::Tcp;
        assert_eq!(form.get_field_count(), 6);
    }

    #[test]
    fn test_node_form_to_node_detail_http() {
        let mut form = NodeForm::default();
        form.http_url = "https://example.com".to_string();
        form.http_expected_status = "200".to_string();
        form.monitor_type = MonitorTypeForm::Http;

        let detail = form.to_node_detail().unwrap();
        match detail {
            MonitorDetail::Http {
                url,
                expected_status,
            } => {
                assert_eq!(url, "https://example.com");
                assert_eq!(expected_status, 200);
            }
            _ => panic!("Expected HTTP detail"),
        }
    }

    #[test]
    fn test_node_form_to_node_detail_http_invalid_status() {
        let mut form = NodeForm::default();
        form.http_url = "https://example.com".to_string();
        form.http_expected_status = "invalid".to_string();
        form.monitor_type = MonitorTypeForm::Http;

        assert!(form.to_node_detail().is_err());
    }

    #[test]
    fn test_node_form_to_node_detail_ping() {
        let mut form = NodeForm::default();
        form.monitor_type = MonitorTypeForm::Ping;
        form.ping_host = "example.com".to_string();
        form.ping_count = "4".to_string();
        form.ping_timeout = "5".to_string();

        let detail = form.to_node_detail().unwrap();
        match detail {
            MonitorDetail::Ping {
                host,
                count,
                timeout,
            } => {
                assert_eq!(host, "example.com");
                assert_eq!(count, 4);
                assert_eq!(timeout, 5);
            }
            _ => panic!("Expected Ping detail"),
        }
    }

    #[test]
    fn test_node_form_to_node_detail_ping_invalid_count() {
        let mut form = NodeForm::default();
        form.monitor_type = MonitorTypeForm::Ping;
        form.ping_host = "example.com".to_string();
        form.ping_count = "invalid".to_string();
        form.ping_timeout = "5".to_string();

        assert!(form.to_node_detail().is_err());
    }

    #[test]
    fn test_node_form_to_node_detail_tcp() {
        let mut form = NodeForm::default();
        form.monitor_type = MonitorTypeForm::Tcp;
        form.tcp_host = "example.com".to_string();
        form.tcp_port = "8080".to_string();
        form.tcp_timeout = "10".to_string();

        let detail = form.to_node_detail().unwrap();
        match detail {
            MonitorDetail::Tcp {
                host,
                port,
                timeout,
            } => {
                assert_eq!(host, "example.com");
                assert_eq!(port, 8080);
                assert_eq!(timeout, 10);
            }
            _ => panic!("Expected TCP detail"),
        }
    }

    #[test]
    fn test_node_form_to_node_detail_tcp_invalid_port() {
        let mut form = NodeForm::default();
        form.monitor_type = MonitorTypeForm::Tcp;
        form.tcp_host = "example.com".to_string();
        form.tcp_port = "invalid".to_string();
        form.tcp_timeout = "10".to_string();

        assert!(form.to_node_detail().is_err());
    }

    #[test]
    fn test_node_form_from_node_http() {
        let node = Node {
            id: Some(1),
            name: "Test Node".to_string(),
            detail: MonitorDetail::Http {
                url: "https://example.com".to_string(),
                expected_status: 404,
            },
            status: NodeStatus::Online,
            last_check: None,
            response_time: None,
            monitoring_interval: 10,
            consecutive_failures: 0,
            max_check_attempts: 3,
            retry_interval: 15,
        };

        let form = NodeForm::from_node(&node);
        assert_eq!(form.name, "Test Node");
        assert_eq!(form.monitor_type, MonitorTypeForm::Http);
        assert_eq!(form.monitoring_interval, "10");
        assert_eq!(form.http_url, "https://example.com");
        assert_eq!(form.http_expected_status, "404");
    }

    #[test]
    fn test_node_form_from_node_ping() {
        let node = Node {
            id: Some(2),
            name: "Ping Node".to_string(),
            detail: MonitorDetail::Ping {
                host: "8.8.8.8".to_string(),
                count: 3,
                timeout: 2,
            },
            status: NodeStatus::Offline,
            last_check: None,
            response_time: None,
            monitoring_interval: 15,
            consecutive_failures: 0,
            max_check_attempts: 3,
            retry_interval: 15,
        };

        let form = NodeForm::from_node(&node);
        assert_eq!(form.name, "Ping Node");
        assert_eq!(form.monitor_type, MonitorTypeForm::Ping);
        assert_eq!(form.monitoring_interval, "15");
        assert_eq!(form.ping_host, "8.8.8.8");
        assert_eq!(form.ping_count, "3");
        assert_eq!(form.ping_timeout, "2");
    }

    #[test]
    fn test_node_form_from_node_tcp() {
        let node = Node {
            id: Some(3),
            name: "TCP Node".to_string(),
            detail: MonitorDetail::Tcp {
                host: "localhost".to_string(),
                port: 9000,
                timeout: 3,
            },
            status: NodeStatus::Online,
            last_check: None,
            response_time: None,
            monitoring_interval: 20,
            consecutive_failures: 0,
            max_check_attempts: 3,
            retry_interval: 15,
        };

        let form = NodeForm::from_node(&node);
        assert_eq!(form.name, "TCP Node");
        assert_eq!(form.monitor_type, MonitorTypeForm::Tcp);
        assert_eq!(form.monitoring_interval, "20");
        assert_eq!(form.tcp_host, "localhost");
        assert_eq!(form.tcp_port, "9000");
        assert_eq!(form.tcp_timeout, "3");
    }

    #[test]
    fn test_node_form_clone() {
        let form1 = NodeForm::default();
        let form2 = form1.clone();
        assert_eq!(form1.name, form2.name);
        assert_eq!(form1.monitor_type, form2.monitor_type);
        assert_eq!(form1.monitoring_interval, form2.monitoring_interval);
    }

    // ============================================================================
    // AppState Tests
    // ============================================================================

    #[test]
    fn test_app_state_equality() {
        assert_eq!(AppState::Main, AppState::Main);
        assert_eq!(AppState::AddNode, AppState::AddNode);
        assert_ne!(AppState::Main, AppState::AddNode);
    }

    #[test]
    fn test_app_state_copy() {
        let state = AppState::Main;
        let state_copy = state;
        assert_eq!(state, state_copy);
    }

    // ============================================================================
    // NodeConfigUpdate Tests
    // ============================================================================

    #[test]
    fn test_node_config_update_add() {
        let node = Node {
            id: Some(1),
            name: "Test".to_string(),
            detail: MonitorDetail::Http {
                url: "https://example.com".to_string(),
                expected_status: 200,
            },
            status: NodeStatus::Online,
            last_check: None,
            response_time: None,
            monitoring_interval: 5,
            consecutive_failures: 0,
            max_check_attempts: 3,
            retry_interval: 15,
        };

        let update = NodeConfigUpdate::Add(node.clone());
        let update_clone = update.clone();

        // Verify clone works
        match (update, update_clone) {
            (NodeConfigUpdate::Add(n1), NodeConfigUpdate::Add(n2)) => {
                assert_eq!(n1.id, n2.id);
                assert_eq!(n1.name, n2.name);
            }
            _ => panic!("Expected Add variants"),
        }
    }

    #[test]
    fn test_node_config_update_update() {
        let node = Node {
            id: Some(2),
            name: "Updated".to_string(),
            detail: MonitorDetail::Ping {
                host: "8.8.8.8".to_string(),
                count: 4,
                timeout: 5,
            },
            status: NodeStatus::Offline,
            last_check: None,
            response_time: None,
            monitoring_interval: 10,
            consecutive_failures: 0,
            max_check_attempts: 3,
            retry_interval: 15,
        };

        let update = NodeConfigUpdate::Update(node);
        let _update_clone = update.clone();
    }

    #[test]
    fn test_node_config_update_delete() {
        let update = NodeConfigUpdate::Delete(42);
        let update_clone = update.clone();

        match (update, update_clone) {
            (NodeConfigUpdate::Delete(id1), NodeConfigUpdate::Delete(id2)) => {
                assert_eq!(id1, id2);
            }
            _ => panic!("Expected Delete variants"),
        }
    }

    // ============================================================================
    // Additional Edge Case Tests
    // ============================================================================

    #[test]
    fn test_node_form_from_node_tcp_default_port() {
        let node = Node {
            id: None,
            name: "SSH Node".to_string(),
            detail: MonitorDetail::Tcp {
                host: "192.168.1.1".to_string(),
                port: 22,
                timeout: 5,
            },
            status: NodeStatus::Offline,
            last_check: None,
            response_time: None,
            monitoring_interval: 30,
            consecutive_failures: 0,
            max_check_attempts: 3,
            retry_interval: 15,
        };

        let form = NodeForm::from_node(&node);
        assert_eq!(form.tcp_host, "192.168.1.1");
        assert_eq!(form.tcp_port, "22");
    }

    #[test]
    fn test_node_form_http_with_various_status_codes() {
        let test_cases = vec![
            ("200", 200),
            ("201", 201),
            ("301", 301),
            ("404", 404),
            ("500", 500),
        ];

        for (status_str, expected_code) in test_cases {
            let mut form = NodeForm::default();
            form.http_url = "https://test.com".to_string();
            form.http_expected_status = status_str.to_string();
            form.monitor_type = MonitorTypeForm::Http;

            let detail = form.to_node_detail().unwrap();
            match detail {
                MonitorDetail::Http {
                    expected_status, ..
                } => {
                    assert_eq!(expected_status, expected_code);
                }
                _ => panic!("Expected HTTP detail"),
            }
        }
    }

    #[test]
    fn test_node_form_ping_with_various_values() {
        let mut form = NodeForm::default();
        form.monitor_type = MonitorTypeForm::Ping;
        form.ping_host = "google.com".to_string();
        form.ping_count = "10".to_string();
        form.ping_timeout = "3".to_string();

        let detail = form.to_node_detail().unwrap();
        match detail {
            MonitorDetail::Ping {
                host,
                count,
                timeout,
            } => {
                assert_eq!(host, "google.com");
                assert_eq!(count, 10);
                assert_eq!(timeout, 3);
            }
            _ => panic!("Expected Ping detail"),
        }
    }

    #[test]
    fn test_node_form_tcp_with_high_port() {
        let mut form = NodeForm::default();
        form.monitor_type = MonitorTypeForm::Tcp;
        form.tcp_host = "server.local".to_string();
        form.tcp_port = "65535".to_string();
        form.tcp_timeout = "30".to_string();

        let detail = form.to_node_detail().unwrap();
        match detail {
            MonitorDetail::Tcp {
                host,
                port,
                timeout,
            } => {
                assert_eq!(host, "server.local");
                assert_eq!(port, 65535);
                assert_eq!(timeout, 30);
            }
            _ => panic!("Expected TCP detail"),
        }
    }

    #[test]
    fn test_node_form_tcp_invalid_port_too_high() {
        let mut form = NodeForm::default();
        form.monitor_type = MonitorTypeForm::Tcp;
        form.tcp_host = "example.com".to_string();
        form.tcp_port = "99999".to_string();
        form.tcp_timeout = "10".to_string();

        // Port 99999 is too high for u16, should error
        assert!(form.to_node_detail().is_err());
    }

    #[test]
    fn test_node_form_tcp_invalid_timeout() {
        let mut form = NodeForm::default();
        form.monitor_type = MonitorTypeForm::Tcp;
        form.tcp_host = "example.com".to_string();
        form.tcp_port = "8080".to_string();
        form.tcp_timeout = "not_a_number".to_string();

        assert!(form.to_node_detail().is_err());
    }

    #[test]
    fn test_node_form_ping_invalid_timeout() {
        let mut form = NodeForm::default();
        form.monitor_type = MonitorTypeForm::Ping;
        form.ping_host = "example.com".to_string();
        form.ping_count = "4".to_string();
        form.ping_timeout = "abc".to_string();

        assert!(form.to_node_detail().is_err());
    }

    #[test]
    fn test_format_duration_edge_cases() {
        // Test 0 milliseconds
        assert_eq!(format_duration(0), "0s");

        // Test exactly 1 minute
        assert_eq!(format_duration(60000), "1m 0s");

        // Test exactly 1 hour
        assert_eq!(format_duration(3600000), "1h 0m");

        // Test exactly 1 day
        assert_eq!(format_duration(86400000), "1d 0h");

        // Test large values
        assert_eq!(format_duration(604800000), "7d 0h"); // 1 week
    }

    #[test]
    fn test_format_duration_negative() {
        // Negative durations should still format (edge case handling)
        let result = format_duration(-1000);
        assert!(result.contains("s")); // Should still return something with seconds
    }

    #[test]
    fn test_centered_rect_edge_cases() {
        // Test with 0% size (should still work but be very small)
        let area = Rect::new(0, 0, 100, 100);
        let centered = centered_rect(0, 0, area);
        assert!(centered.width <= 100);
        assert!(centered.height <= 100);
    }

    #[test]
    fn test_centered_rect_non_square_area() {
        let area = Rect::new(0, 0, 200, 50);
        let centered = centered_rect(50, 80, area);

        // Should center in the rectangular area (x and y are u16, always >= 0)
        assert!(centered.width <= 200);
        assert!(centered.height <= 50);
    }

    #[test]
    fn test_monitor_type_form_all_variants() {
        let variants = vec![
            MonitorTypeForm::Http,
            MonitorTypeForm::Ping,
            MonitorTypeForm::Tcp,
        ];

        for variant in variants {
            // Test that copy works
            let copy = variant;
            assert_eq!(variant, copy);

            // Test that display works
            let display_str = format!("{}", variant);
            assert!(!display_str.is_empty());
        }
    }

    #[test]
    fn test_app_state_all_variants() {
        let variants = vec![
            AppState::Main,
            AppState::AddNode,
            AppState::EditNode,
            AppState::ViewHistory,
            AppState::Help,
            AppState::ConfirmDelete,
            AppState::ImportModeSelect,
            AppState::Reorder,
        ];

        for variant in variants {
            // Test that copy works
            let copy = variant;
            assert_eq!(variant, copy);
        }
    }

    #[test]
    fn test_node_form_roundtrip_http() {
        let original_node = Node {
            id: Some(100),
            name: "Roundtrip Test".to_string(),
            detail: MonitorDetail::Http {
                url: "https://roundtrip.com".to_string(),
                expected_status: 201,
            },
            status: NodeStatus::Online,
            last_check: None,
            response_time: Some(150),
            monitoring_interval: 7,
            consecutive_failures: 0,
            max_check_attempts: 3,
            retry_interval: 15,
        };

        // Convert to form and back
        let form = NodeForm::from_node(&original_node);
        let detail = form.to_node_detail().unwrap();

        // Verify detail matches
        match detail {
            MonitorDetail::Http {
                url,
                expected_status,
            } => {
                assert_eq!(url, "https://roundtrip.com");
                assert_eq!(expected_status, 201);
            }
            _ => panic!("Expected HTTP detail"),
        }

        assert_eq!(form.name, "Roundtrip Test");
        assert_eq!(form.monitoring_interval, "7");
    }

    #[test]
    fn test_node_form_roundtrip_ping() {
        let original_node = Node {
            id: Some(200),
            name: "Ping Roundtrip".to_string(),
            detail: MonitorDetail::Ping {
                host: "1.1.1.1".to_string(),
                count: 5,
                timeout: 10,
            },
            status: NodeStatus::Offline,
            last_check: Some(chrono::Utc::now()),
            response_time: None,
            monitoring_interval: 15,
            consecutive_failures: 0,
            max_check_attempts: 3,
            retry_interval: 15,
        };

        let form = NodeForm::from_node(&original_node);
        let detail = form.to_node_detail().unwrap();

        match detail {
            MonitorDetail::Ping {
                host,
                count,
                timeout,
            } => {
                assert_eq!(host, "1.1.1.1");
                assert_eq!(count, 5);
                assert_eq!(timeout, 10);
            }
            _ => panic!("Expected Ping detail"),
        }
    }

    #[test]
    fn test_node_form_roundtrip_tcp() {
        let original_node = Node {
            id: Some(300),
            name: "TCP Roundtrip".to_string(),
            detail: MonitorDetail::Tcp {
                host: "db.server.com".to_string(),
                port: 5432,
                timeout: 20,
            },
            status: NodeStatus::Online,
            last_check: Some(chrono::Utc::now()),
            response_time: Some(25),
            monitoring_interval: 60,
            consecutive_failures: 0,
            max_check_attempts: 3,
            retry_interval: 15,
        };

        let form = NodeForm::from_node(&original_node);
        let detail = form.to_node_detail().unwrap();

        match detail {
            MonitorDetail::Tcp {
                host,
                port,
                timeout,
            } => {
                assert_eq!(host, "db.server.com");
                assert_eq!(port, 5432);
                assert_eq!(timeout, 20);
            }
            _ => panic!("Expected TCP detail"),
        }
    }

    // ============================================================================
    // List navigation index math
    // ============================================================================

    #[test]
    fn test_next_selection_empty_list_returns_none() {
        // Regression: this used to compute `len - 1` on an empty list, which
        // underflows (panic in debug, usize::MAX in release).
        assert_eq!(next_selection(Some(0), 0), None);
        assert_eq!(next_selection(Some(5), 0), None);
        assert_eq!(next_selection(None, 0), None);
    }

    #[test]
    fn test_previous_selection_empty_list_returns_none() {
        assert_eq!(previous_selection(Some(0), 0), None);
        assert_eq!(previous_selection(Some(5), 0), None);
        assert_eq!(previous_selection(None, 0), None);
    }

    #[test]
    fn test_next_selection_advances_and_wraps() {
        assert_eq!(next_selection(Some(0), 3), Some(1));
        assert_eq!(next_selection(Some(1), 3), Some(2));
        // Last item wraps back to the first.
        assert_eq!(next_selection(Some(2), 3), Some(0));
    }

    #[test]
    fn test_previous_selection_retreats_and_wraps() {
        assert_eq!(previous_selection(Some(2), 3), Some(1));
        assert_eq!(previous_selection(Some(1), 3), Some(0));
        // First item wraps to the last.
        assert_eq!(previous_selection(Some(0), 3), Some(2));
    }

    #[test]
    fn test_selection_with_no_prior_selection_starts_at_first() {
        assert_eq!(next_selection(None, 3), Some(0));
        assert_eq!(previous_selection(None, 3), Some(0));
    }

    #[test]
    fn test_selection_on_single_item_list_stays_put() {
        assert_eq!(next_selection(Some(0), 1), Some(0));
        assert_eq!(previous_selection(Some(0), 1), Some(0));
        assert_eq!(next_selection(None, 1), Some(0));
    }

    #[test]
    fn test_stale_out_of_range_selection_is_brought_back_in_range() {
        // A selection can outlive the rows it pointed at; neither direction
        // should return an index past the end.
        assert_eq!(next_selection(Some(99), 3), Some(0));
        assert_eq!(previous_selection(Some(99), 3), Some(2));
    }
}
