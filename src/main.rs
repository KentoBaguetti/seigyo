mod agent;
mod event;
mod ui;
mod consts;

use std::{io, time::Duration};

use crossterm::event::{Event, KeyCode, KeyEventKind};
use tokio::sync::mpsc;

use crate::event::{AgentEvent, AgentRole, AgentState, App, ReviewJson, WorkflowStatus};
use crate::consts::{REVIEW_INSTRUCTIONS, CODER_REVIEW_INSTRUCTIONS, MAX_REVIEW_COUNT};

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

                        let review_prompt;
                        let Some(previous_input) = app.previous_input.as_deref() else {
                            continue;
                        };
                        let baseline_git_commit = app.baseline_git_commit.as_deref().unwrap_or("Automatic review skipped: no Git starting commit.");
                        review_prompt = format!("
                            {REVIEW_INSTRUCTIONS}\n\n
                            Original coding request: {previous_input}\n\n
                            Baseline git commit hash: {baseline_git_commit}
                            ");

                        app.reviewer.state = AgentState::Running;
                        app.latest_review = None;

                        tokio::spawn(async move {
                            agent::run_agent(agent_tx, AgentRole::Reviewer, review_prompt.to_string(), thread_id).await;
                        });
                    }
                }

                AgentRole::Reviewer => match agent_event {
                    AgentEvent::ReviewReady(review_message) => {
                        app.latest_review = Some(review_message);
                    }

                    AgentEvent::Completed => {
                        app.reviewer.handle_event(AgentEvent::Completed);
                        let Some(latest_review) = &app.latest_review else {
                            continue;
                        };
                        let Some(review_json) = parse_review_json(latest_review.as_str()) else {
                            continue;
                        };

                        let verdict = review_json.verdict;
                        let findings = review_json.findings.trim();

                        app.workflow_status = WorkflowStatus::NeedsManualAttention;

                        if verdict == "PASS" {
                            app.workflow_status = WorkflowStatus::Passed;
                        } else if verdict == "CHANGES_REQUESTED"
                            && matches!(app.coder.state, AgentState::Completed)
                            && app.revision_count < MAX_REVIEW_COUNT
                            && findings.len() > 0
                            {
                            let agent_tx = tx.clone();
                            let Some(thread_id) = app.coder.thread_id.clone() else {
                                continue;
                            };
                            let prompt = format!(
                               "{CODER_REVIEW_INSTRUCTIONS}{findings}"
                            );

                            app.coder.state = AgentState::Running;
                            app.revision_count += 1;
                            app.latest_review = None;
                            app.workflow_status = WorkflowStatus::Revising;

                            tokio::spawn(async move {
                                agent::run_agent(agent_tx, AgentRole::Coder, prompt.to_string(), Some(thread_id)).await;
                            });
                        } else {
                            app.workflow_status = WorkflowStatus::ChangesStillRequested;
                        }
                    }

                    event => {
                        app.reviewer.handle_event(event);
                    }
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
                                    if matches!(app.coder.state, AgentState::Running) || matches!(app.reviewer.state, AgentState::Running) {
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
                                    app.revision_count = 0;
                                    app.latest_review = None;
                                    app.baseline_git_commit = record_git_command_output(vec!["rev-parse".to_string(), "--verify".to_string(), "HEAD".to_string()]); // get the baseline git commit hash
                                    app.workflow_status = WorkflowStatus::Running;

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

                                // temporary branch to send the reviewed message to the coder
                                KeyCode::F(3) => {
                                    if matches!(app.coder.state, AgentState::Running) || matches!(app.reviewer.state, AgentState::Running) {
                                        continue;
                                    }

                                    let thread_id = app.coder.thread_id.clone();
                                    let agent_tx = tx.clone();

                                    let Some(review_message) = app.latest_review.take() else {
                                        continue;
                                    };

                                    let prompt = format!("Address the applicable findings from this review:\n\n{review_message}");
                                    app.coder.state = AgentState::Running;

                                    tokio::spawn(async move {
                                        agent::run_agent(agent_tx, AgentRole::Coder, prompt, thread_id).await;
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

fn parse_review_json(raw_json: &str) -> Option<ReviewJson> {
    return serde_json::from_str(raw_json).ok();
}

fn record_git_command_output(git_args: Vec<String>) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(git_args)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    return Some(String::from_utf8_lossy(&output.stdout).trim().to_string());
}
