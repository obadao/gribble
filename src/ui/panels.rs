use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph, Sparkline, Wrap},
    Frame,
};
use sysinfo::System;
use chrono::{DateTime, Local};

use crate::{
    app::App,
    utils::{
        format_memory_size, format_network_size, format_network_rate, truncate_string, format_path_display,
        PROCESS_NAME_MAX_LEN, INTERFACE_NAME_MAX_LEN,
    },
};

pub fn render_system_info(app: &App, frame: &mut Frame, area: Rect, is_selected: bool) {
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
    let memory_percent = if total_memory > 0 {
        (memory_usage as f64 / total_memory as f64 * 100.0) as u16
    } else {
        0
    };
    
    // Get uptime
    let uptime = System::uptime();
    let uptime_hours = uptime / 3600;
    let uptime_mins = (uptime % 3600) / 60;

    let cpu_blocks = ((cpu_usage / 10.0).floor() as usize).min(10).max(0);
    let mem_blocks = ((memory_percent as f64 / 10.0).floor() as usize).min(10).max(0);
    let cpu_bar = "█".repeat(cpu_blocks) + &" ".repeat(10 - cpu_blocks);
    let mem_bar = "█".repeat(mem_blocks) + &" ".repeat(10 - mem_blocks);

    let content = vec![
        format!("▶ CPU: {:5.1}% [{}]", 
               cpu_usage,
               cpu_bar),
        format!("▶ RAM: {:5.1}% [{}]", 
               memory_percent,
               mem_bar),
        format!("▶ Memory: {} / {}", 
               format_memory_size(memory_usage),
               format_memory_size(total_memory)),
        format!("▶ Processes: {}", app.system.processes().len()),
        format!("▶ Uptime: {}h {:02}m", uptime_hours, uptime_mins),
        format!("▶ OS: {}", System::name().unwrap_or_else(|| "unknown".to_string())),
        format!("▶ Architecture: {}", std::env::consts::ARCH),
    ];

    let paragraph = Paragraph::new(content.join("\n"))
        .style(Style::default().fg(Color::White))
        .block(block)
        .wrap(Wrap { trim: true });

    frame.render_widget(paragraph, area);
}

pub fn render_clock(app: &App, frame: &mut Frame, area: Rect, is_selected: bool) {
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
        let used_str = format_memory_size(used);
        let total_str = format_memory_size(disk.total_space());
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
    let network_info = if let Some(network) = app.cached_networks.get(app.selected_network) {
        let truncated_name = truncate_string(&network.name, INTERFACE_NAME_MAX_LEN);
        format!("{}: ↓{} ↑{}", 
               truncated_name, 
               format_network_size(network.total_received),
               format_network_size(network.total_transmitted))
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

pub fn render_tasks(app: &App, frame: &mut Frame, area: Rect, is_selected: bool) {
    let border_style = if is_selected {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::White)
    };

    let block = Block::default()
        .title("⚙️ Process Manager")
        .borders(Borders::ALL)
        .border_style(border_style);

    let items: Vec<ListItem> = app.cached_processes
        .iter()
        .enumerate()
        .map(|(i, process)| {
            let memory_formatted = format_memory_size(process.memory);
            // Calculate available space for process name (total width minus CPU%, memory, and separators)
            // CPU% (4) + "│ " (2) + memory (8) + " │ " (3) = 17 characters used, leaving ~35 for process name
            let process_name = truncate_string(&process.name, PROCESS_NAME_MAX_LEN);
            let content = format!("{:4.1}% │ {:>8} │ {}", 
                                process.cpu_usage, 
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

pub fn render_file_browser(app: &App, frame: &mut Frame, area: Rect, is_selected: bool) {
    let border_style = if is_selected {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::White)
    };

    let path_display = format_path_display(&app.current_dir);
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

pub fn render_network_graph(app: &App, frame: &mut Frame, area: Rect, is_selected: bool) {
    let border_style = if is_selected {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::White)
    };

    let interface_name = &app.network_history.current_interface;
    let truncated_interface = truncate_string(interface_name, INTERFACE_NAME_MAX_LEN);
    let network_count = app.cached_networks.len();
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
                          format_network_rate(current_rx_rate), 
                          format_network_size(total_rx));
    let rx_sparkline = Sparkline::default()
        .block(Block::default()
            .title(rx_title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green)))
        .data(&rx_data)
        .style(Style::default().fg(Color::Green));

    // TX Graph  
    let tx_title = format!("TX: {} | Total: {}", 
                          format_network_rate(current_tx_rate), 
                          format_network_size(total_tx));
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
