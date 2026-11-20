use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::event::{AgentState, App};

pub fn draw(frame: &mut Frame, app: &App) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Length(6),
            Constraint::Min(5),
            Constraint::Length(3),
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

    // Agent panel
    let status_panel = Paragraph::new(status_text.clone())
        .style(Style::default().fg(color))
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .title(" Codex Agent ")
                .borders(Borders::ALL),
        );

    frame.render_widget(status_panel, sections[0]);

    // Reviewer panel
    let reviewer_panel = Paragraph::new(status_text.clone())
        .style(Style::default().fg(color))
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .title(" Reviewer Agent ")
                .borders(Borders::ALL),
        );

    frame.render_widget(reviewer_panel, sections[1]);

    // Event log panel
    let log_height = sections[2].height.saturating_sub(2) as usize;

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

    frame.render_widget(log_panel, sections[2]);

    // Text prompt for user
    let prompt_block = Block::bordered().title(" Prompt ");
    let mut prompt_area = prompt_block.inner(sections[3]);
    frame.render_widget(prompt_block, sections[3]);

    if !prompt_area.is_empty() {
        prompt_area.width -= 1;
        let input = Line::raw(app.input.as_str());
        let input_width = input.width();
        let cursor_x = input_width.min(usize::from(prompt_area.width)) as u16;
        if input_width > usize::from(prompt_area.width) {
            frame.render_widget(input.right_aligned(), prompt_area);
        } else {
            frame.render_widget(input, prompt_area);
        }
        frame.set_cursor_position((prompt_area.x + cursor_x, prompt_area.y));
    }

    // Footer
    let footer = Paragraph::new(" Press 'esc' to quit ")
        .block(
            Block::default()
                .borders(Borders::ALL),
        );

    frame.render_widget(footer, sections[4]);
}
