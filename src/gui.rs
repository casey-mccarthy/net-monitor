use crate::database::Database;
use crate::models::{MonitorDetail, Node, NodeImport, NodeStatus};
use crate::monitor::check_node;
use anyhow::Result;
use chrono;
use eframe::egui::{self, Color32, Context, Grid, RichText, ScrollArea, Ui, Window};
use rfd::FileDialog;
use std::collections::HashMap;
use std::fmt;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use tracing::{error, info};

#[derive(Clone, Copy, PartialEq)]
enum MonitorTypeForm {
    Http,
    Ping,
    Snmp,
}

impl fmt::Display for MonitorTypeForm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MonitorTypeForm::Http => write!(f, "HTTP"),
            MonitorTypeForm::Ping => write!(f, "Ping"),
            MonitorTypeForm::Snmp => write!(f, "SNMP"),
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
    // SNMP
    snmp_target: String,
    snmp_community: String,
    snmp_oid: String,
}

impl Default for NodeForm {
    fn default() -> Self {
        Self {
            name: String::new(),
            monitor_type: MonitorTypeForm::Http,
            monitoring_interval: "60".to_string(),
            http_url: "https://".to_string(),
            http_expected_status: "200".to_string(),
            ping_host: String::new(),
            ping_count: "4".to_string(),
            ping_timeout: "5".to_string(),
            snmp_target: String::new(),
            snmp_community: "public".to_string(),
            snmp_oid: "1.3.6.1.2.1.1.1.0".to_string(),
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
            MonitorTypeForm::Snmp => Ok(MonitorDetail::Snmp {
                target: self.snmp_target.clone(),
                community: self.snmp_community.clone(),
                oid: self.snmp_oid.clone(),
            }),
        }
    }

