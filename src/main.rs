mod agent;
mod event;
mod ui;

use std::{io, time::Duration};

use crossterm::event::{Event, KeyCode, KeyEventKind};
use tokio::sync::mpsc;

use crate::event::{AgentEvent, AgentRole, AgentState, App};

#[tokio::main]
async fn main() -> io::Result<()> {
    // Create the channel connecting agent.rs and main.rs.
    let (tx, mut rx) = mpsc::channel::<(AgentRole ,AgentEvent)>(100);

    // Create initial application state.
    let mut app = App::new();

    // Initialize terminal interface.
    let mut terminal = ratatui::init();

    // Main application loop.
    let result = loop {
        // Process all currently available agent events.
        while let Ok((agent_role, agent_event)) = rx.try_recv() {
            match agent_role {
                AgentRole::Coder => {
                    let coder_completed = matches!(&agent_event, AgentEvent::Completed);
                    app.coder.handle_event(agent_event);

                    if coder_completed && !matches!(app.reviewer.state, AgentState::Running) {
                        let thread_id = app.reviewer.thread_id.clone();
                        let agent_tx = tx.clone();

                        let mut review_prompt = format!("
                            Review the current Git diff.\n
                            Report bugs and cite file paths and lines when possible.
                            ");
                        match &app.previous_input {
                            Some(previous_input) => {
                                review_prompt = format!("
                                    {review_prompt}\n\n
                                    Previous Prompt: {previous_input}
                                    ");
                            }
                            None => {
                                continue;
                            }
                        }

                        app.reviewer.state = AgentState::Running;

                        tokio::spawn(async move {
                            agent::run_agent(agent_tx, AgentRole::Reviewer, review_prompt.to_string(), thread_id).await;
                        });
                    }
                }

                AgentRole::Reviewer => {
                    app.reviewer.handle_event(agent_event);
                }
            }
        }

        // Render the current application state.
        if let Err(error) = terminal.draw(|frame| {
            ui::draw(frame, &app);
        }) {
            break Err(error);
        }

        // Handle keyboard input without waiting indefinitely.
        match crossterm::event::poll(Duration::from_millis(100)) {
            Ok(true) => {
                match crossterm::event::read() {
                    Ok(Event::Key(key))
                        if key.kind == KeyEventKind::Press => {
                            match key.code {
                                KeyCode::Esc => {
                                    break Ok(());
                                }

                                KeyCode::Char(c) => {
                                    app.input.push(c);
                                }

                                KeyCode::Backspace => {
                                    app.input.pop();
                                }

                                KeyCode::Enter => {
                                    if matches!(app.coder.state, AgentState::Running) {
                                        continue;
                                    }

                                    if app.input.trim().is_empty() {
                                        continue;
                                    }

                                    let prompt = std::mem::take(&mut app.input);
                                    let thread_id = app.coder.thread_id.clone();
                                    let agent_tx = tx.clone();

                                    app.previous_input = Some(prompt.clone());
                                    app.coder.state = AgentState::Running;

                                    tokio::spawn(async move {
                                        agent::run_agent(agent_tx, AgentRole::Coder, prompt, thread_id).await;
                                    });
                                }

                                // Temporary branch. Manually let the user start a review
                                KeyCode::F(2) => {
                                    if matches!(app.reviewer.state, AgentState::Running) {
                                        continue;
                                    }

                                    let prompt = "Review the latest git diff";
                                    let thread_id = app.reviewer.thread_id.clone();
                                    let agent_tx = tx.clone();

                                    app.reviewer.state = AgentState::Running;

                                    tokio::spawn(async move {
                                        agent::run_agent(agent_tx, AgentRole::Reviewer, prompt.to_string(), thread_id).await;
                                    });
                                }

                                _ => {}
                            }
                        }

                    Ok(_) => {}

                    Err(error) => break Err(error),
                }
            }

            Ok(false) => {}

            Err(error) => break Err(error),
        }
    };

    // Restore the normal terminal.
    ratatui::restore();

    result
}
