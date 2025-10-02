use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap, Sparkline},
    DefaultTerminal, Frame,
};
use sysinfo::{System, Disks, Networks};
use chrono::{DateTime, Local};
use std::time::{Duration, Instant};
use std::fs;
use std::path::PathBuf;
use std::collections::VecDeque;

struct App {
    should_quit: bool,
    system: System,
    disks: Disks,
    networks: Networks,
    last_update: Instant,
    selected_panel: usize,
    panels: Vec<Panel>,
    current_dir: PathBuf,
    dir_entries: Vec<String>,
    selected_process: usize,
    selected_file: usize,
    selected_network: usize,
    show_help: bool,
    process_list_state: ListState,
    file_list_state: ListState,
    network_history: NetworkHistory,
}

struct NetworkHistory {
    rx_history: VecDeque<u64>,
    tx_history: VecDeque<u64>,
    rx_rates: VecDeque<u64>,
    tx_rates: VecDeque<u64>,
    last_rx_bytes: u64,
    last_tx_bytes: u64,
    max_history: usize,
    current_interface: String,
}

impl NetworkHistory {
    fn new() -> Self {
        Self {
            rx_history: VecDeque::new(),
            tx_history: VecDeque::new(),
            rx_rates: VecDeque::new(),
            tx_rates: VecDeque::new(),
            last_rx_bytes: 0,
            last_tx_bytes: 0,
            max_history: 60, // Keep 60 data points (2 minutes at 2-second intervals)
            current_interface: String::new(),
        }
    }

    fn update(&mut self, networks: &Networks, selected_interface: &str) {
        // Find the selected network interface or use the first available one
        let network_list: Vec<_> = networks.list().iter().take(100).collect(); // Limit to 100 network interfaces
        let (interface_name, network_data) = if let Some(item) = network_list.get(0) {
            // If we have a specific interface selected, try to find it
            if !selected_interface.is_empty() {
                network_list.iter()
                    .find(|(name, _)| *name == selected_interface)
                    .unwrap_or(item)
            } else {
                item
            }
        } else {
            return; // No network interfaces available
        };

        // Update current interface name
        self.current_interface = interface_name.to_string();

        let total_rx = network_data.total_received();
        let total_tx = network_data.total_transmitted();

        if self.last_rx_bytes > 0 && self.last_tx_bytes > 0 {
            // Calculate rate (bytes per 2 seconds)
            let rx_rate = total_rx.saturating_sub(self.last_rx_bytes);
            let tx_rate = total_tx.saturating_sub(self.last_tx_bytes);
            
            self.rx_rates.push_back(rx_rate);
            self.tx_rates.push_back(tx_rate);
            
            if self.rx_rates.len() > self.max_history {
                self.rx_rates.pop_front();
            }
            if self.tx_rates.len() > self.max_history {
                self.tx_rates.pop_front();
            }
        }

        self.rx_history.push_back(total_rx);
        self.tx_history.push_back(total_tx);
        
        if self.rx_history.len() > self.max_history {
            self.rx_history.pop_front();
        }
        if self.tx_history.len() > self.max_history {
            self.tx_history.pop_front();
        }

        self.last_rx_bytes = total_rx;
        self.last_tx_bytes = total_tx;
    }
}

struct Panel {
    active: bool,
}

impl App {
    fn format_memory_size(bytes: u64) -> String {
        let mb = bytes / 1024 / 1024;
        if mb >= 1024 {
            let gb = mb as f64 / 1024.0;
            format!("{:.1} GB", gb)
        } else {
            format!("{} MB", mb)
        }
    }

    fn format_network_size(bytes: u64) -> String {
        let kb = bytes / 1024;
        if kb < 1024 {
            format!("{} KB", kb)
        } else if kb < 1024 * 1024 {
            let mb = kb / 1024;
            format!("{} MB", mb)
        } else {
            let gb = kb as f64 / (1024.0 * 1024.0);
            format!("{:.1} GB", gb)
        }
    }

    fn truncate_string(s: &str, max_len: usize) -> String {
        if s.len() <= max_len {
            s.to_string()
        } else {
            format!("{}...", &s[..max_len.saturating_sub(3)])
        }
    }