    fn from_node(node: &Node) -> Self {
        let mut form = Self::default();
        form.name = node.name.clone();
        form.monitoring_interval = node.monitoring_interval.to_string();

        match &node.detail {
            MonitorDetail::Http { url, expected_status } => {
                form.monitor_type = MonitorTypeForm::Http;
                form.http_url = url.clone();
                form.http_expected_status = expected_status.to_string();
            }
            MonitorDetail::Ping { host, count, timeout } => {
                form.monitor_type = MonitorTypeForm::Ping;
                form.ping_host = host.clone();
                form.ping_count = count.to_string();
                form.ping_timeout = timeout.to_string();
            }
            MonitorDetail::Snmp { target, community, oid } => {
                form.monitor_type = MonitorTypeForm::Snmp;
                form.snmp_target = target.clone();
                form.snmp_community = community.clone();
                form.snmp_oid = oid.clone();
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
}

struct MonitoringHandle {
    stop_tx: mpsc::Sender<()>,
    #[allow(dead_code)]
    thread: thread::JoinHandle<()>,
}

impl NetworkMonitorApp {
    pub fn new(database: Database) -> Result<Self> {
        let nodes = database.get_all_nodes()?;
        let (update_tx, update_rx) = mpsc::channel();
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
                *node = updated_node;
            }
        }

        self.show_main_window(ctx);
        self.show_add_node_window(ctx);
        self.show_edit_node_window(ctx);
        ctx.request_repaint_after(Duration::from_millis(500));
    }
}

impl NetworkMonitorApp {
    fn show_main_window(&mut self, ctx: &Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Network Monitor");
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Add Node").clicked() {
                    self.show_add_node = true;
                    self.new_node_form = NodeForm::default();
                }
                if ui.button("Import Nodes").clicked() {
                    self.import_nodes();
                }
                if ui.button("Export Nodes").clicked() {
                    self.export_nodes();
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
                .num_columns(6)
                .spacing([20.0, 10.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label(RichText::new("Name").strong());
                    ui.label(RichText::new("Target").strong());
                    ui.label(RichText::new("Type").strong());
                    ui.label(RichText::new("Status").strong());
                    ui.label(RichText::new("Last Check").strong());
                    ui.label(RichText::new("Actions").strong());
                    ui.end_row();

                    for (i, node) in self.nodes.iter().enumerate() {
                        ui.label(&node.name);
                        let target = match &node.detail {
                            MonitorDetail::Http { url, .. } => url.as_str(),
                            MonitorDetail::Ping { host, .. } => host.as_str(),
                            MonitorDetail::Snmp { target, .. } => target.as_str(),
                        };
                        ui.label(target);
                        ui.label(node.detail.to_string());
                        let status_color = match node.status {
                            NodeStatus::Online => Color32::GREEN,
                            NodeStatus::Offline => Color32::RED,
                            NodeStatus::Unknown => Color32::YELLOW,
                        };
                        ui.colored_label(status_color, node.status.to_string());
                        let last_check_str = node
                            .last_check
                            .map(|t| t.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M:%S").to_string())
                            .unwrap_or_else(|| "Never".to_string());
                        ui.label(last_check_str);
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
            Window::new(format!("Edit Node: {}", form.name))
                .open(&mut is_open)
                .resizable(true)
                .show(ctx, |ui| {
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
                    ui.selectable_value(&mut form.monitor_type, MonitorTypeForm::Snmp, "SNMP");
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
                MonitorTypeForm::Snmp => {
                    ui.label("Target:");
                    ui.text_edit_singleline(&mut form.snmp_target);
                    ui.end_row();
                    ui.label("Community:");
                    ui.text_edit_singleline(&mut form.snmp_community);
                    ui.end_row();
                    ui.label("OID:");
                    ui.text_edit_singleline(&mut form.snmp_oid);
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
                };
                match self.database.add_node(&node) {
                    Ok(id) => {
                        let mut new_node = node;
                        new_node.id = Some(id);
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
                    
                    if let Err(e) = self.database.update_node(node) {
                        error!("Failed to update node in database: {}", e);
                        self.set_status_message(format!("Error updating node: {}", e));
                    } else {
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
                            self.nodes.remove(index);
                            self.set_status_message("Node deleted".to_string());
                        } else {
                            error!("Failed to delete node with id {}", id);
                        }
                    }
                }
            }
        }
    }
    
    fn monitoring_toggle_button(&mut self, ui: &mut Ui) {
        if self.monitoring_handle.is_some() {
            if ui.button(RichText::new("Stop Monitoring").color(Color32::RED)).clicked() {
                self.stop_monitoring();
            }
        } else {
            if ui.button("Start Monitoring").clicked() {
                self.start_monitoring();
            }
        }
    }

    fn start_monitoring(&mut self) {
        info!("Starting monitoring thread");
        let (stop_tx, stop_rx) = mpsc::channel();
        let db = self.database.clone();
        let update_tx = self.update_tx.clone();
        
        let initial_nodes = self.nodes.clone();

        let thread = thread::spawn(move || {
            let mut last_check_times: HashMap<i64, Instant> = HashMap::new();

            loop {
                // In a real app, you might want to get a fresh list of nodes periodically from the DB
                // For now, we work with the initial set to keep it simple.
                let mut nodes_to_check = initial_nodes.clone();

                for node in &mut nodes_to_check {
                    let node_id = node.id.unwrap_or(0);
                    if node_id == 0 { continue; }

                    let now = Instant::now();

                    let should_check = last_check_times.get(&node_id).map_or(true, |last_check| {
                        now.duration_since(*last_check).as_secs() >= node.monitoring_interval
                    });

                    if should_check {
                        last_check_times.insert(node_id, now);
                        let result = check_node(&node);

                        match result {
                            Ok(mut check_result) => {
                                node.status = check_result.status;
                                node.last_check = Some(check_result.timestamp);
                                node.response_time = check_result.response_time;
                                check_result.node_id = node_id;

                                if let Err(e) = db.update_node(&node) {
                                    error!("Failed to update node status: {}", e);
                                }
                                if let Err(e) = db.add_monitoring_result(&check_result) {
                                    error!("Failed to save monitoring result: {}", e);
                                }
                                
                                if update_tx.send(node.clone()).is_err() {
                                    info!("Main thread receiver disconnected. Shutting down monitor.");
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

        self.monitoring_handle = Some(MonitoringHandle { stop_tx, thread });
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
                        for import in nodes_to_import {
                            let node = Node {
                                id: None,
                                name: import.name,
                                detail: import.detail,
                                status: NodeStatus::Unknown,
                                last_check: None,
                                response_time: None,
                                monitoring_interval: import.monitoring_interval,
                            };
                            if self.database.add_node(&node).is_ok() {
                                count += 1;
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
            let nodes_to_export: Vec<NodeImport> = self.nodes.iter().map(|node| NodeImport {
                name: node.name.clone(),
                detail: node.detail.clone(),
                monitoring_interval: node.monitoring_interval,
            }).collect();

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
}

enum NodeAction {
    Edit(usize),
    Delete(usize),
} 