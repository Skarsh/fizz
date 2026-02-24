mod tools;

use anyhow::Result;
use reqwest::Client;

use crate::config::Config;
use crate::model::{self, Message};

const MAX_HISTORY_MESSAGES: usize = 40;
const MAX_TOOL_HOPS_PER_TURN: usize = 2;

pub struct Agent<'a> {
    client: &'a Client,
    cfg: &'a Config,
    history: Vec<Message>,
    system_messages: Vec<Message>,
}

impl<'a> Agent<'a> {
    pub fn new(client: &'a Client, cfg: &'a Config) -> Self {
        let system_messages = build_system_messages(cfg);
        let history = system_messages.clone();
        Self {
            client,
            cfg,
            history,
            system_messages,
        }
    }

    pub fn reset(&mut self) {
        self.history = self.system_messages.clone();
    }

    pub fn history(&self) -> &[Message] {
        &self.history
    }

    pub async fn run_turn(&mut self, user_input: &str) -> Result<String> {
        self.history.push(Message::user(user_input));
        self.trim_history();

        let mut tool_hops = 0usize;
        let mut response = model::chat(self.client, self.cfg, &self.history).await?;

        loop {
            let Some(tool_call) = tools::parse_tool_call(&response) else {
                self.history.push(Message::assistant(response.clone()));
                self.trim_history();
                return Ok(response);
            };

            if tool_hops >= MAX_TOOL_HOPS_PER_TURN {
                self.history.push(Message::assistant(response));
                self.trim_history();

                let limit_msg = format!(
                    "I stopped after {} tool calls in one turn. Please try a simpler request.",
                    MAX_TOOL_HOPS_PER_TURN
                );
                self.history.push(Message::assistant(limit_msg.clone()));
                self.trim_history();
                return Ok(limit_msg);
            }

            tool_hops += 1;
            self.history.push(Message::assistant(response));
            self.trim_history();

            let tool_result = match tools::execute(&tool_call) {
                Ok(output) => output,
                Err(err) => format!("ERROR: {err}"),
            };
            self.history.push(Message::user(format!(
                "Tool '{}' result: {}",
                tool_call.name, tool_result
            )));
            self.trim_history();

            response = model::chat(self.client, self.cfg, &self.history).await?;
        }
    }

    fn trim_history(&mut self) {
        if self.history.len() <= MAX_HISTORY_MESSAGES {
            return;
        }

        let keep_tail = MAX_HISTORY_MESSAGES.saturating_sub(self.system_messages.len());
        let mut trimmed = self.system_messages.clone();
        let tail_start = self.history.len().saturating_sub(keep_tail);
        trimmed.extend_from_slice(&self.history[tail_start..]);
        self.history = trimmed;
    }
}

fn build_system_messages(cfg: &Config) -> Vec<Message> {
    let mut messages = Vec::new();

    if !cfg.system_prompt.trim().is_empty() {
        messages.push(Message::system(cfg.system_prompt.clone()));
    }

    messages.push(Message::system(tools::usage_instructions()));
    messages
}
