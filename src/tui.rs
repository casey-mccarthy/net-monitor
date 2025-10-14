use crate::connection::ConnectionStrategy;
use crate::credentials::{CredentialStore, CredentialSummary, FileCredentialStore};
use crate::database::Database;
use crate::models::{MonitorDetail, Node, NodeImport, NodeStatus, StatusChange};
use crate::monitor::check_node;
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
    widgets::{
        Block, Borders, Cell, Clear, List, ListItem, ListState, Paragraph, Row, Table, TableState,
        Wrap,
    },
    Frame, Terminal,
};
use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use tracing::{error, info};

#[derive(Clone, Copy, PartialEq)]
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
    credential_id: Option<String>,
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
            credential_id: None,
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
            credential_id: node.credential_id.clone(),
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
        // name, monitoring_interval, monitor_type, credential_id + type-specific fields
        match self.monitor_type {
            MonitorTypeForm::Http => 6, // name, interval, type, cred, url, status
            MonitorTypeForm::Ping => 7, // name, interval, type, cred, host, count, timeout
            MonitorTypeForm::Tcp => 7,  // name, interval, type, cred, host, port, timeout
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
#[allow(dead_code)] // Future feature: credential form variants
enum CredentialTypeForm {
    Default,
    Password,
    KeyFile,
    KeyData,
}

impl std::fmt::Display for CredentialTypeForm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CredentialTypeForm::Default => write!(f, "System Default"),
            CredentialTypeForm::Password => write!(f, "Username/Password"),
            CredentialTypeForm::KeyFile => write!(f, "SSH Key File"),
            CredentialTypeForm::KeyData => write!(f, "SSH Key Data"),
        }
    }
}

#[derive(Clone)]
#[allow(dead_code)] // Future feature: credential form fields
struct CredentialForm {
    name: String,
    description: String,
    credential_type: CredentialTypeForm,
    username: String,
    password: String,
    ssh_key_path: String,
    ssh_key_data: String,
    passphrase: String,
    current_field: usize,
}

impl Default for CredentialForm {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            credential_type: CredentialTypeForm::Default,
            username: String::new(),
            password: String::new(),
            ssh_key_path: String::new(),
            ssh_key_data: String::new(),
            passphrase: String::new(),
            current_field: 0,
        }
    }
}

