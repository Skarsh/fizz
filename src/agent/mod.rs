mod tools;

use anyhow::Result;
use reqwest::Client;
use std::future::Future;
use std::pin::Pin;
use tracing::{debug, info, warn};

use crate::config::Config;
use crate::model::{self, Message};

const MAX_HISTORY_MESSAGES: usize = 40;
const MAX_TOOL_HOPS_PER_TURN: usize = 2;

type ModelFuture = Pin<Box<dyn Future<Output = Result<String>>>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HistoryMessageKind {
    System,
    UserInput,
    ToolResult,
    Assistant,
}

struct TurnState {
    history: Vec<Message>,
    history_kinds: Vec<HistoryMessageKind>,
    system_len: usize,
}

impl TurnState {
    fn new(cfg: &Config) -> Self {
        Self::from_system_messages(build_system_messages(cfg))
    }

    fn from_system_messages(system_messages: Vec<Message>) -> Self {
        let system_len = system_messages.len();
        let history = system_messages;
        let history_kinds = vec![HistoryMessageKind::System; system_len];
        Self {
            history,
            history_kinds,
            system_len,
        }
    }

    fn reset(&mut self) {
        self.history.truncate(self.system_len);
        self.history_kinds.truncate(self.system_len);
    }

    fn history(&self) -> &[Message] {
        &self.history
    }

    fn push_user_input(&mut self, content: impl Into<String>) {
        self.push_message(Message::user(content), HistoryMessageKind::UserInput);
    }

    fn push_tool_result(&mut self, tool_name: &str, tool_result: &str) {
        self.push_message(
            Message::user(format_tool_result_user_message(tool_name, tool_result)),
            HistoryMessageKind::ToolResult,
        );
    }

    fn push_assistant(&mut self, content: impl Into<String>) {
        self.push_message(Message::assistant(content), HistoryMessageKind::Assistant);
    }

    fn push_message(&mut self, message: Message, kind: HistoryMessageKind) {
        self.history.push(message);
        self.history_kinds.push(kind);
        self.trim_history();
    }

    fn trim_history(&mut self) {
        trim_history_messages(&mut self.history, &mut self.history_kinds, self.system_len);
    }
}

struct TurnEngine {
    state: TurnState,
}

impl TurnEngine {
    fn new(cfg: &Config) -> Self {
        Self {
            state: TurnState::new(cfg),
        }
    }

    fn reset(&mut self) {
        self.state.reset();
    }

    fn history(&self) -> &[Message] {
        self.state.history()
    }

    async fn run_turn_live(
        &mut self,
        user_input: &str,
        client: &Client,
        cfg: &Config,
    ) -> Result<String> {
        let client = client.clone();
        let cfg = cfg.clone();

        self.run_turn_with(
            user_input,
            move |messages| {
                let client = client.clone();
                let cfg = cfg.clone();
                Box::pin(async move { model::chat(&client, &cfg, &messages).await })
            },
            tools::execute,
        )
        .await
    }

    async fn run_turn_with<C, E>(
        &mut self,
        user_input: &str,
        mut chat: C,
        mut execute_tool: E,
    ) -> Result<String>
    where
        C: FnMut(Vec<Message>) -> ModelFuture,
        E: FnMut(&tools::ToolCall) -> Result<String>,
    {
        self.state.push_user_input(user_input);
        debug!(
            user_input_len = user_input.len(),
            history_len = self.state.history().len(),
            "started turn"
        );

        let mut tool_hops = 0usize;
        let mut response = chat(self.state.history().to_vec()).await?;

        loop {
            let Some(tool_call) = tools::parse_tool_call(&response) else {
                self.state.push_assistant(response.clone());
                info!(
                    tool_hops,
                    response_len = response.len(),
                    history_len = self.state.history().len(),
                    "completed turn"
                );
                return Ok(response);
            };

            if tool_hops >= MAX_TOOL_HOPS_PER_TURN {
                warn!(
                    max_tool_hops = MAX_TOOL_HOPS_PER_TURN,
                    tool_hops, "tool hop limit reached"
                );
                let limit_msg = format!(
                    "I stopped after {} tool calls in one turn. Please try a simpler request.",
                    MAX_TOOL_HOPS_PER_TURN
                );
                self.state.push_assistant(limit_msg.clone());
                return Ok(limit_msg);
            }

            tool_hops += 1;
            info!(tool_name = %tool_call.name, tool_hop = tool_hops, "executing tool call");
            self.state.push_assistant(response);

            let tool_result = match execute_tool(&tool_call) {
                Ok(output) => {
                    debug!(
                        tool_name = %tool_call.name,
                        output_len = output.len(),
                        "tool call succeeded"
                    );
                    output
                }
                Err(err) => {
                    warn!(tool_name = %tool_call.name, error = %err, "tool call failed");
                    format!("ERROR: {err}")
                }
            };
            self.state.push_tool_result(&tool_call.name, &tool_result);
            debug!(
                history_len = self.state.history().len(),
                "requesting follow-up model response"
            );

            response = chat(self.state.history().to_vec()).await?;
        }
    }
}