    fn format_network_rate(bytes_per_second: u64) -> String {
        let kb_per_sec = bytes_per_second / 1024;
        if kb_per_sec >= 1024 {
            let mb_per_sec = kb_per_sec as f64 / 1024.0;
            format!("{:.1} MB/s", mb_per_sec)
        } else {
            format!("{} KB/s", kb_per_sec)
        }
    }

    fn new() -> Self {
        let mut system = System::new_all();
        system.refresh_all();
        let disks = Disks::new_with_refreshed_list();
        let networks = Networks::new_with_refreshed_list();
        
        let panels = vec![
            Panel { active: true },
            Panel { active: false },
            Panel { active: false },
            Panel { active: false },
            Panel { active: false },  // Network panel
        ];

        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let dir_entries = Self::read_directory(&current_dir);

        let mut process_list_state = ListState::default();
        process_list_state.select(Some(0));
        let mut file_list_state = ListState::default();
        file_list_state.select(Some(0));

        let network_history = NetworkHistory::new();

        Self {
            should_quit: false,
            system,
            disks,
            networks,
            last_update: Instant::now(),
            selected_panel: 0,
            panels,
            current_dir,
            dir_entries,
            selected_process: 0,
            selected_file: 0,
            selected_network: 0,
            show_help: false,
            process_list_state,
            file_list_state,
            network_history,
        }
    }

    fn read_directory(path: &PathBuf) -> Vec<String> {
        match fs::read_dir(path) {
            Ok(entries) => {
                let mut items = vec!["..".to_string()];
                let mut dirs = Vec::new();
                let mut files = Vec::new();

                for entry in entries.flatten().take(10000) { // Limit to 10000 entries
                    let name = entry.file_name().to_string_lossy().to_string();
                    // Truncate very long file/directory names to prevent layout issues
                    let truncated_name = Self::truncate_string(&name, 40);
                    if entry.path().is_dir() {
                        dirs.push(format!("📁 {}", truncated_name));
                    } else {
                        files.push(format!("📄 {}", truncated_name));
                    }
                }
                
                dirs.sort();
                files.sort();
                items.extend(dirs);
                items.extend(files);
                items
            }
            Err(_) => vec!["<Permission Denied>".to_string()],
        }
    }

    fn update(&mut self) {
        // Update system info every 2 seconds
        if self.last_update.elapsed() >= Duration::from_secs(2) {
            self.system.refresh_all();
            self.disks.refresh(true);
            self.networks.refresh(true);
            
            // Get the selected network interface name
            let network_list: Vec<_> = self.networks.list().iter().take(100).collect(); // Limit to 100 network interfaces
            let selected_interface_name = if let Some((name, _)) = network_list.get(self.selected_network) {
                name.to_string()
            } else {
                String::new()
            };
            
            self.network_history.update(&self.networks, &selected_interface_name);
            self.last_update = Instant::now();
        }
    }

