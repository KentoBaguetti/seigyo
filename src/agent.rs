use std::{process::Stdio};

use serde_json::{Value};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
    sync::mpsc::Sender,
};

use crate::event::AgentEvent;

pub async fn run_agent(tx: Sender<AgentEvent>) {
    let mut child = match Command::new("codex")
        .args([
            "exec",
            "--json",
            "--sandbox",
            "read-only",
            "Inspect this repository and summarize its structure. Do not modify files.",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
    {
        Ok(child) => child,
        Err(err) => {
            let _ = tx
                .send(AgentEvent::Failed(err.to_string()))
                .await;
            return;
        }
    };

    let _ = tx.send(AgentEvent::Started).await;

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let err_tx = tx.clone();

    let stderr_task = tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();

        while let Ok(Some(line)) = lines.next_line().await {
            if err_tx
                .send(AgentEvent::Activity(format!("stderr: {line}")))
                .await
                .is_err()
            {
                break;
            }
        }
    });

    let mut lines = BufReader::new(stdout).lines();

    loop {
        match lines.next_line().await {
            Ok(Some(line)) => {
                let Ok(json) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };

                let msg = describe_event(&json);

                if tx.send(AgentEvent::Activity(msg)).await.is_err() {
                    break;
                }
            }

            Ok(None) => break,

            Err(err) => {
                let _ = tx
                    .send(AgentEvent::Failed(err.to_string()))
                    .await;
                return;
            }
        }
    }

    let result = child.wait().await;
    let _ = stderr_task.await;

    match result {
        Ok(status) if status.success() => {
            let _ = tx.send(AgentEvent::Completed).await;
        }

        Ok(status) => {
            let _ = tx
                .send(AgentEvent::Failed(format!("Codex exite with status: {status}")))
                .await;
        }

        Err(err) => {
            let _ = tx
                .send(AgentEvent::Failed(err.to_string()))
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

            format!("{event_type} - {description}")
        }

        "error" => {
            let message = json["message"]
                .as_str()
                .unwrap_or("Unknown error");

            format!("Codex error: {message}")
        }

        _ => event_type.to_string(),
    }
}
