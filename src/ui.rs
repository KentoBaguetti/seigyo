use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph, Wrap},
};

use crate::{
    consts::MAX_REVIEW_COUNT,
    event::{AgentSession, AgentState, App, WorkflowStatus},
};

pub fn draw(frame: &mut Frame, app: &App) {
    const FULL_TOP_SPACING: u16 = 3;
    const FULL_AGENT_HEIGHT: u16 = 6;
    const ARROW_HEIGHT: u16 = 2;
    const FOOTER_HEIGHT: u16 = 3;
    const MIN_LOG_HEIGHT: u16 = 5;
    const MAX_LOG_HEIGHT: u16 = 30;
    const BORDER_ROWS: u16 = 2;
    const MIN_PROMPT_HEIGHT: u16 = BORDER_ROWS + 1;

    let area = frame.area();
    let full_layout_min_height = FULL_TOP_SPACING
        + 2 * FULL_AGENT_HEIGHT
        + ARROW_HEIGHT
        + FOOTER_HEIGHT
        + MIN_LOG_HEIGHT
        + MIN_PROMPT_HEIGHT;
    let (top_spacing, agent_height) = if area.height < full_layout_min_height {
        (1, 4)
    } else {
        (FULL_TOP_SPACING, FULL_AGENT_HEIGHT)
    };
    let prompt_width = area.width.saturating_sub(2).max(1);
    let (mut prompt_lines, mut cursor_column) = wrap_line(&app.input, prompt_width);
    // The cursor moves to the next row when the final row is full.
    if cursor_column == prompt_width {
        prompt_lines.push(String::new());
        cursor_column = 0;
    }
    let available = area
        .height
        .saturating_sub(top_spacing + 2 * agent_height + ARROW_HEIGHT + FOOTER_HEIGHT);
    let max_prompt_height = available
        .saturating_sub(MIN_LOG_HEIGHT)
        .max(MIN_PROMPT_HEIGHT);
    let prompt_height =
        (prompt_lines.len() + usize::from(BORDER_ROWS)).min(usize::from(max_prompt_height)) as u16;
    let log_height = available.saturating_sub(prompt_height).min(MAX_LOG_HEIGHT);

    let [
        _top_spacing,
        coder_area,
        handoff,
        reviewer_area,
        log_area,
        prompt_section,
        footer_area,
    ] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(top_spacing),
            Constraint::Length(agent_height),
            Constraint::Length(ARROW_HEIGHT),
            Constraint::Length(agent_height),
            Constraint::Length(log_height),
            Constraint::Length(prompt_height),
            Constraint::Length(FOOTER_HEIGHT),
        ])
        .areas(area);

    let inset = Margin {
        horizontal: (coder_area.width / 10)
            .clamp(5, 16)
            .min(coder_area.width / 4),
        vertical: 0,
    };
    let coder_box = coder_area.inner(inset);
    let reviewer_box = reviewer_area.inner(inset);
    frame.render_widget(
        agent_panel(" Coding Agent ", &app.coder, coder_box.height),
        coder_box,
    );
    frame.render_widget(
        agent_panel(" Reviewer Agent ", &app.reviewer, reviewer_box.height),
        reviewer_box,
    );
    draw_agent_arrows(frame, coder_box, handoff, reviewer_box, inset.horizontal);

    let [coder_log, reviewer_log] =
        Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
            .areas(log_area.inner(inset));
    frame.render_widget(
        log_panel(" Coding Agent Log ", &app.coder.logs, coder_log),
        coder_log,
    );
    frame.render_widget(
        log_panel(" Reviewer Agent Log ", &app.reviewer.logs, reviewer_log),
        reviewer_log,
    );

    // Text prompt for user
    let prompt_block = Block::bordered().title(" Prompt ");
    let prompt_area = prompt_block.inner(prompt_section);
    frame.render_widget(prompt_block, prompt_section);

    if !prompt_area.is_empty() {
        let first_visible = prompt_lines
            .len()
            .saturating_sub(usize::from(prompt_area.height));
        let cursor_row = (prompt_lines.len() - first_visible - 1) as u16;
        let visible_lines: Vec<_> = prompt_lines
            .into_iter()
            .skip(first_visible)
            .map(Line::raw)
            .collect();
        frame.render_widget(Paragraph::new(visible_lines), prompt_area);
        frame.set_cursor_position((prompt_area.x + cursor_column, prompt_area.y + cursor_row));
    }

    frame.render_widget(workflow_footer(app, footer_area.width), footer_area);
}

