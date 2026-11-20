use std::process::Stdio;

use serde_json::{Value};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
    sync::mpsc::Sender,
};

use crate::{event::AgentEvent, event::AgentRole};

pub async fn run_agent(
        tx: Sender<(AgentRole, AgentEvent)>,
        role: AgentRole,
        prompt: String,
        thread_id: Option<String>
    ) {

    let mut args = vec![
        "exec".to_string(),
        "--json".to_string(),
        "--model".to_string(),
        "gpt-6.1-terra".to_string(), // use cheap agents for testing
        "--sandbox".to_string()
    ];

    let mut agent_role_args: Vec<String> = vec![];
    match role {
        AgentRole::Coder => {
            agent_role_args = vec!["workspace-write".to_string(), "--skip-git-repo-check".to_string()];
        }

        AgentRole::Reviewer => {
            agent_role_args = vec!["read-only".to_string(), "--skip-git-repo-check".to_string()];
        }
    }
    args.append(&mut agent_role_args);

    match thread_id {
        Some(id) => {
            let mut resume_args = vec!["resume".to_string(), id, prompt];
            args.append(&mut resume_args);
        }

        None => {
            args.push(prompt);
        }
    }

    let mut child = match Command::new("codex")
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
    {
        Ok(child) => child,
        Err(err) => {
            let _ = tx
                .send((role, AgentEvent::Failed(err.to_string())))
                .await;
            return;
        }
    };

    let _ = tx.send((role, AgentEvent::Started)).await;

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let err_tx = tx.clone();

    // Spawn task to read for errors/warnings
    let stderr_task = tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();

        while let Ok(Some(line)) = lines.next_line().await {
            if err_tx
                .send((role, AgentEvent::Activity(format!("stderr: {line}"))))
                .await
                .is_err()
            {
                break;
            }
        }
    });

    let mut lines = BufReader::new(stdout).lines();

    // Main loop to read codex output
    loop {
        match lines.next_line().await {
            Ok(Some(line)) => {
                let Ok(json) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };

                if json["type"] == "thread.started" {
                    if let Some(thread_id) = json["thread_id"].as_str() {
                        let _ = tx
                            .send((role, AgentEvent::ThreadStarted(thread_id.to_string())))
                            .await;
                    }
                }

                let msg = describe_event(&json);

                if tx.send((role, AgentEvent::Activity(msg))).await.is_err() {
                    break;
                }
            }

            Ok(None) => break,

            Err(err) => {
                let _ = tx
                    .send((role, AgentEvent::Failed(err.to_string())))
                    .await;
                return;
            }
        }
    }

    let result = child.wait().await;
    let _ = stderr_task.await;

    match result {
        Ok(status) if status.success() => {
            let _ = tx.send((role, AgentEvent::Completed)).await;
        }

        Ok(status) => {
            let _ = tx
                .send((role, AgentEvent::Failed(format!("Codex exited with status: {status}"))))
                .await;
        }

        Err(err) => {
            let _ = tx
                .send((role, AgentEvent::Failed(err.to_string())))
                .await;
        }
    }
}

fn describe_event(json: &Value) -> String {
    let event_type = json["type"]
        .as_str()
        .unwrap_or("unknown");

    match event_type {
        "thread.started" => "Thread started".to_string(),

        "turn.started" => "Agent is working...".to_string(),

        "turn.completed" => "Agent turn completed".to_string(),

        "turn.failed" => "Agent turn failed".to_string(),

        "item.started" | "item.updated" | "item.completed" => {
            let item_type = json["item"]["type"]
                .as_str()
                .unwrap_or("unknown");

            let detail = json["item"]["command"]
                .as_str()
                .or_else(|| json["item"]["text"].as_str())
                .unwrap_or("");

            let description = if detail.is_empty() {
                item_type.to_string()
            } else {
                format!("{item_type}: {detail}")
            };

            return format!("{event_type} - {description}");
        }

        "error" => {
            let message = json["message"]
                .as_str()
                .unwrap_or("Unknown error");

            return format!("Codex error: {message}");
        }

        _ => event_type.to_string(),
    }
}