    fn handle_key_event(&mut self, key: KeyCode) {
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
                    2 => { // Process manager
                        if self.selected_process > 0 {
                            self.selected_process -= 1;
                            self.process_list_state.select(Some(self.selected_process));
                        }
                    }
                    3 => { // File browser
                        if self.selected_file > 0 {
                            self.selected_file -= 1;
                            self.file_list_state.select(Some(self.selected_file));
                        }
                    }
                    4 => { // Network panel - cycle to previous interface
                        let network_count = self.networks.list().len().min(100); // Limit to 100 network interfaces
                        if network_count > 0 {
                            self.selected_network = if self.selected_network == 0 {
                                network_count - 1
                            } else {
                                self.selected_network - 1
                            };
                            // Reset network history when switching interfaces
                            self.network_history = NetworkHistory::new();
                        }
                    }
                    _ => {}
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                match self.selected_panel {
                    2 => { // Process manager
                        let max_processes = self.system.processes().len().min(1000); // Limit to 1000 processes
                        if self.selected_process < max_processes - 1 {
                            self.selected_process += 1;
                            self.process_list_state.select(Some(self.selected_process));
                        }
                    }
                    3 => { // File browser
                        if self.selected_file < self.dir_entries.len() - 1 {
                            self.selected_file += 1;
                            self.file_list_state.select(Some(self.selected_file));
                        }
                    }
                    4 => { // Network panel - cycle to next interface
                        let network_count = self.networks.list().len().min(100); // Limit to 100 network interfaces
                        if network_count > 0 {
                            self.selected_network = (self.selected_network + 1) % network_count;
                            // Reset network history when switching interfaces
                            self.network_history = NetworkHistory::new();
                        }
                    }
                    _ => {}
                }
            }
            KeyCode::PageUp => {
                match self.selected_panel {
                    2 => { // Process manager
                        let page_size = 10; // Approximate visible items per page
                        self.selected_process = self.selected_process.saturating_sub(page_size);
                        self.process_list_state.select(Some(self.selected_process));
                    }
                    3 => { // File browser
                        let page_size = 10;
                        self.selected_file = self.selected_file.saturating_sub(page_size);
                        self.file_list_state.select(Some(self.selected_file));
                    }
                    _ => {}
                }
            }
            KeyCode::PageDown => {
                match self.selected_panel {
                    2 => { // Process manager
                        let page_size = 10;
                        let max_processes = self.system.processes().len().min(1000); // Limit to 1000 processes
                        self.selected_process = (self.selected_process + page_size).min(max_processes.saturating_sub(1));
                        self.process_list_state.select(Some(self.selected_process));
                    }
                    3 => { // File browser
                        let page_size = 10;
                        let max_files = self.dir_entries.len();
                        self.selected_file = (self.selected_file + page_size).min(max_files.saturating_sub(1));
                        self.file_list_state.select(Some(self.selected_file));
                    }
                    _ => {}
                }
            }
            KeyCode::Home => {
                match self.selected_panel {
                    2 => { // Process manager
                        self.selected_process = 0;
                        self.process_list_state.select(Some(0));
                    }
                    3 => { // File browser
                        self.selected_file = 0;
                        self.file_list_state.select(Some(0));
                    }
                    _ => {}
                }
            }
            KeyCode::End => {
                match self.selected_panel {
                    2 => { // Process manager
                        let max_processes = self.system.processes().len().min(1000); // Limit to 1000 processes
                        if max_processes > 0 {
                            self.selected_process = max_processes - 1;
                            self.process_list_state.select(Some(self.selected_process));
                        }
                    }
                    3 => { // File browser
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
                if self.selected_panel == 3 {
                    self.navigate_into_selected();
                }
            }
            KeyCode::Char('r') => {
                // Force refresh
                self.system.refresh_all();
                self.dir_entries = Self::read_directory(&self.current_dir);
            }
            KeyCode::Char('?') => {
                self.show_help = true;
            }
            _ => {}
        }
    }

    fn navigate_into_selected(&mut self) {
        if self.selected_file >= self.dir_entries.len() {
            return;
        }
        
        let selected_item = &self.dir_entries[self.selected_file];
        
        if selected_item == ".." {
            // Go up one directory
            if let Some(parent) = self.current_dir.parent() {
                self.current_dir = parent.to_path_buf();
                self.dir_entries = Self::read_directory(&self.current_dir);
                self.selected_file = 0;
                self.file_list_state.select(Some(0));
            }
        } else if selected_item.starts_with("📁") {
            // Enter directory
            let dir_name = selected_item.trim_start_matches("📁 ");
            let new_path = self.current_dir.join(dir_name);
            if new_path.is_dir() {
                self.current_dir = new_path;
                self.dir_entries = Self::read_directory(&self.current_dir);
                self.selected_file = 0;
                self.file_list_state.select(Some(0));
            }
        }
        // For files (📄), we don't do anything - could open them in future
    }

    fn select_next_panel(&mut self) {
        self.panels[self.selected_panel].active = false;
        self.selected_panel = (self.selected_panel + 1) % self.panels.len();
        self.panels[self.selected_panel].active = true;
    }

    fn select_previous_panel(&mut self) {
        self.panels[self.selected_panel].active = false;
        self.selected_panel = if self.selected_panel == 0 {
            self.panels.len() - 1
        } else {
            self.selected_panel - 1
        };
        self.panels[self.selected_panel].active = true;
    }
}

fn main() -> Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();
    let result = run(terminal);
    ratatui::restore();
    result
}