fn workflow_footer(app: &App, width: u16) -> Paragraph<'static> {
    let (status, color) = match app.workflow_status {
        WorkflowStatus::Idle => ("Idle", Color::Gray),
        WorkflowStatus::Running => ("Running", Color::Yellow),
        WorkflowStatus::Revising => ("Revising", Color::Yellow),
        WorkflowStatus::Passed => ("Passed", Color::Green),
        WorkflowStatus::ChangesStillRequested => ("Changes still requested", Color::Yellow),
        WorkflowStatus::NeedsManualAttention => ("Needs manual attention", Color::Red),
    };

    let count = format!("{}/{MAX_REVIEW_COUNT}", app.revision_count);
    let detail = format!(" · {count} automatic revisions used");
    let line = Line::from(vec![
        Span::styled(status, Style::default().fg(color)),
        if status.len() + detail.chars().count() <= usize::from(width.saturating_sub(2)) {
            Span::raw(detail)
        } else {
            Span::raw(format!(" · {count}"))
        },
    ]);

    Paragraph::new(line).block(
        Block::bordered()
            .title(" Workflow ")
            .title_bottom(Line::from(" Esc to quit ").right_aligned()),
    )
}

fn wrap_line(input: &str, width: u16) -> (Vec<String>, u16) {
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut column = 0;
    for grapheme in Span::raw(input).styled_graphemes(Style::default()) {
        let cells = Span::raw(grapheme.symbol).width() as u16;
        if column > 0 && cells > width - column {
            lines.push(std::mem::take(&mut line));
            column = 0;
        }
        line.push_str(grapheme.symbol);
        column += cells.min(width);
    }

    lines.push(line);
    (lines, column)
}

fn log_panel(title: &'static str, logs: &[String], area: Rect) -> Paragraph<'static> {
    let block = Block::bordered().title(title);
    let inner = block.inner(area);
    if inner.is_empty() {
        return Paragraph::new("").block(block);
    }

    // Count screen rows after wrapping, starting at the newest line.
    let mut visible: Vec<_> = logs
        .iter()
        .rev()
        .flat_map(|entry| entry.lines().rev())
        .flat_map(|line| wrap_line(line, inner.width).0.into_iter().rev())
        .take(usize::from(inner.height))
        .map(Line::raw)
        .collect();
    visible.reverse();
    Paragraph::new(visible).block(block)
}

fn draw_agent_arrows(frame: &mut Frame, coder: Rect, handoff: Rect, reviewer: Rect, inset: u16) {
    if coder.is_empty() || reviewer.is_empty() {
        return;
    }

    let style = Style::default().fg(Color::Cyan);
    let buffer = frame.buffer_mut();
    let center_x = coder.x + coder.width / 2;
    for y in handoff.y..handoff.bottom() {
        let symbol = if y + 1 == handoff.bottom() {
            "▼"
        } else {
            "┃"
        };
        buffer.set_string(center_x, y, symbol, style);
    }

    if inset >= 4 && coder.height >= 3 && reviewer.height >= 3 {
        let loop_x = coder.x - inset / 2 - 1;
        let coder_y = coder.y + coder.height / 2;
        let reviewer_y = reviewer.y + reviewer.height / 2;
        let branch = "━".repeat(usize::from(coder.x - loop_x - 2));
        buffer.set_string(loop_x, coder_y, format!("┏{branch}▶"), style);
        for y in coder_y + 1..reviewer_y {
            buffer.set_string(loop_x, y, "┃", style);
        }
        buffer.set_string(loop_x, reviewer_y, format!("┗{branch}━┥"), style);
    }
}

fn agent_panel(title: &'static str, session: &AgentSession, height: u16) -> Paragraph<'static> {
    let (status, color) = match session.state {
        AgentState::Idle => ("Idle", Color::Gray),
        AgentState::Running => ("Running", Color::Yellow),
        AgentState::Completed => ("Completed", Color::Green),
        AgentState::Failed => ("Failed", Color::Red),
        AgentState::_Cancelled => ("Cancelled", Color::LightMagenta),
    };

    let spacing = if height >= 5 { "\n\n" } else { "\n" };
    Paragraph::new(format!(
        "Status: {status}{spacing}Current activity: {}",
        session.activity
    ))
    .style(Style::default().fg(color))
    .wrap(Wrap { trim: true })
    .block(Block::bordered().title(title))
}
