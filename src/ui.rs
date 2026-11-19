use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::event::{AgentState, App};

pub fn draw(frame: &mut Frame, app: &App) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(frame.area());

    // Agent state panel
    let (status, color) = match app.state {
        AgentState::Idle => ("Idle", Color::Gray),
        AgentState::Running => ("Running", Color::Yellow),
        AgentState::Completed => ("Completed", Color::Green),
        AgentState::Failed => ("Failed", Color::Red),
        AgentState::_Cancelled => ("Cancelled", Color::LightMagenta)
    };

    let status_text = format!(
        "Status: {}\n\nCurrent activity: {}",
        status,
        app.activity
    );

    let status_panel = Paragraph::new(status_text)
        .style(Style::default().fg(color))
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .title(" Codex Agent ")
                .borders(Borders::ALL),
        );

    frame.render_widget(status_panel, sections[0]);

    // Event log panel
    let log_height = sections[1].height.saturating_sub(2) as usize;

    let logs = app.logs
        .iter()
        .rev()
        .take(log_height)
        .rev()
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");

    let log_panel = Paragraph::new(logs)
        .block(
            Block::default()
                .title(" Live Event Log ")
                .borders(Borders::ALL),
        );

    frame.render_widget(log_panel, sections[1]);

    // Footer
    let footer = Paragraph::new(" Press Q to quit ")
        .block(
            Block::default()
                .borders(Borders::ALL),
        );

    frame.render_widget(footer, sections[2]);
}
