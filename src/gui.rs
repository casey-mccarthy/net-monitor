use crate::connection::{AuthenticatedConnectionStrategy, ConnectionStrategy};
use crate::credentials::{
    CredentialStore, CredentialSummary, FileCredentialStore, SensitiveString, SshCredential,
};
use crate::database::Database;
use crate::models::{MonitorDetail, Node, NodeImport, NodeStatus};
use crate::monitor::check_node;
use anyhow::Result;
use directories::ProjectDirs;
use eframe::egui::{self, Color32, Context, Grid, RichText, ScrollArea, Ui, Window};
use rfd::FileDialog;
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use tracing::{error, info};

#[derive(Clone, Copy, PartialEq)]
enum MonitorTypeForm {
    Http,
    Ping,
}

impl fmt::Display for MonitorTypeForm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MonitorTypeForm::Http => write!(f, "HTTP"),
            MonitorTypeForm::Ping => write!(f, "Ping"),
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
}

/// Form data for creating/editing credentials
#[derive(Clone)]
struct CredentialForm {
    name: String,
    description: String,
    credential_type: CredentialTypeForm,
    username: String,
    password: String,
    ssh_key_path: String,
    ssh_key_data: String,
    passphrase: String,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum CredentialTypeForm {
    Default,
    Password,
    KeyFile,
    KeyData,
}

impl Default for NodeForm {
    fn default() -> Self {
        Self {
            name: String::new(),
            monitor_type: MonitorTypeForm::Http,
            monitoring_interval: "60".to_string(),
            credential_id: None,
            http_url: "https://".to_string(),
            http_expected_status: "200".to_string(),
            ping_host: String::new(),
            ping_count: "4".to_string(),
            ping_timeout: "5".to_string(),
        }
    }
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
        }
    }
}

impl fmt::Display for CredentialTypeForm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CredentialTypeForm::Default => write!(f, "System Default"),
            CredentialTypeForm::Password => write!(f, "Username/Password"),
            CredentialTypeForm::KeyFile => write!(f, "SSH Key File"),
            CredentialTypeForm::KeyData => write!(f, "SSH Key Data"),
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
        }
        form
    }
}

/// Main application state
pub struct NetworkMonitorApp {
    database: Database,
    nodes: Vec<Node>,
    editing_node: Option<(i64, NodeForm)>,
    show_add_node: bool,
    new_node_form: NodeForm,
    status_message: Option<(String, f64)>,
    monitoring_handle: Option<MonitoringHandle>,
    update_rx: mpsc::Receiver<Node>,
    update_tx: mpsc::Sender<Node>,
    updated_nodes: HashMap<i64, Instant>,
    // Credential management
    credential_store: Box<dyn CredentialStore>,
    credentials: Vec<CredentialSummary>,
    show_credentials: bool,
    show_add_credential: bool,
    show_about: bool,
    #[allow(dead_code)]
    editing_credential: Option<String>,
    new_credential_form: CredentialForm,
    pending_credential_action: Option<CredentialAction>,
}

struct MonitoringHandle {
    stop_tx: mpsc::Sender<()>,
    config_tx: mpsc::Sender<NodeConfigUpdate>,
    #[allow(dead_code)]
    thread: thread::JoinHandle<()>,
}

impl NetworkMonitorApp {
    pub fn new(database: Database) -> Result<Self> {
        let nodes = database.get_all_nodes()?;
        let (update_tx, update_rx) = mpsc::channel();

        // Initialize credential store - use file store since keyring list_credentials is not implemented
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

        // Load existing credentials
        let credentials = credential_store.list_credentials().unwrap_or_default();

        Ok(Self {
            database,
            nodes,
            editing_node: None,
            show_add_node: false,
            new_node_form: NodeForm::default(),
            status_message: None,
            monitoring_handle: None,
            update_rx,
            update_tx,
            updated_nodes: HashMap::new(),
            credential_store,
            credentials,
            show_credentials: false,
            show_add_credential: false,
            show_about: false,
            editing_credential: None,
            new_credential_form: CredentialForm::default(),
            pending_credential_action: None,
        })
    }
}

impl eframe::App for NetworkMonitorApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        if let Some((_, time)) = self.status_message {
            if ctx.input(|i| i.time) > time + 5.0 {
                self.status_message = None;
            }
        }

        while let Ok(updated_node) = self.update_rx.try_recv() {
            if let Some(node) = self.nodes.iter_mut().find(|n| n.id == updated_node.id) {
                if let Some(node_id) = updated_node.id {
                    self.updated_nodes.insert(node_id, Instant::now());
                }
                *node = updated_node;
            }
        }

        // Clean up old flash animations (older than 1 second)
        let now = Instant::now();
        self.updated_nodes
            .retain(|_, timestamp| now.duration_since(*timestamp).as_millis() < 1000);

        self.show_main_window(ctx);
        self.show_add_node_window(ctx);
        self.show_edit_node_window(ctx);
        self.show_credentials_window(ctx);
        self.show_add_credential_window(ctx);
        self.show_about_window(ctx);

        // Process pending credential actions
        self.process_pending_credential_action();

        ctx.request_repaint_after(Duration::from_millis(500));
    }
}

impl NetworkMonitorApp {
    fn show_main_window(&mut self, ctx: &Context) {
        // Add menu bar
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("Help", |ui| {
                    if ui.button("About").clicked() {
                        self.show_about = true;
                        ui.close_menu();
                    }
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Network Monitor");
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Add Node").clicked() {
                    self.show_add_node = true;
                    self.new_node_form = NodeForm::default();
                }
                if ui.button("Credentials").clicked() {
                    self.show_credentials = true;
                    self.reload_credentials();
                }
                if ui.button("Import Nodes").clicked() {
                    self.import_nodes();
                }
                if ui.button("Export Nodes").clicked() {
                    self.export_nodes();
                }
                if ui.button("Open Log").clicked() {
                    self.open_log_file();
                }
                self.monitoring_toggle_button(ui);
            });
            ui.separator();