fn run(mut terminal: DefaultTerminal) -> Result<()> {
    let mut app = App::new();
    
    loop {
        app.update();
        terminal.draw(|frame| render(&app, frame))?;
        
        if let Ok(event) = event::poll(Duration::from_millis(100)) {
            if event {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        app.handle_key_event(key.code);
                    }
                }
            }
        }
        
        if app.should_quit {
            break Ok(());
        }
    }
}

fn render(app: &App, frame: &mut Frame) {
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Main content
            Constraint::Length(3), // Footer
        ])
        .split(frame.area());

    // Header
    let hostname = System::host_name().unwrap_or_else(|| "UNKNOWN-HOST".to_string());
    let username = std::env::var("USERNAME").or_else(|_| std::env::var("USER")).unwrap_or_else(|_| "UNKNOWN-USER".to_string());
    let title_text = format!("{}@{} :: SYSTEM MONITOR", username.to_uppercase(), hostname);
    let title = Paragraph::new(title_text)
        .style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Green)));
    frame.render_widget(title, main_layout[0]);

    if app.show_help {
        render_help(frame, main_layout[1]);
    } else {
        // Main content area - split into upper 2x2 grid and bottom network panel
        let main_content_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
            .split(main_layout[1]);

        let content_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(main_content_layout[0]);

        let left_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(content_layout[0]);

        let right_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(content_layout[1]);

        // Render panels
        render_system_info(app, frame, left_layout[0], app.selected_panel == 0);
        render_clock(app, frame, right_layout[0], app.selected_panel == 1);
        render_tasks(app, frame, left_layout[1], app.selected_panel == 2);
        render_file_browser(app, frame, right_layout[1], app.selected_panel == 3);
        render_network_graph(app, frame, main_content_layout[1], app.selected_panel == 4);
    }

    // Footer with navigation help
    let help_text = if app.show_help {
        "ESC or ? to close • System Monitor v1.0"
    } else {
        "Navigation: ←→hl | ↑↓jk/PgUp/PgDn/Home/End (navigate/cycle) | Enter (open dir) | r (refresh) | ? (help) | q (quit)"
    };
    let footer = Paragraph::new(help_text)
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)));
    frame.render_widget(footer, main_layout[2]);
}

fn render_help(frame: &mut Frame, area: Rect) {
    let help_text = "
SYSTEM MONITOR - HELP

NAVIGATION:
  ← → h l  - Switch between panels
  ↑ ↓ j k  - Navigate within lists/cycle network interfaces
  PgUp/PgDn- Jump by page in lists
  Home/End - Jump to first/last item in lists
  Enter    - Navigate directories (File Browser)
  r        - Refresh all data
  ?        - Show/hide this help
  q / Esc  - Quit

PANELS:
  1. System Monitor - CPU, Memory, Uptime, Architecture
  2. System Status  - Time, Disk usage, Network stats  
  3. Process Manager- Top processes (j/k/PgUp/PgDn/Home/End)
  4. File Explorer  - Navigate directories (j/k/PgUp/PgDn/Home/End + Enter)
  5. Network Graph  - Real-time network traffic (↑↓ to cycle interfaces)

FEATURES:
  • Real-time system monitoring
  • Interactive process viewer
  • File system navigation
  • Keyboard-driven interface
  • Live updates every 2 seconds
  • Cross-platform compatibility

Press '?' or Esc to close this help.
    ";

    let help_block = Block::default()
        .title("Help")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let paragraph = Paragraph::new(help_text.trim())
        .style(Style::default().fg(Color::White))
        .block(help_block)
        .wrap(Wrap { trim: true });

    frame.render_widget(paragraph, area);
}

