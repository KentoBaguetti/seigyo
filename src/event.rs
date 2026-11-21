/// Event
#[derive(Debug, Clone)]
pub enum AgentEvent {
    Started,
    Activity(String),
    Completed,
    Failed(String),
    ThreadStarted(String),
    ReviewReady(String)
}

/// Current state of the agent
#[derive(Debug, Clone)]
pub enum AgentState {
    Idle,
    Running,
    Completed,
    Failed,
    _Cancelled // unused for now
}

#[derive(Debug, Clone, Copy)]
pub enum AgentRole {
    Coder,
    Reviewer,
}

#[derive(Debug, Clone)]
pub struct AgentSession {
    pub state: AgentState,
    pub thread_id: Option<String>,
    pub activity: String,
    pub logs: Vec<String>
}

impl AgentSession {
    pub fn new() -> Self {
        Self {
            state: AgentState::Idle,
            thread_id: None,
            activity: "Waiting for agent...".to_string(),
            logs: Vec::new()
        }
    }

    pub fn handle_event(&mut self, event: AgentEvent) {
        match event {
            AgentEvent::Started => {
                self.state = AgentState::Running;
                self.activity = "Codex started".to_string();
                self.logs.push("Agent started".to_string());
            }

            AgentEvent::Activity(msg) => {
                self.activity = msg.clone();
                self.logs.push(msg);
            }

            AgentEvent::Completed => {
                self.state = AgentState::Completed;
                self.activity = "Agent completed".to_string();
                self.logs.push("Agent completed".to_string());
            }

            AgentEvent::Failed(err) => {
                self.state = AgentState::Failed;
                self.activity = err.clone();
                self.logs.push(format!("ERROR: {err}"));
            }

            AgentEvent::ThreadStarted(id) => {
                self.state = AgentState::Running;
                self.thread_id = Some(id);
            }

            AgentEvent::ReviewReady(_) => {}
        }
        // keep track of the latest 100 logs
        if self.logs.len() > 100 {
            self.logs.remove(0);
        }
    }
}

pub struct App {
    pub coder: AgentSession,
    pub reviewer: AgentSession,
    pub input: String,
    pub previous_input: Option<String>,
    pub latest_review: Option<String>
}

impl App {
    pub fn new() -> Self {
        Self {
            coder: AgentSession::new(),
            reviewer: AgentSession::new(),
            input: "".to_string(),
            previous_input: None,
            latest_review: None
        }
    }
}
