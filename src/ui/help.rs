use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

pub fn render_help(frame: &mut Frame, area: Rect) {
    let help_text = "
SYSTEM MONITOR - HELP

NAVIGATION:
  ← → h l  - Switch between panels
  ↑ ↓ j k  - Navigate within lists/cycle network interfaces
  PgUp/PgDn- Jump by page in lists
  Home/End - Jump to first/last item in lists
  Enter    - Navigate directories (File Browser)
  Backspace- Go up one directory (File Browser)
  r        - Refresh all data
  ?        - Show/hide this help
  q / Esc  - Quit

PANELS:
  1. System Monitor - CPU, Memory, Uptime, Architecture
  2. System Status  - Time, Disk usage, Network stats  
  3. Process Manager- Top processes by CPU usage
  4. File Explorer  - Directory browser with navigation
  5. Network Graph  - Real-time network traffic graphs

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