fn render_system_info(app: &App, frame: &mut Frame, area: Rect, is_selected: bool) {
    let border_style = if is_selected {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::White)
    };

    let block = Block::default()
        .title("💻 System Monitor")
        .borders(Borders::ALL)
        .border_style(border_style);

    let cpu_usage = app.system.global_cpu_usage();
    let memory_usage = app.system.used_memory();
    let total_memory = app.system.total_memory();
    let memory_percent = (memory_usage as f64 / total_memory as f64 * 100.0) as u16;
    
    // Get uptime
    let uptime = System::uptime();
    let uptime_hours = uptime / 3600;
    let uptime_mins = (uptime % 3600) / 60;

    let cpu_bar = "█".repeat(((cpu_usage / 10.0) as usize).min(10).max(0));
    let mem_bar = "█".repeat(((memory_percent / 10) as usize).min(10).max(0));

    let content = vec![
        format!("▶ CPU: {:.1}% [{}{}]", 
               cpu_usage,
               cpu_bar,
               " ".repeat(10usize.saturating_sub(cpu_bar.len()))),
        format!("▶ RAM: {:.1}% [{}{}]", 
               memory_percent,
               mem_bar,
               " ".repeat(10usize.saturating_sub(mem_bar.len()))),
        format!("▶ Memory: {} / {}", 
               App::format_memory_size(memory_usage),
               App::format_memory_size(total_memory)),
        format!("▶ Processes: {}", app.system.processes().len()),
        format!("▶ Uptime: {}h {:02}m", uptime_hours, uptime_mins),
        format!("▶ OS: {}", System::name().unwrap_or_else(|| "Unknown".to_string())),
        format!("▶ Architecture: {}", std::env::consts::ARCH),
    ];

    let paragraph = Paragraph::new(content.join("\n"))
        .style(Style::default().fg(Color::White))
        .block(block)
        .wrap(Wrap { trim: true });

    frame.render_widget(paragraph, area);
}

fn render_clock(app: &App, frame: &mut Frame, area: Rect, is_selected: bool) {
    let border_style = if is_selected {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::White)
    };

    let block = Block::default()
        .title("🕐 System Status")
        .borders(Borders::ALL)
        .border_style(border_style);

    let now: DateTime<Local> = Local::now();
    let time_str = now.format("%H:%M:%S").to_string();
    let date_str = now.format("%A, %B %d").to_string();

    // Get disk info
    let main_disk = app.disks.list().first();
    let (disk_usage_str, disk_total_str, disk_percent) = if let Some(disk) = main_disk {
        let used = disk.total_space() - disk.available_space();
        let used_str = App::format_memory_size(used);
        let total_str = App::format_memory_size(disk.total_space());
        let percent = if disk.total_space() > 0 { 
            (used as f64 / disk.total_space() as f64) * 100.0 
        } else { 
            0.0 
        };
        (used_str, total_str, percent)
    } else {
        ("0 MB".to_string(), "0 MB".to_string(), 0.0)
    };

    // Get network info for the selected interface
    let network_list: Vec<_> = app.networks.list().iter().take(100).collect(); // Limit to 100 network interfaces
    let network_info = if let Some((name, network)) = network_list.get(app.selected_network) {
        let truncated_name = App::truncate_string(name, 15);
        format!("{}: ↓{} ↑{}", 
               truncated_name, 
               App::format_network_size(network.total_received()),
               App::format_network_size(network.total_transmitted()))
    } else {
        "No network data".to_string()
    };

    let content = format!("▶ Time: {}\n▶ Date: {}\n▶ Boot disk: {} / {}\n▶ Disk usage: {:.1}%\n\n▶ Network: \n  {}\n\n▶ Load avg: {:.2}", 
                         time_str, 
                         date_str,
                         disk_usage_str,
                         disk_total_str,
                         disk_percent,
                         network_info,
                         System::load_average().one);

    let paragraph = Paragraph::new(content)
        .style(Style::default().fg(Color::Cyan))
        .block(block);

    frame.render_widget(paragraph, area);
}