impl CredentialForm {
    fn get_field_count(&self) -> usize {
        // name, description, credential_type + type-specific fields
        match self.credential_type {
            CredentialTypeForm::Default => 3,  // name, description, type
            CredentialTypeForm::Password => 5, // name, description, type, username, password
            CredentialTypeForm::KeyFile => 6, // name, description, type, username, key_path, passphrase
            CredentialTypeForm::KeyData => 6, // name, description, type, username, key_data, passphrase
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum AppState {
    Main,
    AddNode,
    EditNode,
    ViewHistory,
    ManageCredentials,
    AddCredential,
    Help,
    ConfirmDelete,
    ImportNodes,
    ExportNodes,
}

struct MonitoringHandle {
    stop_tx: mpsc::Sender<()>,
    config_tx: mpsc::Sender<NodeConfigUpdate>,
}

#[derive(Clone)]
enum NodeConfigUpdate {
    Add(Node),
    Update(Node),
    Delete(i64),
}

pub struct NetworkMonitorTui {
    database: Database,
    nodes: Vec<Node>,
    table_state: TableState,
    list_state: ListState,
    state: AppState,
    status_message: Option<(String, Instant)>,
    monitoring_handle: Option<MonitoringHandle>,
    update_rx: mpsc::Receiver<Node>,
    update_tx: mpsc::Sender<Node>,
    updated_nodes: HashMap<i64, Instant>,
    // Node form
    node_form: NodeForm,
    editing_node_id: Option<i64>,
    // Credentials
    credential_store: Box<dyn CredentialStore>,
    credentials: Vec<CredentialSummary>,
    credential_form: CredentialForm,
    // Status history
    viewing_history_node_id: Option<i64>,
    status_changes: Vec<StatusChange>,
    history_table_state: TableState,
    // Delete confirmation
    delete_node_index: Option<usize>,
    delete_credential_index: Option<usize>,
    return_to_credentials_after_delete: bool,
    // Import/Export
    import_export_path: String,
    // Auto-hide selection
    last_input_time: Option<Instant>,
    // Cursor blink state for empty fields
    cursor_blink_state: bool,
    last_blink_time: Instant,
    // Help context
    previous_state: Option<AppState>,
}

impl NetworkMonitorTui {
    pub fn new(database: Database) -> Result<Self> {
        let nodes = database.get_all_nodes()?;
        let (update_tx, update_rx) = mpsc::channel();

        let credential_store: Box<dyn CredentialStore> =
            match FileCredentialStore::new("default_password".to_string()) {
                Ok(store) => {
                    info!("Successfully created file credential store");
                    Box::new(store)
                }
                Err(e) => {
                    error!("Failed to initialize credential store: {}", e);
                    return Err(e);
                }
            };

        let credentials = credential_store.list_credentials().unwrap_or_default();

        let mut app = Self {
            database,
            nodes,
            table_state: TableState::default(),
            list_state: ListState::default(),
            state: AppState::Main,
            status_message: None,
            monitoring_handle: None,
            update_rx,
            update_tx,
            updated_nodes: HashMap::new(),
            node_form: NodeForm::default(),
            editing_node_id: None,
            credential_store,
            credentials,
            credential_form: CredentialForm::default(),
            viewing_history_node_id: None,
            status_changes: Vec::new(),
            history_table_state: TableState::default(),
            delete_node_index: None,
            delete_credential_index: None,
            return_to_credentials_after_delete: false,
            import_export_path: String::new(),
            last_input_time: Some(Instant::now()),
            cursor_blink_state: true,
            last_blink_time: Instant::now(),
            previous_state: None,
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
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let result = self.run_app(&mut terminal);

        // Restore terminal
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        // Stop monitoring
        if let Err(e) = self.stop_monitoring() {
            error!("Failed to stop monitoring: {}", e);
        }

        result
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
                    *node = updated_node;
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
                if now.duration_since(last_input).as_secs() >= 5 {
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
                            AppState::ManageCredentials => {
                                if self.handle_credentials_input(key.code) {
                                    self.state = AppState::Main;
                                }
                            }
                            AppState::AddCredential => {
                                if self.handle_credential_form_input(key.code) {
                                    self.state = AppState::ManageCredentials;
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
                                    let i = match self.history_table_state.selected() {
                                        Some(i) => {
                                            if i >= self.status_changes.len().saturating_sub(1) {
                                                i
                                            } else {
                                                i + 1
                                            }
                                        }
                                        None => 0,
                                    };
                                    if !self.status_changes.is_empty() {
                                        self.history_table_state.select(Some(i));
                                    }
                                }
                                KeyCode::Up => {
                                    let i = match self.history_table_state.selected() {
                                        Some(i) => i.saturating_sub(1),
                                        None => 0,
                                    };
                                    if !self.status_changes.is_empty() {
                                        self.history_table_state.select(Some(i));
                                    }
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
                            AppState::ConfirmDelete => {
                                if self.handle_confirm_delete_input(key.code) {
                                    // Return to credential manager if we came from there
                                    if self.return_to_credentials_after_delete {
                                        self.state = AppState::ManageCredentials;
                                        self.return_to_credentials_after_delete = false;
                                    } else {
                                        self.state = AppState::Main;
                                    }
                                }
                            }
                            AppState::ImportNodes | AppState::ExportNodes => {
                                if self.handle_import_export_input(key.code) {
                                    self.state = AppState::Main;
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn ui(&mut self, f: &mut Frame) {
        match self.state {
            AppState::Main => self.render_main_view(f),
            AppState::AddNode | AppState::EditNode => self.render_node_form(f),
            AppState::ManageCredentials => self.render_credentials_view(f),
            AppState::AddCredential => self.render_credential_form(f),
            AppState::ViewHistory => self.render_history_view(f),
            AppState::Help => self.render_help_view(f),
            AppState::ConfirmDelete => self.render_confirm_delete(f),
            AppState::ImportNodes | AppState::ExportNodes => self.render_import_export(f),
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
        let menu_text = vec![
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
            Span::styled("C", Style::default().fg(Color::Yellow)),
            Span::raw("]redentials "),
            Span::raw("["),
            Span::styled("I", Style::default().fg(Color::Yellow)),
            Span::raw("]mport "),
            Span::raw("["),
            Span::styled("X", Style::default().fg(Color::Yellow)),
            Span::raw("]export "),
            Span::raw("["),
            Span::styled("?", Style::default().fg(Color::Yellow)),
            Span::raw("]Help "),
            Span::raw("["),
            Span::styled("Q", Style::default().fg(Color::Yellow)),
            Span::raw("]uit"),
        ];

        let content_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(0)])
            .split(chunks[1]);

        let menu = Paragraph::new(Line::from(menu_text));
        f.render_widget(menu, content_chunks[0]);

        // Node table
        let header = Row::new(vec!["Name", "Target", "Type", "Status", "Last Check"])
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
                };

                // Add visual indicator for status
                let status_str = match node.status {
                    NodeStatus::Online => "● Online",
                    NodeStatus::Offline => "● Offline",
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
                    }
                } else {
                    Color::White
                };

                // Create cells with individual styling
                let cells = vec![
                    Cell::from(node.name.clone()).style(Style::default().fg(Color::White)),
                    Cell::from(node.detail.get_connection_target())
                        .style(Style::default().fg(Color::Cyan)),
                    Cell::from(node.detail.to_string()).style(Style::default().fg(Color::Yellow)),
                    Cell::from(status_str).style(
                        Style::default()
                            .fg(status_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Cell::from(last_check_display).style(
                        Style::default().fg(last_check_color).add_modifier(
                            if flash_intensity > 0.0 {
                                Modifier::BOLD
                            } else {
                                Modifier::empty()
                            },
                        ),
                    ),
                ];

                Row::new(cells)
            })
            .collect();

        // Conditionally apply gray background highlight based on input activity
        let highlight_style = if self.last_input_time.is_some() {
            // Show gray background when there has been recent input
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD)
        } else {
            // Hide gray background after 5 seconds of inactivity, but keep >> symbol
            Style::default()
        };

        let table = Table::new(
            rows,
            [
                Constraint::Percentage(20),
                Constraint::Percentage(25),
                Constraint::Percentage(10),
                Constraint::Percentage(15),
                Constraint::Percentage(30),
            ],
        )
        .header(header)
        .block(Block::default().borders(Borders::ALL).title("Nodes"))
        .row_highlight_style(highlight_style)
        .highlight_symbol(">> ");

        f.render_stateful_widget(table, content_chunks[1], &mut self.table_state);

        // Status bar
        let monitoring_status = if self.monitoring_handle.is_some() {
            Span::styled("Monitoring: ON", Style::default().fg(Color::Green))
        } else {
            Span::styled("Monitoring: OFF", Style::default().fg(Color::Red))
        };

        let node_stats = format!(
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
        );

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
            Line::from(vec![
                Span::raw("Credential: "),
                Span::styled(
                    form.credential_id.as_deref().unwrap_or("None"),
                    if form.current_field == 3 {
                        Style::default().bg(Color::DarkGray)
                    } else {
                        Style::default()
                    },
                ),
            ]),
        ];

        match form.monitor_type {
            MonitorTypeForm::Http => {
                lines.push(Line::from(vec![
                    Span::raw("URL: "),
                    Span::styled(
                        if form.http_url.is_empty() && form.current_field == 4 {
                            cursor
                        } else {
                            &form.http_url
                        },
                        if form.current_field == 4 {
                            Style::default().bg(Color::DarkGray)
                        } else {
                            Style::default()
                        },
                    ),
                ]));
                lines.push(Line::from(vec![
                    Span::raw("Expected Status: "),
                    Span::styled(
                        if form.http_expected_status.is_empty() && form.current_field == 5 {
                            cursor
                        } else {
                            &form.http_expected_status
                        },
                        if form.current_field == 5 {
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
                        if form.ping_host.is_empty() && form.current_field == 4 {
                            cursor
                        } else {
                            &form.ping_host
                        },
                        if form.current_field == 4 {
                            Style::default().bg(Color::DarkGray)
                        } else {
                            Style::default()
                        },
                    ),
                ]));
                lines.push(Line::from(vec![
                    Span::raw("Count: "),
                    Span::styled(
                        if form.ping_count.is_empty() && form.current_field == 5 {
                            cursor
                        } else {
                            &form.ping_count
                        },
                        if form.current_field == 5 {
                            Style::default().bg(Color::DarkGray)
                        } else {
                            Style::default()
                        },
                    ),
                ]));
                lines.push(Line::from(vec![
                    Span::raw("Timeout (s): "),
                    Span::styled(
                        if form.ping_timeout.is_empty() && form.current_field == 6 {
                            cursor
                        } else {
                            &form.ping_timeout
                        },
                        if form.current_field == 6 {
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
                        if form.tcp_host.is_empty() && form.current_field == 4 {
                            cursor
                        } else {
                            &form.tcp_host
                        },
                        if form.current_field == 4 {
                            Style::default().bg(Color::DarkGray)
                        } else {
                            Style::default()
                        },
                    ),
                ]));
                lines.push(Line::from(vec![
                    Span::raw("Port: "),
                    Span::styled(
                        if form.tcp_port.is_empty() && form.current_field == 5 {
                            cursor
                        } else {
                            &form.tcp_port
                        },
                        if form.current_field == 5 {
                            Style::default().bg(Color::DarkGray)
                        } else {
                            Style::default()
                        },
                    ),
                ]));
                lines.push(Line::from(vec![
                    Span::raw("Timeout (s): "),
                    Span::styled(
                        if form.tcp_timeout.is_empty() && form.current_field == 6 {
                            cursor
                        } else {
                            &form.tcp_timeout
                        },
                        if form.current_field == 6 {
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

    fn render_credentials_view(&mut self, f: &mut Frame) {
        let area = centered_rect(70, 70, f.area());
        f.render_widget(Clear, area);

        let block = Block::default()
            .title("Credential Manager")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let inner = block.inner(area);
        f.render_widget(block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(0),
                Constraint::Length(2),
            ])
            .split(inner);

        let menu = Paragraph::new(Line::from(vec![
            Span::raw("["),
            Span::styled("A", Style::default().fg(Color::Yellow)),
            Span::raw("]dd | ["),
            Span::styled("D", Style::default().fg(Color::Yellow)),
            Span::raw("]elete | ["),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw("] Back"),
        ]));
        f.render_widget(menu, chunks[0]);

        let items: Vec<ListItem> = self
            .credentials
            .iter()
            .map(|cred| {
                ListItem::new(Line::from(vec![
                    Span::raw(&cred.name),
                    Span::raw(" - "),
                    Span::styled(&cred.credential_type, Style::default().fg(Color::DarkGray)),
                    Span::raw(" - "),
                    Span::raw(cred.description.as_deref().unwrap_or("")),
                ]))
            })
            .collect();

        let list = List::new(items)
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");

        f.render_stateful_widget(list, chunks[1], &mut self.list_state);
    }

    fn render_credential_form(&mut self, f: &mut Frame) {
        let area = centered_rect(70, 80, f.area());
        f.render_widget(Clear, area);

        let block = Block::default()
            .title("Add Credential")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let inner = block.inner(area);
        f.render_widget(block, area);

        let form = &self.credential_form;
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
                Span::raw("Description: "),
                Span::styled(
                    if form.description.is_empty() && form.current_field == 1 {
                        cursor
                    } else {
                        &form.description
                    },
                    if form.current_field == 1 {
                        Style::default().bg(Color::DarkGray)
                    } else {
                        Style::default()
                    },
                ),
            ]),
            Line::from(vec![
                Span::raw("Credential Type: "),
                Span::styled(
                    format!("{} ", form.credential_type),
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

        match form.credential_type {
            CredentialTypeForm::Default => {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "Uses system default SSH configuration",
                    Style::default().fg(Color::Gray),
                )));
            }
            CredentialTypeForm::Password => {
                lines.push(Line::from(vec![
                    Span::raw("Username: "),
                    Span::styled(
                        if form.username.is_empty() && form.current_field == 3 {
                            cursor
                        } else {
                            &form.username
                        },
                        if form.current_field == 3 {
                            Style::default().bg(Color::DarkGray)
                        } else {
                            Style::default()
                        },
                    ),
                ]));
                let password_display = if form.password.is_empty() {
                    cursor.to_string()
                } else {
                    "*".repeat(form.password.len())
                };
                lines.push(Line::from(vec![
                    Span::raw("Password: "),
                    Span::styled(
                        password_display,
                        if form.current_field == 4 {
                            Style::default().bg(Color::DarkGray)
                        } else {
                            Style::default()
                        },
                    ),
                ]));
            }
            CredentialTypeForm::KeyFile => {
                lines.push(Line::from(vec![
                    Span::raw("Username: "),
                    Span::styled(
                        if form.username.is_empty() && form.current_field == 3 {
                            cursor
                        } else {
                            &form.username
                        },
                        if form.current_field == 3 {
                            Style::default().bg(Color::DarkGray)
                        } else {
                            Style::default()
                        },
                    ),
                ]));
                lines.push(Line::from(vec![
                    Span::raw("SSH Key Path: "),
                    Span::styled(
                        if form.ssh_key_path.is_empty() && form.current_field == 4 {
                            cursor
                        } else {
                            &form.ssh_key_path
                        },
                        if form.current_field == 4 {
                            Style::default().bg(Color::DarkGray)
                        } else {
                            Style::default()
                        },
                    ),
                ]));
                let passphrase_display = if form.passphrase.is_empty() {
                    cursor.to_string()
                } else {
                    "*".repeat(form.passphrase.len())
                };
                lines.push(Line::from(vec![
                    Span::raw("Passphrase (optional): "),
                    Span::styled(
                        passphrase_display,
                        if form.current_field == 5 {
                            Style::default().bg(Color::DarkGray)
                        } else {
                            Style::default()
                        },
                    ),
                ]));
            }
            CredentialTypeForm::KeyData => {
                lines.push(Line::from(vec![
                    Span::raw("Username: "),
                    Span::styled(
                        if form.username.is_empty() && form.current_field == 3 {
                            cursor
                        } else {
                            &form.username
                        },
                        if form.current_field == 3 {
                            Style::default().bg(Color::DarkGray)
                        } else {
                            Style::default()
                        },
                    ),
                ]));
                let key_data_display = if form.ssh_key_data.is_empty() {
                    if form.current_field == 4 {
                        cursor.to_string()
                    } else {
                        "<paste private key>".to_string()
                    }
                } else {
                    "<key data entered>".to_string()
                };
                lines.push(Line::from(vec![
                    Span::raw("SSH Key Data: "),
                    Span::styled(
                        key_data_display,
                        if form.current_field == 4 {
                            Style::default().bg(Color::DarkGray)
                        } else {
                            Style::default().fg(Color::Gray)
                        },
                    ),
                ]));
                let passphrase_display = if form.passphrase.is_empty() {
                    cursor.to_string()
                } else {
                    "*".repeat(form.passphrase.len())
                };
                lines.push(Line::from(vec![
                    Span::raw("Passphrase (optional): "),
                    Span::styled(
                        passphrase_display,
                        if form.current_field == 5 {
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
            Span::raw(" Change Type | "),
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
        if self.status_changes.is_empty() {
            let msg = Paragraph::new("No status changes recorded.")
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::Gray));
            f.render_widget(msg, chunks[1]);
        } else {
            let header = Row::new(vec!["Timestamp", "From", "To", "Duration"])
                .style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
                .bottom_margin(1);

            let rows: Vec<Row> = self
                .status_changes
                .iter()
                .map(|change| {
                    let timestamp = change
                        .changed_at
                        .with_timezone(&chrono::Local)
                        .format("%Y-%m-%d %H:%M:%S")
                        .to_string();

                    let duration = change
                        .duration_ms
                        .map(format_duration)
                        .unwrap_or_else(|| "N/A".to_string());

                    // Color-code status fields
                    let from_color = match change.from_status {
                        NodeStatus::Online => Color::Green,
                        NodeStatus::Offline => Color::Red,
                    };

                    let to_color = match change.to_status {
                        NodeStatus::Online => Color::Green,
                        NodeStatus::Offline => Color::Red,
                    };

                    Row::new(vec![
                        Cell::from(timestamp),
                        Cell::from(change.from_status.to_string())
                            .style(Style::default().fg(from_color)),
                        Cell::from(change.to_status.to_string())
                            .style(Style::default().fg(to_color)),
                        Cell::from(duration),
                    ])
                })
                .collect();

            let table = Table::new(
                rows,
                [
                    Constraint::Percentage(40),
                    Constraint::Percentage(15),
                    Constraint::Percentage(15),
                    Constraint::Percentage(30),
                ],
            )
            .header(header)
            .highlight_style(
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
                        Span::styled("c", Style::default().fg(Color::Yellow)),
                        Span::raw(" - Manage credentials"),
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
            Some(AppState::ManageCredentials) => (
                "Help - Credentials Manager",
                vec![
                    Line::from(vec![
                        Span::styled("a", Style::default().fg(Color::Yellow)),
                        Span::raw(" - Add new credential"),
                    ]),
                    Line::from(vec![
                        Span::styled("d", Style::default().fg(Color::Yellow)),
                        Span::raw(" - Delete selected credential"),
                    ]),
                    Line::from(vec![
                        Span::styled("↑/↓", Style::default().fg(Color::Yellow)),
                        Span::raw(" - Navigate credentials"),
                    ]),
                    Line::from(vec![
                        Span::styled("Esc/q", Style::default().fg(Color::Yellow)),
                        Span::raw(" - Return to main view"),
                    ]),
                ],
            ),
            Some(AppState::AddCredential) => (
                "Help - Credential Form",
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
                        Span::raw(" - Change credential type"),
                    ]),
                    Line::from(vec![
                        Span::styled("Enter", Style::default().fg(Color::Yellow)),
                        Span::raw(" - Save credential"),
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
            Some(AppState::ImportNodes) => (
                "Help - Import Nodes",
                vec![
                    Line::from(vec![Span::raw(
                        "Enter the path to a JSON file to import nodes.",
                    )]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("Enter", Style::default().fg(Color::Yellow)),
                        Span::raw(" - Confirm import"),
                    ]),
                    Line::from(vec![
                        Span::styled("Esc", Style::default().fg(Color::Yellow)),
                        Span::raw(" - Cancel"),
                    ]),
                ],
            ),
            Some(AppState::ExportNodes) => (
                "Help - Export Nodes",
                vec![
                    Line::from(vec![Span::raw(
                        "Enter the path where nodes will be exported as JSON.",
                    )]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("Enter", Style::default().fg(Color::Yellow)),
                        Span::raw(" - Confirm export"),
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
        } else if let Some(index) = self.delete_credential_index {
            let name = self
                .credentials
                .get(index)
                .map(|c| c.name.as_str())
                .unwrap_or("Unknown");
            ("credential", name)
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

    fn render_import_export(&mut self, f: &mut Frame) {
        let area = centered_rect(60, 20, f.area());
        f.render_widget(Clear, area);

        let title = if self.state == AppState::ImportNodes {
            "Import Nodes"
        } else {
            "Export Nodes"
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let cursor = if self.cursor_blink_state { "│" } else { "" };
        let text = vec![
            Line::from("Enter file path:"),
            Line::from(Span::styled(
                if self.import_export_path.is_empty() {
                    cursor
                } else {
                    &self.import_export_path
                },
                Style::default().bg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("[Enter]", Style::default().fg(Color::Yellow)),
                Span::raw(" Confirm | "),
                Span::styled("[Esc]", Style::default().fg(Color::Yellow)),
                Span::raw(" Cancel"),
            ]),
        ];

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
                    if let Some(node) = self.nodes.get(selected) {
                        self.node_form = NodeForm::from_node(node);
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
            KeyCode::Char('c') | KeyCode::Char('C') => {
                self.reload_credentials();
                if !self.credentials.is_empty() {
                    self.list_state.select(Some(0));
                }
                self.state = AppState::ManageCredentials;
            }
            KeyCode::Char('i') | KeyCode::Char('I') => {
                self.import_export_path.clear();
                self.state = AppState::ImportNodes;
            }
            KeyCode::Char('x') | KeyCode::Char('X') => {
                self.import_export_path.clear();
                self.state = AppState::ExportNodes;
            }
            KeyCode::Char('?') => {
                self.previous_state = Some(AppState::Main);
                self.state = AppState::Help;
            }
            KeyCode::Down => {
                let i = match self.table_state.selected() {
                    Some(i) => {
                        if i >= self.nodes.len() - 1 {
                            0
                        } else {
                            i + 1
                        }
                    }
                    None => 0,
                };
                if !self.nodes.is_empty() {
                    self.table_state.select(Some(i));
                }
            }
            KeyCode::Up => {
                let i = match self.table_state.selected() {
                    Some(i) => {
                        if i == 0 {
                            self.nodes.len() - 1
                        } else {
                            i - 1
                        }
                    }
                    None => 0,
                };
                if !self.nodes.is_empty() {
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

    fn handle_node_form_input(&mut self, key: KeyCode, _modifiers: KeyModifiers) -> bool {
        match key {
            KeyCode::Esc => return true,
            KeyCode::Enter => {
                if self.state == AppState::AddNode {
                    self.add_node_from_form();
                } else {
                    self.update_node_from_form();
                }
                return true;
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

    fn handle_credentials_input(&mut self, key: KeyCode) -> bool {
        match key {
            KeyCode::Esc | KeyCode::Char('q') => return true,
            KeyCode::Char('a') | KeyCode::Char('A') => {
                self.credential_form = CredentialForm::default();
                self.state = AppState::AddCredential;
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                if let Some(selected) = self.list_state.selected() {
                    self.delete_credential_index = Some(selected);
                    self.return_to_credentials_after_delete = true;
                    self.state = AppState::ConfirmDelete;
                }
            }
            KeyCode::Char('?') => {
                self.previous_state = Some(AppState::ManageCredentials);
                self.state = AppState::Help;
                return false;
            }
            KeyCode::Down => {
                let i = match self.list_state.selected() {
                    Some(i) => {
                        if i >= self.credentials.len() - 1 {
                            0
                        } else {
                            i + 1
                        }
                    }
                    None => 0,
                };
                if !self.credentials.is_empty() {
                    self.list_state.select(Some(i));
                }
            }
            KeyCode::Up => {
                let i = match self.list_state.selected() {
                    Some(i) => {
                        if i == 0 {
                            self.credentials.len() - 1
                        } else {
                            i - 1
                        }
                    }
                    None => 0,
                };
                if !self.credentials.is_empty() {
                    self.list_state.select(Some(i));
                }
            }
            _ => {}
        }
        false
    }

    fn handle_credential_form_input(&mut self, key: KeyCode) -> bool {
        match key {
            KeyCode::Esc => return true,
            KeyCode::Enter => {
                self.save_credential_from_form();
                return true;
            }
            KeyCode::Tab => {
                self.credential_form.current_field = (self.credential_form.current_field + 1)
                    % self.credential_form.get_field_count();
            }
            KeyCode::BackTab => {
                if self.credential_form.current_field == 0 {
                    self.credential_form.current_field = self.credential_form.get_field_count() - 1;
                } else {
                    self.credential_form.current_field -= 1;
                }
            }
            KeyCode::Left | KeyCode::Right => {
                // Handle arrow keys for Credential Type field
                if self.credential_form.current_field == 2 {
                    self.cycle_credential_type(key == KeyCode::Right);
                }
            }
            KeyCode::Char('?') => {
                self.previous_state = Some(AppState::AddCredential);
                self.state = AppState::Help;
                return false;
            }
            KeyCode::Char(c) => {
                self.add_char_to_credential_field(c);
            }
            KeyCode::Backspace => {
                self.remove_char_from_credential_field();
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
                } else if let Some(index) = self.delete_credential_index.take() {
                    self.delete_credential_at_index(index);
                }
                return true;
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.delete_node_index = None;
                self.delete_credential_index = None;
                return true;
            }
            _ => {}
        }
        false
    }

    fn handle_import_export_input(&mut self, key: KeyCode) -> bool {
        match key {
            KeyCode::Esc => return true,
            KeyCode::Enter => {
                if self.state == AppState::ImportNodes {
                    self.import_nodes();
                } else {
                    self.export_nodes();
                }
                return true;
            }
            KeyCode::Char('?') => {
                self.previous_state = Some(self.state);
                self.state = AppState::Help;
                return false;
            }
            KeyCode::Char(c) => {
                self.import_export_path.push(c);
            }
            KeyCode::Backspace => {
                self.import_export_path.pop();
            }
            _ => {}
        }
        false
    }

    // Helper methods

    fn cycle_credential_type(&mut self, forward: bool) {
        self.credential_form.credential_type = if forward {
            match self.credential_form.credential_type {
                CredentialTypeForm::Default => CredentialTypeForm::Password,
                CredentialTypeForm::Password => CredentialTypeForm::KeyFile,
                CredentialTypeForm::KeyFile => CredentialTypeForm::KeyData,
                CredentialTypeForm::KeyData => CredentialTypeForm::Default,
            }
        } else {
            match self.credential_form.credential_type {
                CredentialTypeForm::Default => CredentialTypeForm::KeyData,
                CredentialTypeForm::KeyData => CredentialTypeForm::KeyFile,
                CredentialTypeForm::KeyFile => CredentialTypeForm::Password,
                CredentialTypeForm::Password => CredentialTypeForm::Default,
            }
        };
    }

    fn add_char_to_credential_field(&mut self, c: char) {
        let field = self.credential_form.current_field;
        match field {
            0 => self.credential_form.name.push(c),
            1 => self.credential_form.description.push(c),
            2 => {
                // Cycle through credential types with Space only
                if c == ' ' {
                    self.cycle_credential_type(true);
                }
            }
            3 => match self.credential_form.credential_type {
                CredentialTypeForm::Default => {} // No username field for Default
                CredentialTypeForm::Password
                | CredentialTypeForm::KeyFile
                | CredentialTypeForm::KeyData => self.credential_form.username.push(c),
            },
            4 => match self.credential_form.credential_type {
                CredentialTypeForm::Default => {} // No field 4 for Default
                CredentialTypeForm::Password => self.credential_form.password.push(c),
                CredentialTypeForm::KeyFile => self.credential_form.ssh_key_path.push(c),
                CredentialTypeForm::KeyData => self.credential_form.ssh_key_data.push(c),
            },
            5 => match self.credential_form.credential_type {
                CredentialTypeForm::KeyFile | CredentialTypeForm::KeyData => {
                    self.credential_form.passphrase.push(c)
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn remove_char_from_credential_field(&mut self) {
        let field = self.credential_form.current_field;
        match field {
            0 => {
                self.credential_form.name.pop();
            }
            1 => {
                self.credential_form.description.pop();
            }
            2 => {} // Credential type
            3 => match self.credential_form.credential_type {
                CredentialTypeForm::Default => {}
                CredentialTypeForm::Password
                | CredentialTypeForm::KeyFile
                | CredentialTypeForm::KeyData => {
                    self.credential_form.username.pop();
                }
            },
            4 => match self.credential_form.credential_type {
                CredentialTypeForm::Default => {}
                CredentialTypeForm::Password => {
                    self.credential_form.password.pop();
                }
                CredentialTypeForm::KeyFile => {
                    self.credential_form.ssh_key_path.pop();
                }
                CredentialTypeForm::KeyData => {
                    self.credential_form.ssh_key_data.pop();
                }
            },
            5 => match self.credential_form.credential_type {
                CredentialTypeForm::KeyFile | CredentialTypeForm::KeyData => {
                    self.credential_form.passphrase.pop();
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn save_credential_from_form(&mut self) {
        use crate::credentials::{SensitiveString, SshCredential};
        use std::path::PathBuf;

        if self.credential_form.name.trim().is_empty() {
            self.set_status_message("Credential name cannot be empty");
            return;
        }

        let credential = match self.credential_form.credential_type {
            CredentialTypeForm::Default => SshCredential::Default,
            CredentialTypeForm::Password => {
                if self.credential_form.username.trim().is_empty()
                    || self.credential_form.password.trim().is_empty()
                {
                    self.set_status_message("Username and password are required");
                    return;
                }
                SshCredential::Password {
                    username: self.credential_form.username.clone(),
                    password: SensitiveString::new(self.credential_form.password.clone()),
                }
            }
            CredentialTypeForm::KeyFile => {
                if self.credential_form.username.trim().is_empty()
                    || self.credential_form.ssh_key_path.trim().is_empty()
                {
                    self.set_status_message("Username and SSH key path are required");
                    return;
                }
                SshCredential::Key {
                    username: self.credential_form.username.clone(),
                    private_key_path: PathBuf::from(&self.credential_form.ssh_key_path),
                    passphrase: if self.credential_form.passphrase.trim().is_empty() {
                        None
                    } else {
                        Some(SensitiveString::new(
                            self.credential_form.passphrase.clone(),
                        ))
                    },
                }
            }
            CredentialTypeForm::KeyData => {
                if self.credential_form.username.trim().is_empty()
                    || self.credential_form.ssh_key_data.trim().is_empty()
                {
                    self.set_status_message("Username and SSH key data are required");
                    return;
                }
                SshCredential::KeyData {
                    username: self.credential_form.username.clone(),
                    private_key_data: SensitiveString::new(
                        self.credential_form.ssh_key_data.clone(),
                    ),
                    passphrase: if self.credential_form.passphrase.trim().is_empty() {
                        None
                    } else {
                        Some(SensitiveString::new(
                            self.credential_form.passphrase.clone(),
                        ))
                    },
                }
            }
        };

        let description = if self.credential_form.description.trim().is_empty() {
            None
        } else {
            Some(self.credential_form.description.clone())
        };

        match self.credential_store.store_credential(
            self.credential_form.name.clone(),
            description,
            credential,
        ) {
            Ok(_) => {
                self.set_status_message("Credential saved successfully");
                self.reload_credentials();
                self.credential_form = CredentialForm::default();
            }
            Err(e) => {
                self.set_status_message(format!("Failed to save credential: {}", e));
            }
        }
    }

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
            3 => {} // Credential selection - would need dropdown
            4 => match self.node_form.monitor_type {
                MonitorTypeForm::Http => self.node_form.http_url.push(c),
                MonitorTypeForm::Ping => self.node_form.ping_host.push(c),
                MonitorTypeForm::Tcp => self.node_form.tcp_host.push(c),
            },
            5 => match self.node_form.monitor_type {
                MonitorTypeForm::Http => self.node_form.http_expected_status.push(c),
                MonitorTypeForm::Ping => self.node_form.ping_count.push(c),
                MonitorTypeForm::Tcp => self.node_form.tcp_port.push(c),
            },
            6 => match self.node_form.monitor_type {
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
            3 => {} // Credential
            4 => match self.node_form.monitor_type {
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
            5 => match self.node_form.monitor_type {
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
            6 => match self.node_form.monitor_type {
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

    fn add_node_from_form(&mut self) {
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
                    credential_id: self.node_form.credential_id.clone(),
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
                    }
                    Err(e) => {
                        self.set_status_message(format!("Error adding node: {}", e));
                    }
                }
            }
            Err(e) => {
                self.set_status_message(format!("Invalid data: {}", e));
            }
        }
    }

    fn update_node_from_form(&mut self) {
        if let Some(node_id) = self.editing_node_id {
            match self.node_form.to_node_detail() {
                Ok(detail) => {
                    if let Some(node) = self.nodes.iter_mut().find(|n| n.id == Some(node_id)) {
                        node.name = self.node_form.name.clone();
                        node.detail = detail;
                        node.monitoring_interval =
                            self.node_form.monitoring_interval.parse().unwrap_or(5);
                        node.credential_id = self.node_form.credential_id.clone();

                        if let Err(e) = self.database.update_node(node) {
                            self.set_status_message(format!("Error updating node: {}", e));
                        } else {
                            if let Some(handle) = &self.monitoring_handle {
                                let _ = handle
                                    .config_tx
                                    .send(NodeConfigUpdate::Update(node.clone()));
                            }
                            self.set_status_message("Node updated successfully");
                        }
                    }
                }
                Err(e) => {
                    self.set_status_message(format!("Invalid data: {}", e));
                }
            }
        }
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

    fn delete_credential_at_index(&mut self, index: usize) {
        if let Some(credential) = self.credentials.get(index) {
            if self
                .credential_store
                .delete_credential(&credential.id)
                .is_ok()
            {
                self.credentials.remove(index);
                self.set_status_message("Credential deleted");

                // Adjust selection
                if self.credentials.is_empty() {
                    self.list_state.select(None);
                } else if index >= self.credentials.len() {
                    self.list_state.select(Some(self.credentials.len() - 1));
                }
            } else {
                self.set_status_message("Failed to delete credential");
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
        info!("Starting monitoring thread");
        let (stop_tx, stop_rx) = mpsc::channel();
        let (config_tx, config_rx) = mpsc::channel();
        let db = self.database.clone();
        let update_tx = self.update_tx.clone();

        let initial_nodes = self.nodes.clone();

        thread::spawn(move || {
            let mut last_check_times: HashMap<i64, Instant> = HashMap::new();
            // Initialize previous_statuses with actual last known status from database
            // This prevents recording duplicate entries when the app restarts
            let mut previous_statuses: HashMap<i64, NodeStatus> = initial_nodes
                .iter()
                .filter_map(|n| {
                    n.id.and_then(|id| {
                        // Try to get the last monitoring result from database
                        db.get_latest_monitoring_result(id)
                            .ok()
                            .flatten()
                            .map(|result| (id, result.status))
                            // If no previous result, use the node's current status
                            .or(Some((id, n.status)))
                    })
                })
                .collect();
            let mut last_status_change_times: HashMap<i64, chrono::DateTime<Utc>> = HashMap::new();
            let mut current_nodes = initial_nodes.clone();
            let runtime = tokio::runtime::Runtime::new().unwrap();

            loop {
                // Check for configuration updates
                while let Ok(config_update) = config_rx.try_recv() {
                    match config_update {
                        NodeConfigUpdate::Add(node) => {
                            if !current_nodes.iter().any(|n| n.id == node.id) {
                                // Initialize new node's previous status from database or use current status
                                if let Some(node_id) = node.id {
                                    let status = db
                                        .get_latest_monitoring_result(node_id)
                                        .ok()
                                        .flatten()
                                        .map(|result| result.status)
                                        .unwrap_or(node.status);
                                    previous_statuses.insert(node_id, status);
                                }
                                current_nodes.push(node);
                            }
                        }
                        NodeConfigUpdate::Update(updated_node) => {
                            if let Some(node) =
                                current_nodes.iter_mut().find(|n| n.id == updated_node.id)
                            {
                                let status = node.status;
                                let last_check = node.last_check;
                                let response_time = node.response_time;

                                *node = updated_node;
                                node.status = status;
                                node.last_check = last_check;
                                node.response_time = response_time;

                                if let Some(node_id) = node.id {
                                    last_check_times.remove(&node_id);
                                }
                            }
                        }
                        NodeConfigUpdate::Delete(node_id) => {
                            current_nodes.retain(|n| n.id != Some(node_id));
                            last_check_times.remove(&node_id);
                            previous_statuses.remove(&node_id);
                            last_status_change_times.remove(&node_id);
                        }
                    }
                }

                for node in &mut current_nodes {
                    let node_id = node.id.unwrap_or(0);
                    if node_id == 0 {
                        continue;
                    }

                    let now = Instant::now();
                    let should_check = last_check_times.get(&node_id).is_none_or(|last_check| {
                        now.duration_since(*last_check).as_secs() >= node.monitoring_interval
                    });

                    if should_check {
                        last_check_times.insert(node_id, now);
                        let previous_status = previous_statuses.get(&node_id).cloned();
                        let result = runtime.block_on(check_node(node));

                        if let Ok(mut check_result) = result {
                            let new_status = check_result.status;

                            if let Some(prev_status) = previous_status {
                                if prev_status != new_status {
                                    let current_time = Utc::now();
                                    let duration_ms =
                                        last_status_change_times.get(&node_id).map(|last_change| {
                                            StatusChange::calculate_duration(
                                                *last_change,
                                                current_time,
                                            )
                                        });

                                    let status_change = StatusChange {
                                        id: None,
                                        node_id,
                                        from_status: prev_status,
                                        to_status: new_status,
                                        changed_at: current_time,
                                        duration_ms,
                                    };

                                    let _ = db.add_status_change(&status_change);
                                    last_status_change_times.insert(node_id, current_time);
                                }
                            }

                            previous_statuses.insert(node_id, new_status);
                            node.status = check_result.status;
                            node.last_check = Some(check_result.timestamp);
                            node.response_time = check_result.response_time;
                            check_result.node_id = node_id;

                            let _ = db.update_node(node);

                            // Only record monitoring result if status changed
                            // This prevents spam when app restarts with unchanged status
                            if let Some(prev_status) = previous_status {
                                if prev_status != new_status {
                                    let _ = db.add_monitoring_result(&check_result);
                                }
                            } else {
                                // First check ever for this node, record it
                                let _ = db.add_monitoring_result(&check_result);
                            }

                            if update_tx.send(node.clone()).is_err() {
                                break;
                            }
                        }
                    }
                }

                // Check for stop signal
                match stop_rx.recv_timeout(Duration::from_secs(1)) {
                    Ok(_) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
            }
        });

        self.monitoring_handle = Some(MonitoringHandle { stop_tx, config_tx });
        self.set_status_message("Monitoring started");
    }

    fn stop_monitoring(&mut self) -> Result<()> {
        if let Some(handle) = self.monitoring_handle.take() {
            handle.stop_tx.send(())?;
            self.set_status_message("Monitoring stopped");
        }
        Ok(())
    }

    fn import_nodes(&mut self) {
        let path = PathBuf::from(&self.import_export_path);
        match std::fs::read_to_string(&path) {
            Ok(data) => match serde_json::from_str::<Vec<NodeImport>>(&data) {
                Ok(nodes_to_import) => {
                    let mut count = 0;
                    for import in nodes_to_import {
                        let mut node = Node {
                            id: None,
                            name: import.name,
                            detail: import.detail,
                            status: NodeStatus::Offline,
                            last_check: None,
                            response_time: None,
                            monitoring_interval: import.monitoring_interval,
                            credential_id: import.credential_id,
                        };
                        if let Ok(id) = self.database.add_node(&node) {
                            node.id = Some(id);
                            if let Some(handle) = &self.monitoring_handle {
                                let _ = handle.config_tx.send(NodeConfigUpdate::Add(node.clone()));
                            }
                            self.nodes.push(node);
                            count += 1;
                        }
                    }
                    self.set_status_message(format!("Imported {} nodes", count));
                }
                Err(e) => {
                    self.set_status_message(format!("Failed to parse import file: {}", e));
                }
            },
            Err(e) => {
                self.set_status_message(format!("Failed to read import file: {}", e));
            }
        }
    }

    fn export_nodes(&mut self) {
        let path = PathBuf::from(&self.import_export_path);
        let nodes_to_export: Vec<NodeImport> = self
            .nodes
            .iter()
            .map(|node| NodeImport {
                name: node.name.clone(),
                detail: node.detail.clone(),
                monitoring_interval: node.monitoring_interval,
                credential_id: node.credential_id.clone(),
            })
            .collect();

        match serde_json::to_string_pretty(&nodes_to_export) {
            Ok(data) => {
                if let Err(e) = std::fs::write(&path, data) {
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

    fn load_status_history(&mut self, node_id: i64) {
        match self.database.get_status_changes(node_id, Some(50)) {
            Ok(changes) => {
                self.status_changes = changes;
                // Select first row if there are any changes
                if !self.status_changes.is_empty() {
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

    fn reload_credentials(&mut self) {
        match self.credential_store.list_credentials() {
            Ok(credentials) => {
                self.credentials = credentials;
            }
            Err(e) => {
                error!("Failed to reload credentials: {}", e);
            }
        }
    }

    fn set_status_message(&mut self, message: impl Into<String>) {
        self.status_message = Some((message.into(), Instant::now()));
    }
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