pub struct Agent<'a> {
    client: &'a Client,
    cfg: &'a Config,
    turn_engine: TurnEngine,
}

impl<'a> Agent<'a> {
    pub fn new(client: &'a Client, cfg: &'a Config) -> Self {
        Self {
            client,
            cfg,
            turn_engine: TurnEngine::new(cfg),
        }
    }

    pub fn reset(&mut self) {
        self.turn_engine.reset();
    }

    pub fn history(&self) -> &[Message] {
        self.turn_engine.history()
    }

    pub async fn run_turn(&mut self, user_input: &str) -> Result<String> {
        self.turn_engine
            .run_turn_live(user_input, self.client, self.cfg)
            .await
    }
}

fn format_tool_result_user_message(tool_name: &str, tool_result: &str) -> String {
    format!("Tool '{}' result: {}", tool_name, tool_result)
}

fn is_user_turn_start(kind: HistoryMessageKind) -> bool {
    matches!(kind, HistoryMessageKind::UserInput)
}

fn trim_history_messages(
    history: &mut Vec<Message>,
    history_kinds: &mut Vec<HistoryMessageKind>,
    system_len: usize,
) {
    debug_assert_eq!(history.len(), history_kinds.len());

    if history.len() <= MAX_HISTORY_MESSAGES {
        return;
    }

    let keep_tail = MAX_HISTORY_MESSAGES.saturating_sub(system_len);
    let min_start = history.len().saturating_sub(keep_tail).max(system_len);
    let aligned_start =
        (min_start..history.len()).find(|&idx| is_user_turn_start(history_kinds[idx]));

    let mut trimmed_history = history[..system_len].to_vec();
    let mut trimmed_kinds = history_kinds[..system_len].to_vec();

    if let Some(start) = aligned_start {
        trimmed_history.extend_from_slice(&history[start..]);
        trimmed_kinds.extend_from_slice(&history_kinds[start..]);
    }

    *history = trimmed_history;
    *history_kinds = trimmed_kinds;
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
    use std::cell::RefCell;
    use std::collections::VecDeque;

    use super::{
        HistoryMessageKind, MAX_HISTORY_MESSAGES, MAX_TOOL_HOPS_PER_TURN, ModelFuture, TurnEngine,
        TurnState,
    };
    use crate::model::Message;

    struct StubModel {
        responses: VecDeque<String>,
        call_count: usize,
    }

    impl StubModel {
        fn new(responses: Vec<&str>) -> Self {
            Self {
                responses: responses.into_iter().map(str::to_string).collect(),
                call_count: 0,
            }
        }

        fn chat(&mut self, _messages: Vec<Message>) -> ModelFuture {
            self.call_count += 1;
            let response = self
                .responses
                .pop_front()
                .expect("stub model missing queued response");
            Box::pin(async move { Ok(response) })
        }
    }

    fn test_system_messages() -> Vec<Message> {
        vec![Message::system("sys"), Message::system("tools")]
    }

    fn test_state() -> TurnState {
        TurnState::from_system_messages(test_system_messages())
    }

    fn test_engine() -> TurnEngine {
        TurnEngine {
            state: test_state(),
        }
    }

    #[test]
    fn trim_history_preserves_turn_boundaries() {
        let mut state = test_state();

        for i in 0..25 {
            state.push_user_input(format!("user-{i}"));
            state.push_assistant(format!("assistant-{i}"));
        }

        assert!(state.history.len() <= MAX_HISTORY_MESSAGES);
        assert_eq!(state.history[0].content, "sys");
        assert_eq!(state.history[1].content, "tools");
        assert_eq!(state.history_kinds[2], HistoryMessageKind::UserInput);
    }

    #[test]
    fn trim_history_skips_tool_result_messages_as_turn_starts() {
        let mut state = test_state();

        state.push_user_input("q0");
        state.push_assistant(r#"{"tool_call":{"name":"time.now"}}"#);
        state.push_tool_result("time.now", "one");
        state.push_assistant(r#"{"tool_call":{"name":"time.now"}}"#);
        state.push_tool_result("time.now", "two");
        state.push_assistant("done");

        for i in 1..=17 {
            state.push_user_input(format!("q{i}"));
            state.push_assistant(format!("a{i}"));
        }

        assert!(state.history.len() <= MAX_HISTORY_MESSAGES);
        assert_eq!(state.history_kinds[2], HistoryMessageKind::UserInput);
        assert_eq!(state.history[2].content, "q1");
    }

    #[test]
    fn trim_history_drops_non_system_when_no_complete_turn_fits() {
        let mut state = test_state();

        state.push_user_input("q0");
        for i in 0..25 {
            state.push_assistant(r#"{"tool_call":{"name":"time.now"}}"#);
            state.push_tool_result("time.now", &i.to_string());
        }

        state.trim_history();

        assert!(state.history.len() <= MAX_HISTORY_MESSAGES);
        assert_eq!(state.history[0].content, "sys");
        assert_eq!(state.history[1].content, "tools");
        assert!(
            state.history_kinds[state.system_len..]
                .iter()
                .all(|kind| *kind != HistoryMessageKind::UserInput)
        );
    }

    #[tokio::test]
    async fn turn_engine_handles_plain_assistant_reply() {
        let mut engine = test_engine();
        let mut model = StubModel::new(vec!["plain answer"]);
        let tool_calls = RefCell::new(Vec::<String>::new());

        let answer = engine
            .run_turn_with(
                "hello",
                |messages| model.chat(messages),
                |call| {
                    tool_calls.borrow_mut().push(call.name.clone());
                    Ok(format!("stub-result-for-{}", call.name))
                },
            )
            .await
            .expect("turn should succeed");

        assert_eq!(answer, "plain answer");
        assert_eq!(model.call_count, 1);
        assert!(tool_calls.borrow().is_empty());
        assert_eq!(
            engine
                .history()
                .last()
                .expect("history should have reply")
                .content,
            answer
        );
    }

    #[tokio::test]
    async fn turn_engine_runs_single_tool_call_then_returns_final_answer() {
        let mut engine = test_engine();
        let mut model = StubModel::new(vec![
            r#"{"tool_call":{"name":"time.now"}}"#,
            "Here is the final answer.",
        ]);
        let tool_calls = RefCell::new(Vec::<String>::new());

        let answer = engine
            .run_turn_with(
                "what time?",
                |messages| model.chat(messages),
                |call| {
                    tool_calls.borrow_mut().push(call.name.clone());
                    Ok(format!("stub-result-for-{}", call.name))
                },
            )
            .await
            .expect("turn should succeed");

        assert_eq!(answer, "Here is the final answer.");
        assert_eq!(model.call_count, 2);
        assert_eq!(tool_calls.borrow().as_slice(), &["time.now".to_string()]);
        assert!(
            engine
                .history()
                .iter()
                .any(|msg| msg.content.starts_with("Tool 'time.now' result:")),
            "tool result should be recorded in history"
        );
    }

    #[tokio::test]
    async fn turn_engine_treats_malformed_tool_output_as_normal_reply() {
        let mut engine = test_engine();
        let mixed_output = "Let me check.\n{\"tool_call\":{\"name\":\"time.now\"}}";
        let mut model = StubModel::new(vec![mixed_output]);
        let tool_calls = RefCell::new(Vec::<String>::new());

        let answer = engine
            .run_turn_with(
                "what time now?",
                |messages| model.chat(messages),
                |call| {
                    tool_calls.borrow_mut().push(call.name.clone());
                    Ok(format!("stub-result-for-{}", call.name))
                },
            )
            .await
            .expect("turn should succeed");

        assert_eq!(answer, mixed_output);
        assert_eq!(model.call_count, 1);
        assert!(tool_calls.borrow().is_empty());
    }

    #[tokio::test]
    async fn turn_engine_stops_when_tool_hop_limit_is_reached() {
        let mut engine = test_engine();
        let mut model = StubModel::new(vec![
            r#"{"tool_call":{"name":"time.now"}}"#,
            r#"{"tool_call":{"name":"time.now"}}"#,
            r#"{"tool_call":{"name":"time.now"}}"#,
        ]);
        let tool_calls = RefCell::new(Vec::<String>::new());

        let answer = engine
            .run_turn_with(
                "keep checking",
                |messages| model.chat(messages),
                |call| {
                    tool_calls.borrow_mut().push(call.name.clone());
                    Ok(format!("stub-result-for-{}", call.name))
                },
            )
            .await
            .expect("turn should succeed");

        assert!(
            answer.contains(&format!(
                "I stopped after {} tool calls",
                MAX_TOOL_HOPS_PER_TURN
            )),
            "unexpected limit message: {answer}"
        );
        assert_eq!(model.call_count, MAX_TOOL_HOPS_PER_TURN + 1);
        assert_eq!(tool_calls.borrow().len(), MAX_TOOL_HOPS_PER_TURN);
        assert_eq!(
            engine
                .history()
                .last()
                .expect("history should include limit message")
                .content,
            answer
        );
    }
}
