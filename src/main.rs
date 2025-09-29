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
        }
    }

    fn update(&mut self, networks: &Networks) {
        let (total_rx, total_tx) = networks.list().iter()
            .fold((0, 0), |(rx_acc, tx_acc), (_, network)| {
                (rx_acc + network.total_received(), tx_acc + network.total_transmitted())
            });

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

                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if entry.path().is_dir() {
                        dirs.push(format!("📁 {}", name));
                    } else {
                        files.push(format!("📄 {}", name));
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
            self.network_history.update(&self.networks);
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
                    _ => {}
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                match self.selected_panel {
                    2 => { // Process manager
                        let max_processes = self.system.processes().len().min(10);
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
        "Navigation: ←→hl | ↑↓jk (navigate lists) | Enter (open directory) | r (refresh) | ? (help) | q (quit) • Live Updates"
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
  ↑ ↓ j k  - Navigate within Task Manager
  Enter    - Navigate directories (File Browser)
  r        - Refresh all data
  ?        - Show/hide this help
  q / Esc  - Quit

PANELS:
  1. System Monitor - CPU, Memory, Uptime, Architecture
  2. System Status  - Time, Disk usage, Network stats  
  3. Process Manager- Top processes (navigable with j/k)
  4. File Explorer  - Navigate directories (j/k + Enter)
  5. Network Graph  - Real-time network traffic visualization

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

    let cpu_bar = "█".repeat((cpu_usage / 10.0) as usize).chars().take(10).collect::<String>();
    let mem_bar = "█".repeat((memory_percent / 10) as usize).chars().take(10).collect::<String>();

    let content = vec![
        format!("▶ CPU: {:.1}% [{}{}]", 
               cpu_usage,
               cpu_bar,
               " ".repeat(10 - cpu_bar.len())),
        format!("▶ RAM: {:.1}% [{}{}]", 
               memory_percent,
               mem_bar,
               " ".repeat(10 - mem_bar.len())),
        format!("▶ Memory: {} MB / {} MB", 
               memory_usage / 1024 / 1024,
               total_memory / 1024 / 1024),
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
    let (disk_usage, disk_total) = if let Some(disk) = main_disk {
        let used = disk.total_space() - disk.available_space();
        let used_gb = used / 1024 / 1024 / 1024;
        let total_gb = disk.total_space() / 1024 / 1024 / 1024;
        (used_gb, total_gb)
    } else {
        (0, 0)
    };

    // Get network info
    let network_info = app.networks.list().iter()
        .map(|(name, network)| {
            format!("{}: ↓{} MB ↑{} MB", 
                   name, 
                   network.total_received() / 1024 / 1024,
                   network.total_transmitted() / 1024 / 1024)
        })
        .next()
        .unwrap_or_else(|| "No network data".to_string());

    let content = format!("▶ Time: {}\n▶ Date: {}\n▶ Boot disk: {} GB / {} GB\n▶ Disk usage: {:.1}%\n\n▶ Network: \n  {}\n\n▶ Load avg: {:.2}", 
                         time_str, 
                         date_str,
                         disk_usage,
                         disk_total,
                         if disk_total > 0 { (disk_usage as f64 / disk_total as f64) * 100.0 } else { 0.0 },
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

    let mut processes: Vec<_> = app.system.processes().iter().collect();
    processes.sort_by(|a, b| b.1.cpu_usage().partial_cmp(&a.1.cpu_usage()).unwrap());
    
    let items: Vec<ListItem> = processes
        .iter()
        .enumerate()
        .map(|(i, (_, process))| {
            let memory_mb = process.memory() / 1024 / 1024;
            let content = format!("{:4.1}% │ {:3}MB │ {}", 
                                process.cpu_usage(), 
                                memory_mb,
                                process.name().to_string_lossy());
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

    let main_block = Block::default()
        .title("📡 Network Traffic Monitor")
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
    let rx_title = format!("RX: {} KB/s | Total: {} MB", 
                          current_rx_rate / 1024, 
                          total_rx / 1024 / 1024);
    let rx_sparkline = Sparkline::default()
        .block(Block::default()
            .title(rx_title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green)))
        .data(&rx_data)
        .style(Style::default().fg(Color::Green));

    // TX Graph  
    let tx_title = format!("TX: {} KB/s | Total: {} MB", 
                          current_tx_rate / 1024, 
                          total_tx / 1024 / 1024);
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