fn render_tasks(app: &App, frame: &mut Frame, area: Rect, is_selected: bool) {
    let border_style = if is_selected {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::White)
    };

    let block = Block::default()
        .title("⚙️ Process Manager")
        .borders(Borders::ALL)
        .border_style(border_style);

    let mut processes: Vec<_> = app.system.processes().iter().take(1000).collect(); // Limit to 1000 processes
    processes.sort_by(|a, b| b.1.cpu_usage().partial_cmp(&a.1.cpu_usage()).unwrap());
    
    let items: Vec<ListItem> = processes
        .iter()
        .enumerate()
        .map(|(i, (_, process))| {
            let memory_formatted = App::format_memory_size(process.memory());
            // Calculate available space for process name (total width minus CPU%, memory, and separators)
            // CPU% (4) + "│ " (2) + memory (8) + " │ " (3) = 17 characters used, leaving ~35 for process name
            let process_name = App::truncate_string(&process.name().to_string_lossy(), 35);
            let content = format!("{:4.1}% │ {:>8} │ {}", 
                                process.cpu_usage(), 
                                memory_formatted,
                                process_name);
            let style = if is_selected && i == app.selected_process {
                Style::default().fg(Color::Black).bg(Color::Yellow)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(content).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .style(Style::default().fg(Color::White))
        .highlight_style(Style::default().fg(Color::Black).bg(Color::Yellow));

    let mut list_state = app.process_list_state.clone();
    frame.render_stateful_widget(list, area, &mut list_state);
}

fn render_file_browser(app: &App, frame: &mut Frame, area: Rect, is_selected: bool) {
    let border_style = if is_selected {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::White)
    };

    let path_display = if app.current_dir.to_string_lossy().len() > 30 {
        format!("...{}", app.current_dir.to_string_lossy().chars().rev().take(27).collect::<String>().chars().rev().collect::<String>())
    } else {
        app.current_dir.to_string_lossy().to_string()
    };
    let title = format!("📂 Explorer: {}", path_display);
    
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);

    let items: Vec<ListItem> = app.dir_entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let style = if is_selected && i == app.selected_file {
                Style::default().fg(Color::Black).bg(Color::Yellow)
            } else {
                Style::default().fg(Color::Cyan)
            };
            ListItem::new(entry.clone()).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .style(Style::default().fg(Color::Cyan))
        .highlight_style(Style::default().fg(Color::Black).bg(Color::Yellow));

    let mut list_state = app.file_list_state.clone();
    frame.render_stateful_widget(list, area, &mut list_state);
}

fn render_network_graph(app: &App, frame: &mut Frame, area: Rect, is_selected: bool) {
    let border_style = if is_selected {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::White)
    };

    let interface_name = &app.network_history.current_interface;
    let truncated_interface = App::truncate_string(interface_name, 20);
    let network_count = app.networks.list().len().min(100); // Limit to 100 network interfaces
    let title = if network_count > 1 {
        format!("📡 Network Traffic Monitor - {} ({}/{}) [↑↓ to cycle]", 
                truncated_interface, app.selected_network + 1, network_count)
    } else {
        format!("📡 Network Traffic Monitor - {}", truncated_interface)
    };
    
    let main_block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner_area = main_block.inner(area);
    frame.render_widget(main_block, area);

    // Split into two halves for RX and TX graphs
    let graph_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner_area);

    // Calculate current rates and totals
    let current_rx_rate = app.network_history.rx_rates.back().copied().unwrap_or(0);
    let current_tx_rate = app.network_history.tx_rates.back().copied().unwrap_or(0);
    let total_rx = app.network_history.rx_history.back().copied().unwrap_or(0);
    let total_tx = app.network_history.tx_history.back().copied().unwrap_or(0);

    // Convert to sparkline data (u64 values)
    let rx_data: Vec<u64> = app.network_history.rx_rates.iter().copied().collect();
    let tx_data: Vec<u64> = app.network_history.tx_rates.iter().copied().collect();

    // RX Graph
    let rx_title = format!("RX: {} | Total: {}", 
                          App::format_network_rate(current_rx_rate), 
                          App::format_network_size(total_rx));
    let rx_sparkline = Sparkline::default()
        .block(Block::default()
            .title(rx_title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green)))
        .data(&rx_data)
        .style(Style::default().fg(Color::Green));

    // TX Graph  
    let tx_title = format!("TX: {} | Total: {}", 
                          App::format_network_rate(current_tx_rate), 
                          App::format_network_size(total_tx));
    let tx_sparkline = Sparkline::default()
        .block(Block::default()
            .title(tx_title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Red)))
        .data(&tx_data)
        .style(Style::default().fg(Color::Red));

    frame.render_widget(rx_sparkline, graph_layout[0]);
    frame.render_widget(tx_sparkline, graph_layout[1]);
}
