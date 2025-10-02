use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Wrap, Clear},
    style::{Style, Color},
};

use crate::app::{App, ModalData};
use crate::utils::{format_memory_size, format_network_size, format_network_rate};

pub fn render_modal(app: &App, frame: &mut Frame, area: Rect) {
    if !app.show_modal {
        return;
    }

    // Create modal area (centered)
    let modal_width = (area.width * 3) / 4;
    let modal_height = (area.height * 3) / 4;
    let modal_x = (area.width - modal_width) / 2;
    let modal_y = (area.height - modal_height) / 2;

    let modal_area = Rect::new(
        area.x + modal_x,
        area.y + modal_y,
        modal_width,
        modal_height,
    );

    // Create modal content based on type
    let (title, content) = match &app.modal_data {
        ModalData::ProcessDetails { name, pid, cpu_usage, memory_usage, status, cmd } => {
            let title = format!("Process Details: {}", name);
            let content = format!(
                "PID: {}\n\
                CPU Usage: {:.1}%\n\
                Memory Usage: {}\n\
                Status: {}\n\
                Command: {}",
                pid,
                cpu_usage,
                format_memory_size(*memory_usage),
                status,
                cmd
            );
            (title, content)
        }
        ModalData::NetworkDetails { name, total_received, total_transmitted, received_rate, transmitted_rate } => {
            let title = format!("Network Details: {}", name);
            let content = format!(
                "Total Received: {}\n\
                Total Transmitted: {}\n\
                Current RX Rate: {}\n\
                Current TX Rate: {}",
                format_network_size(*total_received),
                format_network_size(*total_transmitted),
                format_network_rate(*received_rate),
                format_network_rate(*transmitted_rate)
            );
            (title, content)
        }
        ModalData::SystemDetails { hostname, os_name, os_version, kernel_version, cpu_count, total_memory, uptime } => {
            let title = if hostname.starts_with("File Info:") {
                hostname.clone()
            } else {
                "System Details".to_string()
            };
            let content = if os_version.is_empty() {
                // This is file info (reusing SystemDetails struct)
                os_name.clone()
            } else {
                // This is actual system info
                format!(
                    "Hostname: {}\n\
                    OS: {} {}\n\
                    Kernel: {}\n\
                    CPU Count: {}\n\
                    Total Memory: {}\n\
                    Uptime: {} seconds",
                    hostname,
                    os_name,
                    os_version,
                    kernel_version,
                    cpu_count,
                    format_memory_size(*total_memory),
                    uptime
                )
            };
            (title, content)
        }
        ModalData::DiskDetails { name, mount_point, total_space, available_space, file_system } => {
            let title = format!("Disk Details: {}", name);
            let used_space = total_space - available_space;
            let usage_percent = if *total_space > 0 {
                (used_space as f64 / *total_space as f64) * 100.0
            } else {
                0.0
            };
            let content = format!(
                "Mount Point: {}\n\
                File System: {}\n\
                Total Space: {}\n\
                Used Space: {} ({:.1}%)\n\
                Available Space: {}",
                mount_point,
                file_system,
                format_memory_size(*total_space),
                format_memory_size(used_space),
                usage_percent,
                format_memory_size(*available_space)
            );
            (title, content)
        }
    };

    // Clear the modal area first to ensure no background interference
    frame.render_widget(Clear, modal_area);
    
    // Then fill the entire modal area with solid black background (matching app default)
    let solid_background = Block::default()
        .style(Style::default().bg(Color::Black));
    frame.render_widget(solid_background, modal_area);

    // Create content area inside the modal (full height minus borders)
    let content_area = Rect::new(
        modal_area.x + 1,
        modal_area.y + 1,
        modal_area.width - 2,
        modal_area.height - 2, // Full height minus borders
    );

    // Render modal content with solid black background
    let modal_content = Paragraph::new(content)
        .wrap(Wrap { trim: true })
        .style(Style::default()
            .fg(Color::White)
            .bg(Color::Black)); // Solid black background

    frame.render_widget(modal_content, content_area);

    // Render the modal border on top with title
    let modal_block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .style(Style::default()
            .fg(Color::White)
            .bg(Color::Black)); // Solid black background
    frame.render_widget(modal_block, modal_area);
}
