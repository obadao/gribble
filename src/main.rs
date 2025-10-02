use color_eyre::Result;
use crossterm::event::{self, Event, KeyEventKind};
use tracing::{error, info};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    DefaultTerminal, Frame,
};
use std::time::Duration;

mod app;
mod network;
mod ui;
mod utils;

use app::App;
use ui::{render_help, render_system_info, render_clock, render_tasks, render_file_browser, render_network_graph};

fn main() -> Result<()> {
    color_eyre::install()?;
    
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("gribble=info")
        .init();
    
    info!("Starting Gribble system monitor");
    
    let terminal = ratatui::init();
    let result = run(terminal);
    ratatui::restore();
    
    match &result {
        Ok(()) => info!("Gribble exited successfully"),
        Err(e) => error!("Gribble exited with error: {}", e),
    }
    
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
    app.render_header(frame, main_layout[0]);

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

    // Footer
    app.render_footer(frame, main_layout[2]);
}