            if let Some((msg, _)) = &self.status_message {
                ui.label(RichText::new(msg).color(Color32::GREEN));
            }

            self.show_node_list(ui);
        });
    }

    fn show_node_list(&mut self, ui: &mut Ui) {
        let mut action = None;
        ScrollArea::vertical().show(ui, |ui| {
            Grid::new("node_list")
                .num_columns(7)
                .spacing([20.0, 10.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label(RichText::new("Name").strong());
                    ui.label(RichText::new("Target").strong());
                    ui.label(RichText::new("Type").strong());
                    ui.label(RichText::new("Status").strong());
                    ui.label(RichText::new("Last Check").strong());
                    ui.label(RichText::new("Connect").strong());
                    ui.label(RichText::new("Actions").strong());
                    ui.end_row();

                    for (i, node) in self.nodes.iter().enumerate() {
                        // Check if this node was recently updated for flash effect
                        let flash_intensity = if let Some(node_id) = node.id {
                            if let Some(update_time) = self.updated_nodes.get(&node_id) {
                                let elapsed =
                                    Instant::now().duration_since(*update_time).as_millis();
                                if elapsed < 500 {
                                    // Fade from 1.0 to 0.0 over 500ms
                                    1.0 - (elapsed as f32 / 500.0)
                                } else {
                                    0.0
                                }
                            } else {
                                0.0
                            }
                        } else {
                            0.0
                        };

                        ui.label(&node.name);
                        let target = match &node.detail {
                            MonitorDetail::Http { url, .. } => url.as_str(),
                            MonitorDetail::Ping { host, .. } => host.as_str(),
                        };
                        ui.label(target);
                        ui.label(node.detail.to_string());
                        let status_color = match node.status {
                            NodeStatus::Online => Color32::GREEN,
                            NodeStatus::Offline => Color32::RED,
                            NodeStatus::Unknown => Color32::YELLOW,
                        };
                        ui.colored_label(status_color, node.status.to_string());

                        // Last check with glow effect
                        let last_check_str = node
                            .last_check
                            .map(|t| {
                                t.with_timezone(&chrono::Local)
                                    .format("%Y-%m-%d %H:%M:%S")
                                    .to_string()
                            })
                            .unwrap_or_else(|| "Never".to_string());

                        if flash_intensity > 0.0 {
                            // Create a glowing effect by rendering the text multiple times with different alphas
                            let response =
                                ui.allocate_response(ui.available_size(), egui::Sense::hover());
                            let text_pos = response.rect.min;
                            let painter = ui.painter();
                            let font_id = egui::TextStyle::Body.resolve(ui.style());

                            // Draw multiple layers for glow effect
                            let glow_layers = [
                                (3.0, 0.02), // Outermost, most faded
                                (2.0, 0.04),
                                (1.0, 0.06),
                                (0.5, 0.08),
                            ];

                            for (offset, alpha_multiplier) in glow_layers.iter() {
                                let glow_alpha = (flash_intensity * alpha_multiplier * 255.0) as u8;
                                let glow_color =
                                    Color32::from_rgba_unmultiplied(255, 255, 255, glow_alpha);

                                // Draw glow in multiple directions for radial effect
                                let offsets = [
                                    egui::Vec2::new(*offset, 0.0),
                                    egui::Vec2::new(-*offset, 0.0),
                                    egui::Vec2::new(0.0, *offset),
                                    egui::Vec2::new(0.0, -*offset),
                                    egui::Vec2::new(*offset * 0.7, *offset * 0.7),
                                    egui::Vec2::new(-*offset * 0.7, *offset * 0.7),
                                    egui::Vec2::new(*offset * 0.7, -*offset * 0.7),
                                    egui::Vec2::new(-*offset * 0.7, -*offset * 0.7),
                                ];

                                for offset_vec in offsets.iter() {
                                    painter.text(
                                        text_pos + *offset_vec,
                                        egui::Align2::LEFT_TOP,
                                        &last_check_str,
                                        font_id.clone(),
                                        glow_color,
                                    );
                                }
                            }

                            // Draw the main text on top
                            painter.text(
                                text_pos,
                                egui::Align2::LEFT_TOP,
                                &last_check_str,
                                font_id,
                                ui.visuals().text_color(),
                            );
                        } else {
                            ui.label(last_check_str);
                        }

                        // Connect button
                        if ui.button("Connect").clicked() {
                            action = Some(NodeAction::Connect(i));
                        }

                        ui.horizontal(|ui| {
                            if ui.button("Edit").clicked() {
                                action = Some(NodeAction::Edit(i));
                            }
                            if ui.button("Delete").clicked() {
                                action = Some(NodeAction::Delete(i));
                            }
                        });
                        ui.end_row();
                    }
                });
        });

        if let Some(action) = action {
            self.handle_node_action(action);
        }
    }

    fn show_add_node_window(&mut self, ctx: &Context) {
        let mut close_window = false;
        if self.show_add_node {
            let mut is_open = self.show_add_node;
            let mut form = std::mem::take(&mut self.new_node_form);
            Window::new("Add Node")
                .open(&mut is_open)
                .resizable(true)
                .show(ctx, |ui| {
                    self.node_form_ui(ui, &mut form);
                    if ui.button("Add").clicked() {
                        self.add_node_from_form(&form.clone());
                        close_window = true;
                    }
                });
            self.new_node_form = form;
            if !is_open || close_window {
                self.show_add_node = false;
            }
        }
    }

    fn show_edit_node_window(&mut self, ctx: &Context) {
        if let Some((node_id, _)) = self.editing_node {
            let mut is_open = true;
            let mut close_window = false;
            let mut form = self.editing_node.as_ref().unwrap().1.clone();
            Window::new("Edit Node")
                .open(&mut is_open)
                .resizable(true)
                .show(ctx, |ui| {
                    ui.heading(format!("Editing: {}", form.name));
                    ui.separator();
                    self.node_form_ui(ui, &mut form);
                    if ui.button("Save").clicked() {
                        self.update_node_from_form(node_id, &form.clone());
                        close_window = true;
                    }
                });
            if let Some(edit) = &mut self.editing_node {
                edit.1 = form;
            }
            if !is_open || close_window {
                self.editing_node = None;
            }
        }
    }

    fn node_form_ui(&mut self, ui: &mut Ui, form: &mut NodeForm) {
        Grid::new("node_form").num_columns(2).show(ui, |ui| {
            ui.label("Name:");
            ui.text_edit_singleline(&mut form.name);
            ui.end_row();
            ui.label("Monitoring Interval (s):");
            ui.text_edit_singleline(&mut form.monitoring_interval);
            ui.end_row();
            ui.label("Monitor Type:");
            egui::ComboBox::from_id_source("monitor_type_combo")
                .selected_text(form.monitor_type.to_string())
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut form.monitor_type, MonitorTypeForm::Http, "HTTP");
                    ui.selectable_value(&mut form.monitor_type, MonitorTypeForm::Ping, "Ping");
                });
            ui.end_row();

            ui.label("SSH Credential:");
            let selected_text = if let Some(ref cred_id) = form.credential_id {
                // Find the credential name by ID
                self.credentials
                    .iter()
                    .find(|c| c.id.to_string().as_str() == cred_id)
                    .map(|c| c.name.clone())
                    .unwrap_or_else(|| cred_id.clone())
            } else {
                "None (Use default SSH)".to_string()
            };
            egui::ComboBox::from_id_source("credential_combo")
                .selected_text(selected_text)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut form.credential_id, None, "None (Use default SSH)");
                    for credential in &self.credentials {
                        let cred_id_str = credential.id.to_string();
                        ui.selectable_value(
                            &mut form.credential_id,
                            Some(cred_id_str.clone()),
                            &credential.name,
                        );
                    }
                });
            ui.end_row();

            match form.monitor_type {
                MonitorTypeForm::Http => {
                    ui.label("URL:");
                    ui.text_edit_singleline(&mut form.http_url);
                    ui.end_row();
                    ui.label("Expected Status:");
                    ui.text_edit_singleline(&mut form.http_expected_status);
                    ui.end_row();
                }
                MonitorTypeForm::Ping => {
                    ui.label("Host:");
                    ui.text_edit_singleline(&mut form.ping_host);
                    ui.end_row();
                    ui.label("Count:");
                    ui.text_edit_singleline(&mut form.ping_count);
                    ui.end_row();
                    ui.label("Timeout (s):");
                    ui.text_edit_singleline(&mut form.ping_timeout);
                    ui.end_row();
                }
            }
        });
    }

    fn add_node_from_form(&mut self, form: &NodeForm) {
        match form.to_node_detail() {
            Ok(detail) => {
                let node = Node {
                    id: None,
                    name: form.name.clone(),
                    detail,
                    status: NodeStatus::Unknown,
                    last_check: None,
                    response_time: None,
                    monitoring_interval: form.monitoring_interval.parse().unwrap_or(60),
                    credential_id: form.credential_id.clone(),
                };
                match self.database.add_node(&node) {
                    Ok(id) => {
                        let mut new_node = node;
                        new_node.id = Some(id);

                        // Send update to monitoring thread if it's running
                        if let Some(handle) = &self.monitoring_handle {
                            if let Err(e) = handle
                                .config_tx
                                .send(NodeConfigUpdate::Add(new_node.clone()))
                            {
                                error!("Failed to send node addition to monitoring thread: {}", e);
                            }
                        }

                        self.nodes.push(new_node);
                        self.set_status_message("Node added successfully".to_string());
                    }
                    Err(e) => {
                        error!("Failed to add node to database: {}", e);
                        self.set_status_message(format!("Error adding node: {}", e));
                    }
                }
            }
            Err(e) => {
                error!("Invalid form data: {}", e);
                self.set_status_message(format!("Invalid data: {}", e));
            }
        }
    }

    fn update_node_from_form(&mut self, node_id: i64, form: &NodeForm) {
        match form.to_node_detail() {
            Ok(detail) => {
                if let Some(node) = self.nodes.iter_mut().find(|n| n.id == Some(node_id)) {
                    node.name = form.name.clone();
                    node.detail = detail;
                    node.monitoring_interval = form.monitoring_interval.parse().unwrap_or(60);
                    node.credential_id = form.credential_id.clone();

                    if let Err(e) = self.database.update_node(node) {
                        error!("Failed to update node in database: {}", e);
                        self.set_status_message(format!("Error updating node: {}", e));
                    } else {
                        // Send update to monitoring thread if it's running
                        if let Some(handle) = &self.monitoring_handle {
                            if let Err(e) = handle
                                .config_tx
                                .send(NodeConfigUpdate::Update(node.clone()))
                            {
                                error!("Failed to send node update to monitoring thread: {}", e);
                            }
                        }
                        self.set_status_message("Node updated successfully".to_string());
                    }
                }
            }
            Err(e) => {
                error!("Invalid form data: {}", e);
                self.set_status_message(format!("Invalid data: {}", e));
            }
        }
    }

    fn handle_node_action(&mut self, action: NodeAction) {
        match action {
            NodeAction::Edit(index) => {
                if let Some(node) = self.nodes.get(index) {
                    if let Some(id) = node.id {
                        let form = NodeForm::from_node(node);
                        self.editing_node = Some((id, form));
                    }
                }
            }
            NodeAction::Delete(index) => {
                if let Some(node) = self.nodes.get(index).cloned() {
                    if let Some(id) = node.id {
                        if self.database.delete_node(id).is_ok() {
                            // Send update to monitoring thread if it's running
                            if let Some(handle) = &self.monitoring_handle {
                                if let Err(e) = handle.config_tx.send(NodeConfigUpdate::Delete(id))
                                {
                                    error!(
                                        "Failed to send node deletion to monitoring thread: {}",
                                        e
                                    );
                                }
                            }
                            self.nodes.remove(index);
                            self.set_status_message("Node deleted".to_string());
                        } else {
                            error!("Failed to delete node with id {}", id);
                        }
                    }
                }
            }
            NodeAction::Connect(index) => {
                if let Some(node) = self.nodes.get(index) {
                    let target = node.detail.get_connection_target();
                    let connection_type = node.detail.get_connection_type();

                    match connection_type {
                        crate::connection::ConnectionType::Http => {
                            // HTTP hosts always open web browser, regardless of credentials
                            let http_strategy = crate::connection::HttpConnectionStrategy;
                            match http_strategy.connect(target) {
                                Ok(_) => {
                                    info!("Successfully opened {} in web browser", target);
                                    self.set_status_message(format!(
                                        "Opening {} in browser...",
                                        target
                                    ));
                                }
                                Err(e) => {
                                    error!("Failed to open {} in browser: {}", target, e);
                                    self.set_status_message(format!(
                                        "Failed to open in browser: {}",
                                        e
                                    ));
                                }
                            }
                        }
                        crate::connection::ConnectionType::Ping => {
                            // ICMP/Ping hosts use SSH connection with credentials if available
                            if let Some(ref credential_id) = node.credential_id {
                                match self
                                    .credential_store
                                    .get_credential(&credential_id.parse().unwrap_or_default())
                                {
                                    Ok(Some(stored_credential)) => {
                                        println!(
                                            "Connecting to {} using credential: {}",
                                            target, stored_credential.name
                                        );
                                        let ssh_strategy =
                                            crate::connection::SshConnectionStrategy::new();
                                        match ssh_strategy.connect_with_credentials(
                                            target,
                                            &stored_credential.credential,
                                        ) {
                                            Ok(_) => {
                                                info!("Successfully initiated authenticated SSH connection to {}", target);
                                                self.set_status_message(format!(
                                                    "Connecting to {} with SSH credentials...",
                                                    target
                                                ));
                                            }
                                            Err(e) => {
                                                error!("Failed to connect to {} with SSH credentials: {}", target, e);
                                                self.set_status_message(format!(
                                                    "Failed to connect with SSH credentials: {}",
                                                    e
                                                ));
                                            }
                                        }
                                    }
                                    Ok(None) => {
                                        error!("Credential {} not found", credential_id);
                                        self.set_status_message("Credential not found".to_string());
                                    }
                                    Err(e) => {
                                        error!(
                                            "Failed to retrieve credential {}: {}",
                                            credential_id, e
                                        );
                                        self.set_status_message(format!(
                                            "Failed to retrieve credential: {}",
                                            e
                                        ));
                                    }
                                }
                            } else {
                                // No credentials, use default SSH connection for ICMP hosts
                                let ssh_strategy = crate::connection::SshConnectionStrategy::new();
                                match ssh_strategy.connect(target) {
                                    Ok(_) => {
                                        info!(
                                            "Successfully initiated SSH connection to {}",
                                            target
                                        );
                                        self.set_status_message(format!(
                                            "Connecting to {} via SSH...",
                                            target
                                        ));
                                    }
                                    Err(e) => {
                                        error!("Failed to connect to {} via SSH: {}", target, e);
                                        self.set_status_message(format!(
                                            "Failed to connect via SSH: {}",
                                            e
                                        ));
                                    }
                                }
                            }
                        }
                        crate::connection::ConnectionType::Ssh => {
                            // SSH hosts use SSH connection with credentials if available
                            if let Some(ref credential_id) = node.credential_id {
                                match self
                                    .credential_store
                                    .get_credential(&credential_id.parse().unwrap_or_default())
                                {
                                    Ok(Some(stored_credential)) => {
                                        println!(
                                            "Connecting to {} using credential: {}",
                                            target, stored_credential.name
                                        );
                                        let ssh_strategy =
                                            crate::connection::SshConnectionStrategy::new();
                                        match ssh_strategy.connect_with_credentials(
                                            target,
                                            &stored_credential.credential,
                                        ) {
                                            Ok(_) => {
                                                info!("Successfully initiated authenticated SSH connection to {}", target);
                                                self.set_status_message(format!(
                                                    "Connecting to {} with SSH credentials...",
                                                    target
                                                ));
                                            }
                                            Err(e) => {
                                                error!("Failed to connect to {} with SSH credentials: {}", target, e);
                                                self.set_status_message(format!(
                                                    "Failed to connect with SSH credentials: {}",
                                                    e
                                                ));
                                            }
                                        }
                                    }
                                    Ok(None) => {
                                        error!("Credential {} not found", credential_id);
                                        self.set_status_message("Credential not found".to_string());
                                    }
                                    Err(e) => {
                                        error!(
                                            "Failed to retrieve credential {}: {}",
                                            credential_id, e
                                        );
                                        self.set_status_message(format!(
                                            "Failed to retrieve credential: {}",
                                            e
                                        ));
                                    }
                                }
                            } else {
                                // No credentials, use default SSH connection
                                let ssh_strategy = crate::connection::SshConnectionStrategy::new();
                                match ssh_strategy.connect(target) {
                                    Ok(_) => {
                                        info!(
                                            "Successfully initiated SSH connection to {}",
                                            target
                                        );
                                        self.set_status_message(format!(
                                            "Connecting to {} via SSH...",
                                            target
                                        ));
                                    }
                                    Err(e) => {
                                        error!("Failed to connect to {} via SSH: {}", target, e);
                                        self.set_status_message(format!(
                                            "Failed to connect via SSH: {}",
                                            e
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn monitoring_toggle_button(&mut self, ui: &mut Ui) {
        if self.monitoring_handle.is_some() {
            if ui
                .button(RichText::new("Stop Monitoring").color(Color32::RED))
                .clicked()
            {
                self.stop_monitoring();
            }
        } else if ui.button("Start Monitoring").clicked() {
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

        let thread = thread::spawn(move || {
            let mut last_check_times: HashMap<i64, Instant> = HashMap::new();
            let mut previous_statuses: HashMap<i64, NodeStatus> = HashMap::new();
            let mut current_nodes = initial_nodes.clone();
            let runtime = tokio::runtime::Runtime::new().unwrap();

            loop {
                // Check for configuration updates
                while let Ok(config_update) = config_rx.try_recv() {
                    match config_update {
                        NodeConfigUpdate::Add(node) => {
                            info!("Adding new node to monitoring: {}", node.name);
                            if !current_nodes.iter().any(|n| n.id == node.id) {
                                current_nodes.push(node);
                            }
                        }
                        NodeConfigUpdate::Update(updated_node) => {
                            info!("Updating node configuration: {}", updated_node.name);
                            if let Some(node) =
                                current_nodes.iter_mut().find(|n| n.id == updated_node.id)
                            {
                                // Preserve the current status and last check time
                                let status = node.status;
                                let last_check = node.last_check;
                                let response_time = node.response_time;

                                *node = updated_node;
                                node.status = status;
                                node.last_check = last_check;
                                node.response_time = response_time;

                                // Reset the check timer if monitoring interval changed
                                if let Some(node_id) = node.id {
                                    last_check_times.remove(&node_id);
                                }
                            }
                        }
                        NodeConfigUpdate::Delete(node_id) => {
                            info!("Removing node from monitoring: ID {}", node_id);
                            current_nodes.retain(|n| n.id != Some(node_id));
                            last_check_times.remove(&node_id);
                            previous_statuses.remove(&node_id);
                        }
                    }
                }

                let mut nodes_to_check = current_nodes.clone();

                for node in &mut nodes_to_check {
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

                        match result {
                            Ok(mut check_result) => {
                                let new_status = check_result.status;

                                // Log the check result with status
                                match new_status {
                                    NodeStatus::Online => {
                                        info!("Node '{}' checked - Status: UP", node.name);
                                    }
                                    NodeStatus::Offline => {
                                        info!("Node '{}' checked - Status: DOWN", node.name);
                                    }
                                    NodeStatus::Unknown => {
                                        info!("Node '{}' checked - Status: UNKNOWN", node.name);
                                    }
                                }

                                // Log status changes
                                if let Some(prev_status) = previous_status {
                                    if prev_status != new_status {
                                        match (prev_status, new_status) {
                                            (NodeStatus::Online, NodeStatus::Offline) => {
                                                error!(
                                                    "ALERT: Node '{}' went DOWN (was UP)",
                                                    node.name
                                                );
                                            }
                                            (NodeStatus::Offline, NodeStatus::Online) => {
                                                info!(
                                                    "RECOVERY: Node '{}' is back UP (was DOWN)",
                                                    node.name
                                                );
                                            }
                                            (NodeStatus::Unknown, NodeStatus::Online) => {
                                                info!(
                                                    "Node '{}' is now UP (was UNKNOWN)",
                                                    node.name
                                                );
                                            }
                                            (NodeStatus::Unknown, NodeStatus::Offline) => {
                                                error!(
                                                    "Node '{}' is now DOWN (was UNKNOWN)",
                                                    node.name
                                                );
                                            }
                                            (NodeStatus::Online, NodeStatus::Unknown) => {
                                                error!(
                                                    "Node '{}' status is now UNKNOWN (was UP)",
                                                    node.name
                                                );
                                            }
                                            (NodeStatus::Offline, NodeStatus::Unknown) => {
                                                error!(
                                                    "Node '{}' status is now UNKNOWN (was DOWN)",
                                                    node.name
                                                );
                                            }
                                            _ => {}
                                        }
                                    }
                                }

                                // Update the previous status tracking
                                previous_statuses.insert(node_id, new_status);

                                node.status = check_result.status;
                                node.last_check = Some(check_result.timestamp);
                                node.response_time = check_result.response_time;
                                check_result.node_id = node_id;

                                if let Err(e) = db.update_node(node) {
                                    error!("Failed to update node status: {}", e);
                                }
                                if let Err(e) = db.add_monitoring_result(&check_result) {
                                    error!("Failed to save monitoring result: {}", e);
                                }

                                if update_tx.send(node.clone()).is_err() {
                                    info!(
                                        "Main thread receiver disconnected. Shutting down monitor."
                                    );
                                    break;
                                }
                            }
                            Err(e) => {
                                error!("Error checking node {}: {}", node.name, e);
                            }
                        }
                    }
                }

                // Check for stop signal
                match stop_rx.recv_timeout(Duration::from_secs(1)) {
                    Ok(_) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                        info!("Stopping monitoring thread");
                        break;
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
            }
        });

        self.monitoring_handle = Some(MonitoringHandle {
            stop_tx,
            config_tx,
            thread,
        });
        self.set_status_message("Monitoring started".to_string());
    }

    fn stop_monitoring(&mut self) {
        info!("Sending stop signal to monitoring thread");
        if let Some(handle) = self.monitoring_handle.take() {
            if let Err(e) = handle.stop_tx.send(()) {
                error!("Failed to send stop signal to monitoring thread: {}", e);
            }
        }
        self.set_status_message("Monitoring stopped".to_string());
    }

    fn import_nodes(&mut self) {
        if let Some(path) = FileDialog::new().add_filter("JSON", &["json"]).pick_file() {
            match std::fs::read_to_string(path) {
                Ok(data) => match serde_json::from_str::<Vec<NodeImport>>(&data) {
                    Ok(nodes_to_import) => {
                        let mut count = 0;
                        let mut added_nodes = Vec::new();
                        for import in nodes_to_import {
                            let mut node = Node {
                                id: None,
                                name: import.name,
                                detail: import.detail,
                                status: NodeStatus::Unknown,
                                last_check: None,
                                response_time: None,
                                monitoring_interval: import.monitoring_interval,
                                credential_id: import.credential_id,
                            };
                            if let Ok(id) = self.database.add_node(&node) {
                                node.id = Some(id);
                                added_nodes.push(node);
                                count += 1;
                            }
                        }

                        // Send all imported nodes to monitoring thread if it's running
                        if let Some(handle) = &self.monitoring_handle {
                            for node in added_nodes {
                                if let Err(e) = handle.config_tx.send(NodeConfigUpdate::Add(node)) {
                                    error!(
                                        "Failed to send imported node to monitoring thread: {}",
                                        e
                                    );
                                }
                            }
                        }

                        self.set_status_message(format!("Imported {} nodes", count));
                        self.reload_nodes();
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
    }

    fn export_nodes(&mut self) {
        if let Some(path) = FileDialog::new().add_filter("JSON", &["json"]).save_file() {
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
                    if let Err(e) = std::fs::write(path, data) {
                        self.set_status_message(format!("Failed to write export file: {}", e));
                    } else {
                        self.set_status_message("Nodes exported successfully".to_string());
                    }
                }
                Err(e) => {
                    self.set_status_message(format!("Failed to serialize nodes for export: {}", e));
                }
            }
        }
    }

    fn reload_nodes(&mut self) {
        match self.database.get_all_nodes() {
            Ok(nodes) => self.nodes = nodes,
            Err(e) => {
                self.set_status_message(format!("Failed to reload nodes: {}", e));
                error!("Failed to reload nodes from database: {}", e);
            }
        }
    }

    fn set_status_message(&mut self, message: String) {
        info!("Status: {}", message);
        self.status_message = Some((message, self.get_current_time()));
    }

    fn get_current_time(&self) -> f64 {
        chrono::Utc::now().timestamp() as f64
    }

    fn open_log_file(&mut self) {
        if let Some(proj_dirs) = ProjectDirs::from("com", "casey", "net-monitor") {
            let log_file = proj_dirs.data_dir().join("net-monitor.log");

            if log_file.exists() {
                #[cfg(target_os = "windows")]
                {
                    if let Err(e) = Command::new("notepad").arg(&log_file).spawn() {
                        self.set_status_message(format!("Failed to open log file: {}", e));
                    }
                }

                #[cfg(target_os = "macos")]
                {
                    if let Err(e) = Command::new("open").arg(&log_file).spawn() {
                        self.set_status_message(format!("Failed to open log file: {}", e));
                    }
                }

                #[cfg(target_os = "linux")]
                {
                    if let Err(e) = Command::new("xdg-open").arg(&log_file).spawn() {
                        self.set_status_message(format!("Failed to open log file: {}", e));
                    }
                }
            } else {
                self.set_status_message(format!("Log file not found at: {:?}", log_file));
            }
        } else {
            self.set_status_message("Could not determine log file location".to_string());
        }
    }

    fn show_credentials_window(&mut self, ctx: &egui::Context) {
        if self.show_credentials {
            let mut add_credential = false;
            let mut refresh_credentials = false;
            let mut show_credentials = self.show_credentials;

            egui::Window::new("Credential Manager")
                .resizable(true)
                .default_width(600.0)
                .default_height(400.0)
                .open(&mut show_credentials)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("Add New Credential").clicked() {
                            add_credential = true;
                        }
                        ui.separator();
                        if ui.button("Refresh").clicked() {
                            refresh_credentials = true;
                        }
                    });

                    ui.separator();

                    // Show existing credentials
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        egui::Grid::new("credentials_grid")
                            .num_columns(4)
                            .spacing([40.0, 4.0])
                            .striped(true)
                            .show(ui, |ui| {
                                ui.label(RichText::new("Name").strong());
                                ui.label(RichText::new("Description").strong());
                                ui.label(RichText::new("Type").strong());
                                ui.label(RichText::new("Actions").strong());
                                ui.end_row();

                                let mut delete_credential_id: Option<
                                    crate::credentials::CredentialId,
                                > = None;

                                for credential in &self.credentials {
                                    ui.label(&credential.name);
                                    ui.label(credential.description.as_deref().unwrap_or(""));
                                    ui.label(&credential.credential_type);

                                    ui.horizontal(|ui| {
                                        if ui.small_button("Edit").clicked() {
                                            // TODO: Implement edit functionality
                                        }
                                        if ui.small_button("Delete").clicked() {
                                            delete_credential_id = Some(credential.id.clone());
                                        }
                                    });
                                    ui.end_row();
                                }

                                // Handle deletion after the loop
                                if let Some(cred_id) = delete_credential_id {
                                    self.pending_credential_action =
                                        Some(CredentialAction::Delete(cred_id));
                                }
                            });
                    });
                });

            // Update state after the window
            self.show_credentials = show_credentials;
            if add_credential {
                self.show_add_credential = true;
            }
            if refresh_credentials {
                self.reload_credentials();
            }
        }
    }

    fn show_add_credential_window(&mut self, ctx: &egui::Context) {
        if self.show_add_credential {
            let mut save_clicked = false;
            let mut cancel_clicked = false;

            egui::Window::new("Add New Credential")
                .resizable(false)
                .default_width(400.0)
                .open(&mut self.show_add_credential)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Name:");
                        ui.text_edit_singleline(&mut self.new_credential_form.name);
                    });

                    ui.horizontal(|ui| {
                        ui.label("Description:");
                        ui.text_edit_singleline(&mut self.new_credential_form.description);
                    });

                    ui.horizontal(|ui| {
                        ui.label("Type:");
                        egui::ComboBox::from_label("")
                            .selected_text(format!(
                                "{:?}",
                                self.new_credential_form.credential_type
                            ))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.new_credential_form.credential_type,
                                    CredentialTypeForm::Default,
                                    "Default",
                                );
                                ui.selectable_value(
                                    &mut self.new_credential_form.credential_type,
                                    CredentialTypeForm::Password,
                                    "Password",
                                );
                                ui.selectable_value(
                                    &mut self.new_credential_form.credential_type,
                                    CredentialTypeForm::KeyFile,
                                    "SSH Key File",
                                );
                                ui.selectable_value(
                                    &mut self.new_credential_form.credential_type,
                                    CredentialTypeForm::KeyData,
                                    "SSH Key Data",
                                );
                            });
                    });

                    ui.separator();

                    match self.new_credential_form.credential_type {
                        CredentialTypeForm::Default => {
                            ui.label("Uses system default SSH configuration");
                        }
                        CredentialTypeForm::Password => {
                            ui.horizontal(|ui| {
                                ui.label("Username:");
                                ui.text_edit_singleline(&mut self.new_credential_form.username);
                            });
                            ui.horizontal(|ui| {
                                ui.label("Password:");
                                ui.add(
                                    egui::TextEdit::singleline(
                                        &mut self.new_credential_form.password,
                                    )
                                    .password(true),
                                );
                            });
                        }
                        CredentialTypeForm::KeyFile => {
                            ui.horizontal(|ui| {
                                ui.label("Username:");
                                ui.text_edit_singleline(&mut self.new_credential_form.username);
                            });
                            ui.horizontal(|ui| {
                                ui.label("SSH Key Path:");
                                ui.text_edit_singleline(&mut self.new_credential_form.ssh_key_path);
                                if ui.button("Browse").clicked() {
                                    if let Some(path) = FileDialog::new()
                                        .add_filter("SSH Keys", &["pem", "pub", "key"])
                                        .pick_file()
                                    {
                                        self.new_credential_form.ssh_key_path =
                                            path.to_string_lossy().to_string();
                                    }
                                }
                            });
                            ui.horizontal(|ui| {
                                ui.label("Passphrase (optional):");
                                ui.add(
                                    egui::TextEdit::singleline(
                                        &mut self.new_credential_form.passphrase,
                                    )
                                    .password(true),
                                );
                            });
                        }
                        CredentialTypeForm::KeyData => {
                            ui.horizontal(|ui| {
                                ui.label("Username:");
                                ui.text_edit_singleline(&mut self.new_credential_form.username);
                            });
                            ui.label("SSH Private Key Data:");
                            ui.add(
                                egui::TextEdit::multiline(
                                    &mut self.new_credential_form.ssh_key_data,
                                )
                                .desired_rows(6),
                            );
                            ui.horizontal(|ui| {
                                ui.label("Passphrase (optional):");
                                ui.add(
                                    egui::TextEdit::singleline(
                                        &mut self.new_credential_form.passphrase,
                                    )
                                    .password(true),
                                );
                            });
                        }
                    }

                    ui.separator();

                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked() {
                            save_clicked = true;
                        }
                        if ui.button("Cancel").clicked() {
                            cancel_clicked = true;
                        }
                    });
                });

            // Handle actions after the window
            if save_clicked {
                self.prepare_save_credential();
            }
            if cancel_clicked {
                self.show_add_credential = false;
                self.reset_credential_form();
            }
        }
    }

    fn reload_credentials(&mut self) {
        println!("Reloading credentials from store");
        info!("Reloading credentials from store");
        match self.credential_store.list_credentials() {
            Ok(credentials) => {
                println!("Loaded {} credentials", credentials.len());
                for cred in &credentials {
                    println!(
                        "  - ID: {:?}, Name: {}, Type: {}",
                        cred.id, cred.name, cred.credential_type
                    );
                }
                info!("Loaded {} credentials", credentials.len());
                self.credentials = credentials;
            }
            Err(e) => {
                println!("Failed to reload credentials: {}", e);
                self.set_status_message(format!("Failed to reload credentials: {}", e));
                error!("Failed to reload credentials: {}", e);
            }
        }
    }

    fn prepare_save_credential(&mut self) {
        println!(
            "prepare_save_credential called with name: '{}'",
            self.new_credential_form.name
        );

        if self.new_credential_form.name.trim().is_empty() {
            println!("Credential name is empty, aborting");
            self.set_status_message("Credential name cannot be empty".to_string());
            return;
        }

        let credential = match self.new_credential_form.credential_type {
            CredentialTypeForm::Default => SshCredential::Default,
            CredentialTypeForm::Password => {
                if self.new_credential_form.username.trim().is_empty()
                    || self.new_credential_form.password.trim().is_empty()
                {
                    self.set_status_message("Username and password are required".to_string());
                    return;
                }
                SshCredential::Password {
                    username: self.new_credential_form.username.clone(),
                    password: SensitiveString::new(self.new_credential_form.password.clone()),
                }
            }
            CredentialTypeForm::KeyFile => {
                if self.new_credential_form.username.trim().is_empty()
                    || self.new_credential_form.ssh_key_path.trim().is_empty()
                {
                    self.set_status_message("Username and SSH key path are required".to_string());
                    return;
                }
                SshCredential::Key {
                    username: self.new_credential_form.username.clone(),
                    private_key_path: PathBuf::from(&self.new_credential_form.ssh_key_path),
                    passphrase: if self.new_credential_form.passphrase.trim().is_empty() {
                        None
                    } else {
                        Some(SensitiveString::new(
                            self.new_credential_form.passphrase.clone(),
                        ))
                    },
                }
            }
            CredentialTypeForm::KeyData => {
                if self.new_credential_form.username.trim().is_empty()
                    || self.new_credential_form.ssh_key_data.trim().is_empty()
                {
                    self.set_status_message("Username and SSH key data are required".to_string());
                    return;
                }
                SshCredential::KeyData {
                    username: self.new_credential_form.username.clone(),
                    private_key_data: SensitiveString::new(
                        self.new_credential_form.ssh_key_data.clone(),
                    ),
                    passphrase: if self.new_credential_form.passphrase.trim().is_empty() {
                        None
                    } else {
                        Some(SensitiveString::new(
                            self.new_credential_form.passphrase.clone(),
                        ))
                    },
                }
            }
        };

        let description = if self.new_credential_form.description.trim().is_empty() {
            None
        } else {
            Some(self.new_credential_form.description.clone())
        };

        println!(
            "Creating pending save action for credential: {}",
            self.new_credential_form.name
        );

        self.pending_credential_action = Some(CredentialAction::Save {
            name: self.new_credential_form.name.clone(),
            description,
            credential,
        });

        println!("Closing credential form and resetting");
        self.show_add_credential = false;
        self.reset_credential_form();
    }

    fn reset_credential_form(&mut self) {
        self.new_credential_form = CredentialForm {
            name: String::new(),
            description: String::new(),
            credential_type: CredentialTypeForm::Default,
            username: String::new(),
            password: String::new(),
            ssh_key_path: String::new(),
            ssh_key_data: String::new(),
            passphrase: String::new(),
        };
    }

    fn process_pending_credential_action(&mut self) {
        if let Some(action) = self.pending_credential_action.take() {
            println!(
                "Processing pending credential action: {:?}",
                match &action {
                    CredentialAction::Save { name, .. } => format!("Save({})", name),
                    CredentialAction::Delete(id) => format!("Delete({:?})", id),
                }
            );

            match action {
                CredentialAction::Save {
                    name,
                    description,
                    credential,
                } => {
                    println!("Attempting to save credential: {}", name);
                    match self
                        .credential_store
                        .store_credential(name, description, credential)
                    {
                        Ok(id) => {
                            println!("Credential saved successfully with ID: {:?}", id);
                            self.set_status_message("Credential saved successfully".to_string());
                            self.reload_credentials();
                        }
                        Err(e) => {
                            println!("Failed to save credential: {}", e);
                            self.set_status_message(format!("Failed to save credential: {}", e));
                        }
                    }
                }
                CredentialAction::Delete(id) => {
                    println!("Attempting to delete credential: {:?}", id);
                    match self.credential_store.delete_credential(&id) {
                        Ok(_) => {
                            println!("Credential deleted successfully");
                            self.set_status_message("Credential deleted successfully".to_string());
                            self.reload_credentials();
                        }
                        Err(e) => {
                            println!("Failed to delete credential: {}", e);
                            self.set_status_message(format!("Failed to delete credential: {}", e));
                        }
                    }
                }
            }
        }
    }

    fn show_about_window(&mut self, ctx: &egui::Context) {
        if self.show_about {
            let mut close_dialog = false;

            egui::Window::new("About Network Monitor")
                .resizable(false)
                .collapsible(false)
                .default_width(400.0)
                .default_height(300.0)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .open(&mut self.show_about)
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("Network Monitor");
                        ui.add_space(10.0);

                        ui.label(RichText::new("Version 0.2.1").strong());
                        ui.add_space(5.0);

                        ui.label("A simple network monitoring application");
                        ui.label("written in Rust with a GUI interface.");
                        ui.add_space(10.0);

                        ui.label(RichText::new("Repository:").strong());
                        if ui
                            .link("https://github.com/casey-mccarthy/net-monitor")
                            .clicked()
                        {
                            let _ = open::that("https://github.com/casey-mccarthy/net-monitor");
                        }
                        ui.add_space(10.0);

                        ui.label(RichText::new("Author:").strong());
                        ui.label("Casey McCarthy");
                        ui.add_space(15.0);

                        if ui.button("Close").clicked() {
                            close_dialog = true;
                        }
                    });
                });

            if close_dialog {
                self.show_about = false;
            }
        }
    }
}

enum NodeAction {
    Edit(usize),
    Delete(usize),
    Connect(usize),
}

enum CredentialAction {
    Save {
        name: String,
        description: Option<String>,
        credential: SshCredential,
    },
    Delete(crate::credentials::CredentialId),
}

#[derive(Clone)]
enum NodeConfigUpdate {
    Add(Node),
    Update(Node),
    Delete(i64),
}
