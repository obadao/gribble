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
use tracing::{error, warn};

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
    pub cpu_usage: f32,
    pub memory: u64,
}

#[derive(Clone)]
pub struct CachedNetwork {
    pub name: String,
    pub total_received: u64,
    pub total_transmitted: u64,
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
            current_dir,
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
        };
        
        // Initial data cache
        app.refresh_cached_data();
        app
    }

    fn read_directory(path: &PathBuf) -> (Vec<String>, Vec<PathBuf>) {
        match fs::read_dir(path) {
            Ok(entries) => {
                let mut items = vec!["..".to_string()];
                let mut paths = vec![path.parent().unwrap_or(path).to_path_buf()]; // Parent path for ".."
                let mut dirs = Vec::new();
                let mut dir_paths = Vec::new();
                let mut files = Vec::new();
                let mut file_paths = Vec::new();

                for entry in entries.flatten().take(MAX_FILES) {
                    let entry_path = entry.path();
                    let name = entry.file_name().to_string_lossy().to_string();
                    let truncated_name = truncate_string(&name, FILE_NAME_MAX_LEN);
                    
                    if entry_path.is_dir() {
                        dirs.push(format!("📁 {}", truncated_name));
                        dir_paths.push(entry_path);
                    } else {
                        files.push(format!("📄 {}", truncated_name));
                        file_paths.push(entry_path);
                    }
                }
                
                // Sort directories and files together with their paths
                let mut combined: Vec<_> = dirs.into_iter().zip(dir_paths.into_iter()).collect();
                combined.sort_by(|a, b| a.0.cmp(&b.0));
                
                let mut file_combined: Vec<_> = files.into_iter().zip(file_paths.into_iter()).collect();
                file_combined.sort_by(|a, b| a.0.cmp(&b.0));
                
                // Extract sorted items and paths
                for (item, path) in combined {
                    items.push(item);
                    paths.push(path);
                }
                for (item, path) in file_combined {
                    items.push(item);
                    paths.push(path);
                }
                
                (items, paths)
            }
            Err(e) => {
                error!("Failed to read directory {:?}: {}", path, e);
                (vec![format!("<Error: {}>", e)], vec![path.clone()])
            },
        }
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
                self.should_quit = true;
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
                    let (dir_entries, dir_entry_paths) = Self::read_directory(&self.current_dir);
                    self.dir_entries = dir_entries;
                    self.dir_entry_paths = dir_entry_paths;
                    self.last_manual_refresh = Instant::now();
                }
            }
            KeyCode::Char('?') => {
                self.show_help = true;
            }
            KeyCode::Backspace => {
                // Go up one directory (same as selecting "..")
                if self.selected_panel == Panel::FileExplorer { // File browser panel
                    if let Some(parent) = self.current_dir.parent() {
                        self.current_dir = parent.to_path_buf();
                        let (dir_entries, dir_entry_paths) = Self::read_directory(&self.current_dir);
                        self.dir_entries = dir_entries;
                        self.dir_entry_paths = dir_entry_paths;
                        self.selected_file = 0;
                        self.file_list_state.select(Some(0));
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
        
        if selected_item == ".." {
            // Go up one directory using the stored parent path
            self.current_dir = selected_path.clone();
            let (dir_entries, dir_entry_paths) = Self::read_directory(&self.current_dir);
            self.dir_entries = dir_entries;
            self.dir_entry_paths = dir_entry_paths;
            self.selected_file = 0;
            self.file_list_state.select(Some(0));
        } else if selected_item.starts_with("📁") {
            // Enter directory using the stored original path
            if selected_path.is_dir() {
                self.current_dir = selected_path.clone();
                let (dir_entries, dir_entry_paths) = Self::read_directory(&self.current_dir);
                self.dir_entries = dir_entries;
                self.dir_entry_paths = dir_entry_paths;
                self.selected_file = 0;
                self.file_list_state.select(Some(0));
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
