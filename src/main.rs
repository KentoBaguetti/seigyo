
mod agent;
mod event;
mod ui;

use std::{io, time::Duration};

use crossterm::event::{Event, KeyCode, KeyEventKind};
use tokio::sync::mpsc;

use crate::event::{AgentEvent, App};

#[tokio::main]
async fn main() -> io::Result<()> {
    // Create the channel connecting agent.rs and main.rs.
    let (tx, mut rx) = mpsc::channel::<AgentEvent>(100);

    // Create initial application state.
    let mut app = App::new();

    // Launch Codex in a separate Tokio task.
    let agent_task = tokio::spawn(agent::run_agent(tx));

    // Initialize terminal interface.
    let mut terminal = ratatui::init();

    // Main application loop.
    let result = loop {
        // Process all currently available agent events.
        while let Ok(agent_event) = rx.try_recv() {
            app.handle_event(agent_event);
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
                        if key.kind == KeyEventKind::Press
                            && key.code == KeyCode::Char('q') =>
                    {
                        break Ok(());
                    }

                    Ok(_) => {}

                    Err(error) => break Err(error),
                }
            }

            Ok(false) => {}

            Err(error) => break Err(error),
        }
    };

    // Stop the agent task when the user exits.
    agent_task.abort();
    let _ = agent_task.await;

    // Restore the normal terminal.
    ratatui::restore();

    result
}
