mod tools;

use anyhow::Result;
use reqwest::Client;

use crate::config::Config;
use crate::model::{self, Message, MessageRole};

const MAX_HISTORY_MESSAGES: usize = 40;
const MAX_TOOL_HOPS_PER_TURN: usize = 2;
const TOOL_RESULT_USER_PREFIX: &str = "Tool '";

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
            self.history
                .push(Message::user(format_tool_result_user_message(
                    &tool_call.name,
                    &tool_result,
                )));
            self.trim_history();

            response = model::chat(self.client, self.cfg, &self.history).await?;
        }
    }

    fn trim_history(&mut self) {
        trim_history_messages(&mut self.history, &self.system_messages);
    }
}

fn format_tool_result_user_message(tool_name: &str, tool_result: &str) -> String {
    format!("Tool '{}' result: {}", tool_name, tool_result)
}

fn is_internal_tool_result_user_message(msg: &Message) -> bool {
    matches!(&msg.role, MessageRole::User)
        && msg.content.starts_with(TOOL_RESULT_USER_PREFIX)
        && msg.content.contains("' result:")
}

fn is_user_turn_start(msg: &Message) -> bool {
    matches!(&msg.role, MessageRole::User) && !is_internal_tool_result_user_message(msg)
}

fn trim_history_messages(history: &mut Vec<Message>, system_messages: &[Message]) {
    if history.len() <= MAX_HISTORY_MESSAGES {
        return;
    }

    let system_len = system_messages.len();
    let keep_tail = MAX_HISTORY_MESSAGES.saturating_sub(system_len);
    let min_start = history.len().saturating_sub(keep_tail).max(system_len);
    let aligned_start = (min_start..history.len()).find(|&idx| is_user_turn_start(&history[idx]));

    let mut trimmed = system_messages.to_vec();
    if let Some(start) = aligned_start {
        trimmed.extend_from_slice(&history[start..]);
    }
    *history = trimmed;
}

fn build_system_messages(cfg: &Config) -> Vec<Message> {
    let mut messages = Vec::new();

    if !cfg.system_prompt.trim().is_empty() {
        messages.push(Message::system(cfg.system_prompt.clone()));
    }

    messages.push(Message::system(tools::usage_instructions()));
    messages
}

#[cfg(test)]
mod tests {
    use super::{MAX_HISTORY_MESSAGES, trim_history_messages};
    use crate::model::Message;

    #[test]
    fn trim_history_preserves_turn_boundaries() {
        let system_messages = vec![Message::system("sys"), Message::system("tools")];
        let mut history = system_messages.clone();

        for i in 0..25 {
            history.push(Message::user(format!("user-{i}")));
            history.push(Message::assistant(format!("assistant-{i}")));
        }

        trim_history_messages(&mut history, &system_messages);

        assert!(history.len() <= MAX_HISTORY_MESSAGES);
        assert_eq!(history[0].content, "sys");
        assert_eq!(history[1].content, "tools");
        assert_eq!(history[2].role.as_str(), "user");
    }

    #[test]
    fn trim_history_skips_tool_result_messages_as_turn_starts() {
        let system_messages = vec![Message::system("sys"), Message::system("tools")];
        let mut history = system_messages.clone();

        history.push(Message::user("q0"));
        history.push(Message::assistant(r#"{"tool_call":{"name":"time.now"}}"#));
        history.push(Message::user("Tool 'time.now' result: one"));
        history.push(Message::assistant(r#"{"tool_call":{"name":"time.now"}}"#));
        history.push(Message::user("Tool 'time.now' result: two"));
        history.push(Message::assistant("done"));

        for i in 1..=17 {
            history.push(Message::user(format!("q{i}")));
            history.push(Message::assistant(format!("a{i}")));
        }

        trim_history_messages(&mut history, &system_messages);

        assert!(history.len() <= MAX_HISTORY_MESSAGES);
        assert_eq!(history[2].role.as_str(), "user");
        assert_eq!(history[2].content, "q1");
    }

    #[test]
    fn trim_history_drops_non_system_when_no_complete_turn_fits() {
        let system_messages = vec![Message::system("sys"), Message::system("tools")];
        let mut history = system_messages.clone();

        history.push(Message::user("q0"));
        for i in 0..25 {
            history.push(Message::assistant(r#"{"tool_call":{"name":"time.now"}}"#));
            history.push(Message::user(format!("Tool 'time.now' result: {i}")));
        }

        trim_history_messages(&mut history, &system_messages);

        assert_eq!(history.len(), system_messages.len());
        assert_eq!(history[0].content, "sys");
        assert_eq!(history[1].content, "tools");
    }
}
