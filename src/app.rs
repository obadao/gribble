use crossterm::event::KeyCode;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, ListState, Paragraph},
    Frame,
};
use sysinfo::{System, Disks, Networks};
use std::time::Instant;
use std::fs;
use std::path::PathBuf;
use std::os::unix::fs::PermissionsExt;
use tracing::{error, warn, info};

use crate::{
    network::NetworkHistory,
    utils::{
        truncate_string, MAX_PROCESSES, MAX_NETWORKS, MAX_FILES, PAGE_SIZE, 
        UPDATE_INTERVAL, MANUAL_REFRESH_COOLDOWN, FILE_NAME_MAX_LEN,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    SystemMonitor = 0,
    SystemStatus = 1,
    ProcessManager = 2,
    FileExplorer = 3,
    NetworkGraph = 4,
}

impl Panel {
    pub const COUNT: usize = 5;
    
    pub fn from_index(index: usize) -> Option<Panel> {
        match index {
            0 => Some(Panel::SystemMonitor),
            1 => Some(Panel::SystemStatus),
            2 => Some(Panel::ProcessManager),
            3 => Some(Panel::FileExplorer),
            4 => Some(Panel::NetworkGraph),
            _ => None,
        }
    }
    
    pub fn as_index(self) -> usize {
        self as usize
    }
}

// Cached data structures
#[derive(Clone)]
pub struct CachedProcess {
    pub name: String,
    pub pid: u32,
    pub cpu_usage: f32,
    pub memory: u64,
}

#[derive(Clone)]
pub struct CachedNetwork {
    pub name: String,
    pub total_received: u64,
    pub total_transmitted: u64,
}

#[derive(Debug, Clone)]
pub enum ModalType {
    ProcessDetails,
    NetworkDetails,
    SystemDetails,
    DiskDetails,
}

#[derive(Clone)]
pub enum ModalData {
    ProcessDetails {
        name: String,
        pid: u32,
        cpu_usage: f32,
        memory_usage: u64,
        status: String,
        cmd: String,
    },
    NetworkDetails {
        name: String,
        total_received: u64,
        total_transmitted: u64,
        received_rate: u64,
        transmitted_rate: u64,
    },
    SystemDetails {
        hostname: String,
        os_name: String,
        os_version: String,
        kernel_version: String,
        cpu_count: usize,
        total_memory: u64,
        uptime: u64,
    },
    DiskDetails {
        name: String,
        mount_point: String,
        total_space: u64,
        available_space: u64,
        file_system: String,
    },
}

pub struct App {
    pub should_quit: bool,
    pub system: System,
    pub disks: Disks,
    pub networks: Networks,
    pub last_update: Instant,
    pub last_manual_refresh: Instant,
    pub selected_panel: Panel,
    pub current_dir: PathBuf,
    pub dir_entries: Vec<String>,
    pub dir_entry_paths: Vec<PathBuf>, // Store original paths for navigation
    pub selected_process: usize,
    pub selected_file: usize,
    pub selected_network: usize,
    pub show_help: bool,
    pub process_list_state: ListState,
    pub file_list_state: ListState,
    pub network_history: NetworkHistory,
    // Cached data
    pub cached_processes: Vec<CachedProcess>,
    pub cached_networks: Vec<CachedNetwork>,
    pub last_data_refresh: Instant,
    // Error recovery
    pub directory_history: Vec<PathBuf>, // Track directory history for recovery
    pub last_successful_dir: PathBuf, // Last directory that loaded successfully
    // Modal system
    pub show_modal: bool,
    pub modal_type: ModalType,
    pub modal_data: ModalData,
}

impl App {
    pub fn new() -> Self {
        let mut system = System::new_all();
        system.refresh_all();
        let disks = Disks::new_with_refreshed_list();
        let networks = Networks::new_with_refreshed_list();
        
        let current_dir = std::env::current_dir().unwrap_or_else(|e| {
            warn!("Failed to get current directory: {}, using '.'", e);
            PathBuf::from(".")
        });
        let (dir_entries, dir_entry_paths) = Self::read_directory(&current_dir);

        let mut process_list_state = ListState::default();
        process_list_state.select(Some(0));
        let mut file_list_state = ListState::default();
        file_list_state.select(Some(0));

        let network_history = NetworkHistory::new();

        let mut app = Self {
            should_quit: false,
            system,
            disks,
            networks,
            last_update: Instant::now(),
            last_manual_refresh: Instant::now(),
            selected_panel: Panel::SystemMonitor,
            current_dir: current_dir.clone(),
            dir_entries,
            dir_entry_paths,
            selected_process: 0,
            selected_file: 0,
            selected_network: 0,
            show_help: false,
            process_list_state,
            file_list_state,
            network_history,
            cached_processes: Vec::new(),
            cached_networks: Vec::new(),
            last_data_refresh: Instant::now(),
            directory_history: vec![current_dir.clone()],
            last_successful_dir: current_dir,
            show_modal: false,
            modal_type: ModalType::SystemDetails,
            modal_data: ModalData::SystemDetails {
                hostname: "Unknown".to_string(),
                os_name: "Unknown".to_string(),
                os_version: "Unknown".to_string(),
                kernel_version: "Unknown".to_string(),
                cpu_count: 0,
                total_memory: 0,
                uptime: 0,
            },
        };
        
        // Initial data cache
        app.refresh_cached_data();
        app
    }

    fn read_directory(path: &PathBuf) -> (Vec<String>, Vec<PathBuf>) {
        match fs::read_dir(path) {
            Ok(entries) => {
                // Pre-allocate vectors with capacity to avoid reallocations
                let mut items = Vec::with_capacity(MAX_FILES + 1); // +1 for ".."
                let mut paths = Vec::with_capacity(MAX_FILES + 1);
                
                // Add parent directory entry
                items.push("..".to_string());
                paths.push(path.parent().unwrap_or(path).to_path_buf());
                
                // Pre-allocate separate vectors for directories and files
                let mut dirs = Vec::with_capacity(MAX_FILES / 2);
                let mut files = Vec::with_capacity(MAX_FILES / 2);

                // Collect entries efficiently, limiting to MAX_FILES
                let mut entry_count = 0;
                for entry in entries.flatten() {
                    if entry_count >= MAX_FILES {
                        break; // Stop reading once we have enough entries
                    }
                    
                    let entry_path = entry.path();
                    let name = entry.file_name().to_string_lossy().to_string();
                    let truncated_name = truncate_string(&name, FILE_NAME_MAX_LEN);
                    
                    if entry_path.is_dir() {
                        dirs.push((format!("📁 {}", truncated_name), entry_path));
                    } else {
                        files.push((format!("📄 {}", truncated_name), entry_path));
                    }
                    entry_count += 1;
                }
                
                // Sort directories and files separately for better performance
                dirs.sort_by(|a, b| a.0.cmp(&b.0));
                files.sort_by(|a, b| a.0.cmp(&b.0));
                
                // Add sorted directories first, then files
                for (item, path) in dirs {
                    items.push(item);
                    paths.push(path);
                }
                for (item, path) in files {
                    items.push(item);
                    paths.push(path);
                }
                
                (items, paths)
            }
            Err(e) => {
                error!("Failed to read directory {:?}: {}", path, e);
                (vec![format!("<Error: {}>", e), "<Press 'r' to retry>".to_string()], vec![path.clone(), path.clone()])
            },
        }
    }

    fn try_navigate_to_directory(&mut self, target_path: &PathBuf) -> bool {
        let (dir_entries, dir_entry_paths) = Self::read_directory(target_path);
        
        // Check if we successfully read the directory (not an error)
        if dir_entries.len() > 0 && !dir_entries[0].starts_with("<Error:") {
            self.current_dir = target_path.clone();
            self.dir_entries = dir_entries;
            self.dir_entry_paths = dir_entry_paths;
            self.selected_file = 0;
            self.file_list_state.select(Some(0));
            
            // Update directory history and last successful directory
            if !self.directory_history.contains(target_path) {
                self.directory_history.push(target_path.clone());
                // Limit history to prevent memory growth
                if self.directory_history.len() > 20 {
                    self.directory_history.remove(0);
                }
            }
            self.last_successful_dir = target_path.clone();
            return true;
        }
        
        warn!("Failed to navigate to directory: {:?}", target_path);
        false
    }

    fn navigate_back_to_safe_directory(&mut self) {
        // Try to go back to the last successful directory
        if self.last_successful_dir != self.current_dir {
            let safe_dir = self.last_successful_dir.clone();
            if self.try_navigate_to_directory(&safe_dir) {
                info!("Recovered to safe directory: {:?}", safe_dir);
                return;
            }
        }
        
        // If that fails, try the parent directory
        if let Some(parent) = self.current_dir.parent() {
            let parent_path = parent.to_path_buf();
            if self.try_navigate_to_directory(&parent_path) {
                info!("Recovered to parent directory: {:?}", parent_path);
                return;
            }
        }
        
        // Last resort: go to home directory
        if let Ok(home) = std::env::var("HOME") {
            let home_path = PathBuf::from(home);
            if self.try_navigate_to_directory(&home_path) {
                info!("Recovered to home directory: {:?}", home_path);
                return;
            }
        }
        
        // Ultimate fallback: current directory
        if let Ok(current) = std::env::current_dir() {
            if self.try_navigate_to_directory(&current) {
                info!("Recovered to current directory: {:?}", current);
                return;
            }
        }
        
        error!("Unable to recover from directory navigation error");
    }

    pub fn update(&mut self) {
        // Update system info every 2 seconds
        if self.last_update.elapsed() >= UPDATE_INTERVAL {
            self.system.refresh_all();
            self.disks.refresh(true);
            self.networks.refresh(true);
            
            // Refresh cached data
            self.refresh_cached_data();
            
            // Get the selected network interface name
            let selected_interface_name = if let Some(network) = self.cached_networks.get(self.selected_network) {
                network.name.clone()
            } else {
                String::new()
            };
            
            self.network_history.update(&self.networks, &selected_interface_name);
            self.last_update = Instant::now();
        }
    }

    fn refresh_cached_data(&mut self) {
        // Cache processes
        self.cached_processes.clear();
        for (_, process) in self.system.processes().iter().take(MAX_PROCESSES) {
            self.cached_processes.push(CachedProcess {
                name: process.name().to_string_lossy().to_string(),
                pid: process.pid().as_u32(),
                cpu_usage: process.cpu_usage(),
                memory: process.memory(),
            });
        }
        
        // Sort processes by CPU usage
        self.cached_processes.sort_by(|a, b| b.cpu_usage.partial_cmp(&a.cpu_usage).unwrap_or(std::cmp::Ordering::Equal));
        
        // Cache networks
        self.cached_networks.clear();
        for (name, network) in self.networks.list().iter().take(MAX_NETWORKS) {
            self.cached_networks.push(CachedNetwork {
                name: name.to_string(),
                total_received: network.total_received(),
                total_transmitted: network.total_transmitted(),
            });
        }
        
        self.last_data_refresh = Instant::now();
    }

    pub fn handle_key_event(&mut self, key: KeyCode) {
        if self.show_help {
            match key {
                KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('h') => {
                    self.show_help = false;
                }
                _ => {}
            }
            return;
        }

        match key {
            KeyCode::Char('q') | KeyCode::Esc => {
                if self.show_modal {
                    self.hide_modal();
                } else if self.show_help {
                    self.show_help = false;
                } else {
                    self.should_quit = true;
                }
            }
            KeyCode::Left | KeyCode::Char('h') => {
                self.select_previous_panel();
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.select_next_panel();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                match self.selected_panel {
                    Panel::ProcessManager => { // Process manager
                        if self.selected_process > 0 {
                            self.selected_process -= 1;
                            self.process_list_state.select(Some(self.selected_process));
                        }
                    }
                    Panel::FileExplorer => { // File browser
                        if self.selected_file > 0 {
                            self.selected_file -= 1;
                            self.file_list_state.select(Some(self.selected_file));
                        }
                    }
                    Panel::NetworkGraph => { // Network panel - cycle to previous interface
                        let network_count = self.cached_networks.len();
                        if network_count > 0 {
                            self.selected_network = if self.selected_network == 0 {
                                network_count.saturating_sub(1)
                            } else {
                                self.selected_network - 1
                            };
                            // Reset network history when switching interfaces
                            self.network_history.clear();
                        }
                    }
                    _ => {}
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                match self.selected_panel {
                    Panel::ProcessManager => { // Process manager
                        let max_processes = self.cached_processes.len();
                        if self.selected_process < max_processes.saturating_sub(1) {
                            self.selected_process += 1;
                            self.process_list_state.select(Some(self.selected_process));
                        }
                    }
                    Panel::FileExplorer => { // File browser
                        if self.selected_file < self.dir_entries.len() - 1 {
                            self.selected_file += 1;
                            self.file_list_state.select(Some(self.selected_file));
                        }
                    }
                    Panel::NetworkGraph => { // Network panel - cycle to next interface
                        let network_count = self.cached_networks.len();
                        if network_count > 0 {
                            self.selected_network = (self.selected_network + 1) % network_count;
                            // Reset network history when switching interfaces
                            self.network_history.clear();
                        }
                    }
                    _ => {}
                }
            }
            KeyCode::PageUp => {
                match self.selected_panel {
                    Panel::ProcessManager => { // Process manager
                        let page_size = PAGE_SIZE; // Approximate visible items per page
                        self.selected_process = self.selected_process.saturating_sub(page_size);
                        self.process_list_state.select(Some(self.selected_process));
                    }
                    Panel::FileExplorer => { // File browser
                        let page_size = PAGE_SIZE;
                        self.selected_file = self.selected_file.saturating_sub(page_size);
                        self.file_list_state.select(Some(self.selected_file));
                    }
                    _ => {}
                }
            }
            KeyCode::PageDown => {
                match self.selected_panel {
                    Panel::ProcessManager => { // Process manager
                        let page_size = PAGE_SIZE;
                        let max_processes = self.cached_processes.len();
                        self.selected_process = (self.selected_process + page_size).min(max_processes.saturating_sub(1));
                        self.process_list_state.select(Some(self.selected_process));
                    }
                    Panel::FileExplorer => { // File browser
                        let page_size = PAGE_SIZE;
                        let max_files = self.dir_entries.len();
                        self.selected_file = (self.selected_file + page_size).min(max_files.saturating_sub(1));
                        self.file_list_state.select(Some(self.selected_file));
                    }
                    _ => {}
                }
            }
            KeyCode::Home => {
                match self.selected_panel {
                    Panel::ProcessManager => { // Process manager
                        self.selected_process = 0;
                        self.process_list_state.select(Some(0));
                    }
                    Panel::FileExplorer => { // File browser
                        self.selected_file = 0;
                        self.file_list_state.select(Some(0));
                    }
                    _ => {}
                }
            }
            KeyCode::End => {
                match self.selected_panel {
                    Panel::ProcessManager => { // Process manager
                        let max_processes = self.cached_processes.len();
                        if max_processes > 0 {
                            self.selected_process = max_processes.saturating_sub(1);
                            self.process_list_state.select(Some(self.selected_process));
                        }
                    }
                    Panel::FileExplorer => { // File browser
                        let max_files = self.dir_entries.len();
                        if max_files > 0 {
                            self.selected_file = max_files - 1;
                            self.file_list_state.select(Some(self.selected_file));
                        }
                    }
                    _ => {}
                }
            }
            KeyCode::Enter => {
                if self.selected_panel == Panel::FileExplorer {
                    self.navigate_into_selected();
                }
            }
            KeyCode::Char('r') => {
                // Force refresh with rate limiting
                if self.last_manual_refresh.elapsed() >= MANUAL_REFRESH_COOLDOWN {
                    self.system.refresh_all();
                    if self.selected_panel == Panel::FileExplorer {
                        // For file explorer, try to refresh current directory or recover if it fails
                        let current_dir = self.current_dir.clone();
                        if !self.try_navigate_to_directory(&current_dir) {
                            self.navigate_back_to_safe_directory();
                        }
                    }
                    self.last_manual_refresh = Instant::now();
                }
            }
            KeyCode::Char('?') => {
                self.show_help = true;
            }
            KeyCode::Char('i') => {
                if self.show_modal {
                    // Close modal if already open
                    self.hide_modal();
                } else {
                    // Show info modal based on current panel
                    match self.selected_panel {
                        Panel::ProcessManager => self.show_process_modal(),
                        Panel::NetworkGraph => self.show_network_modal(),
                        Panel::SystemMonitor | Panel::SystemStatus => self.show_system_modal(),
                        Panel::FileExplorer => self.show_file_modal(),
                    }
                }
            }
            KeyCode::Char('b') => {
                // Navigate back in directory history
                if self.selected_panel == Panel::FileExplorer && self.directory_history.len() > 1 {
                    // Go back to the previous directory in history
                    let current_index = self.directory_history.iter()
                        .position(|path| path == &self.current_dir)
                        .unwrap_or(0);
                    
                    if current_index > 0 {
                        let prev_path = self.directory_history[current_index - 1].clone();
                        if !self.try_navigate_to_directory(&prev_path) {
                            self.navigate_back_to_safe_directory();
                        }
                    }
                }
            }
            KeyCode::Backspace => {
                // Go up one directory (same as selecting "..")
                if self.selected_panel == Panel::FileExplorer { // File browser panel
                    if let Some(parent) = self.current_dir.parent() {
                        if !self.try_navigate_to_directory(&parent.to_path_buf()) {
                            self.navigate_back_to_safe_directory();
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn navigate_into_selected(&mut self) {
        if self.selected_file >= self.dir_entries.len() || self.selected_file >= self.dir_entry_paths.len() {
            return;
        }
        
        let selected_item = &self.dir_entries[self.selected_file];
        let selected_path = &self.dir_entry_paths[self.selected_file];
        
        // Check if we're trying to navigate to an error state
        if selected_item.starts_with("<Error:") {
            warn!("Attempting to navigate to error state, attempting recovery");
            self.navigate_back_to_safe_directory();
            return;
        }
        
        if selected_item == ".." {
            // Go up one directory using the stored parent path
            let target_path = selected_path.clone();
            if !self.try_navigate_to_directory(&target_path) {
                self.navigate_back_to_safe_directory();
            }
        } else if selected_item.starts_with("📁") {
            // Enter directory using the stored original path
            if selected_path.is_dir() {
                let target_path = selected_path.clone();
                if !self.try_navigate_to_directory(&target_path) {
                    self.navigate_back_to_safe_directory();
                }
            }
        }
        // Files are not opened - this could be a future feature
    }

    fn select_next_panel(&mut self) {
        let current_index = self.selected_panel.as_index();
        let next_index = (current_index + 1) % Panel::COUNT;
        self.selected_panel = Panel::from_index(next_index).unwrap_or(Panel::SystemMonitor);
    }

    fn select_previous_panel(&mut self) {
        let current_index = self.selected_panel.as_index();
        let prev_index = if current_index == 0 {
            Panel::COUNT - 1
        } else {
            current_index - 1
        };
        self.selected_panel = Panel::from_index(prev_index).unwrap_or(Panel::SystemMonitor);
    }

    fn show_process_modal(&mut self) {
        if self.selected_panel == Panel::ProcessManager && !self.cached_processes.is_empty() {
            let selected_process = &self.cached_processes[self.selected_process];
            if let Some(process) = self.system.process(sysinfo::Pid::from_u32(selected_process.pid)) {
                self.modal_data = ModalData::ProcessDetails {
                    name: selected_process.name.clone(),
                    pid: selected_process.pid,
                    cpu_usage: selected_process.cpu_usage,
                    memory_usage: selected_process.memory,
                    status: process.status().to_string(),
                    cmd: process.cmd().join(std::ffi::OsStr::new(" ")).to_string_lossy().to_string(),
                };
                self.modal_type = ModalType::ProcessDetails;
                self.show_modal = true;
            }
        }
    }

    fn show_network_modal(&mut self) {
        if self.selected_panel == Panel::NetworkGraph && !self.cached_networks.is_empty() {
            let selected_network = &self.cached_networks[self.selected_network];
            let _network_data = self.networks.get(&selected_network.name).unwrap();
            
            // Calculate current rates from network history
            let (rx_rate, tx_rate) = if !self.network_history.rx_rates.is_empty() {
                (
                    *self.network_history.rx_rates.back().unwrap_or(&0),
                    *self.network_history.tx_rates.back().unwrap_or(&0),
                )
            } else {
                (0, 0)
            };

            self.modal_data = ModalData::NetworkDetails {
                name: selected_network.name.clone(),
                total_received: selected_network.total_received,
                total_transmitted: selected_network.total_transmitted,
                received_rate: rx_rate,
                transmitted_rate: tx_rate,
            };
            self.modal_type = ModalType::NetworkDetails;
            self.show_modal = true;
        }
    }

    fn show_system_modal(&mut self) {
        self.modal_data = ModalData::SystemDetails {
            hostname: sysinfo::System::host_name().unwrap_or_else(|| "Unknown".to_string()),
            os_name: sysinfo::System::name().unwrap_or_else(|| "Unknown".to_string()),
            os_version: sysinfo::System::os_version().unwrap_or_else(|| "Unknown".to_string()),
            kernel_version: sysinfo::System::kernel_version().unwrap_or_else(|| "Unknown".to_string()),
            cpu_count: self.system.cpus().len(),
            total_memory: self.system.total_memory(),
            uptime: sysinfo::System::uptime(),
        };
        self.modal_type = ModalType::SystemDetails;
        self.show_modal = true;
    }

    fn show_file_modal(&mut self) {
        if self.selected_file >= self.dir_entries.len() || self.selected_file >= self.dir_entry_paths.len() {
            return;
        }
        
        let selected_item = &self.dir_entries[self.selected_file];
        let selected_path = &self.dir_entry_paths[self.selected_file];
        
        if selected_item == ".." || selected_item.starts_with("<Error:") {
            return; // Don't show info for parent directory or error items
        }
        
        // Check if this directory is a mount point - if so, show disk details
        if selected_path.is_dir() {
            // Look for a disk that matches this mount point
            for disk in &self.disks {
                if disk.mount_point() == selected_path {
                    self.modal_data = ModalData::DiskDetails {
                        name: disk.name().to_string_lossy().to_string(),
                        mount_point: disk.mount_point().to_string_lossy().to_string(),
                        total_space: disk.total_space(),
                        available_space: disk.available_space(),
                        file_system: disk.file_system().to_string_lossy().to_string(),
                    };
                    self.modal_type = ModalType::DiskDetails;
                    self.show_modal = true;
                    return;
                }
            }
        }
        
        // Get file metadata for regular files/directories
        if let Ok(metadata) = std::fs::metadata(selected_path) {
            let file_size = metadata.len();
            let is_dir = metadata.is_dir();
            let permissions = format!("{:o}", metadata.permissions().mode() & 0o777);
            
            // Get file name without emoji prefix
            let clean_name = selected_item
                .trim_start_matches("📁 ")
                .trim_start_matches("📄 ")
                .to_string();
            
            let content = if is_dir {
                format!(
                    "Name: {}\n\
                    Type: Directory\n\
                    Size: {} items\n\
                    Permissions: {}\n\
                    Path: {}",
                    clean_name,
                    "N/A", // Directory item count would require reading the directory
                    permissions,
                    selected_path.display()
                )
            } else {
                format!(
                    "Name: {}\n\
                    Type: File\n\
                    Size: {}\n\
                    Permissions: {}\n\
                    Path: {}",
                    clean_name,
                    crate::utils::format_memory_size(file_size),
                    permissions,
                    selected_path.display()
                )
            };
            
            self.modal_data = ModalData::SystemDetails {
                hostname: format!("File Info: {}", clean_name),
                os_name: content,
                os_version: String::new(),
                kernel_version: String::new(),
                cpu_count: 0,
                total_memory: 0,
                uptime: 0,
            };
            self.modal_type = ModalType::SystemDetails;
            self.show_modal = true;
        }
    }

    fn hide_modal(&mut self) {
        self.show_modal = false;
    }

    pub fn render_header(&self, frame: &mut Frame, area: Rect) {
        let hostname = System::host_name().unwrap_or_else(|| "unknown-host".to_string());
        let username = std::env::var("USERNAME").or_else(|_| std::env::var("USER")).unwrap_or_else(|_| "unknown-user".to_string());
        let title_text = format!("{}@{} :: SYSTEM MONITOR", username.to_uppercase(), hostname);
        let title = Paragraph::new(title_text)
            .style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
            .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Green)));
        frame.render_widget(title, area);
    }

    pub fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let help_text = if self.show_help {
            "ESC or ? to close • System Monitor v1.0"
        } else {
            "Navigation: ←→hl | ↑↓jk/PgUp/PgDn/Home/End (navigate/cycle) | Enter (open dir) | Backspace (up dir) | r (refresh) | ? (help) | q (quit)"
        };
        let footer = Paragraph::new(help_text)
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)));
        frame.render_widget(footer, area);
    }
}
