use chrono::{DateTime, SecondsFormat, Utc};
use serde::Deserialize;
use std::error::Error;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, warn};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCall {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolOutput {
    pub content: String,
}

impl ToolOutput {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolExecutionError {
    message: String,
}

impl ToolExecutionError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ToolExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for ToolExecutionError {}

pub type ToolExecutionResult = std::result::Result<ToolOutput, ToolExecutionError>;
pub type ToolFuture<'a> = Pin<Box<dyn Future<Output = ToolExecutionResult> + 'a>>;

pub trait ToolRunner {
    fn execute<'a>(&'a self, call: &'a ToolCall) -> ToolFuture<'a>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct BuiltinRunner;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolCallEnvelope {
    tool_call: ToolCallPayload,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolCallPayload {
    name: String,
}

pub fn usage_instructions() -> &'static str {
    "Tools are available.
Available tools:
- time.now: returns current UTC time and unix time in seconds.
If a tool is needed, reply with exactly this JSON object and nothing else:
{\"tool_call\":{\"name\":\"time.now\"}}
After receiving tool results, respond normally to the user."
}

pub fn parse_tool_call(text: &str) -> Option<ToolCall> {
    let parsed: ToolCallEnvelope = serde_json::from_str(text.trim()).ok()?;
    let name = parsed.tool_call.name.trim();
    if name.is_empty() {
        return None;
    }
    Some(ToolCall {
        name: name.to_string(),
    })
}

impl ToolRunner for BuiltinRunner {
    fn execute<'a>(&'a self, call: &'a ToolCall) -> ToolFuture<'a> {
        Box::pin(async move {
            debug!(tool_name = %call.name, "running built-in tool");

            match call.name.as_str() {
                "time.now" => {
                    let now = SystemTime::now();
                    let secs = now
                        .duration_since(UNIX_EPOCH)
                        .map_err(|err| ToolExecutionError::new(format!("time.now failed: {err}")))?
                        .as_secs();
                    let timestamp =
                        DateTime::<Utc>::from(now).to_rfc3339_opts(SecondsFormat::Secs, true);
                    Ok(ToolOutput::new(format!("{timestamp} (unix: {secs})")))
                }
                _ => {
                    warn!(tool_name = %call.name, "unknown built-in tool");
                    Err(ToolExecutionError::new(format!(
                        "unknown tool '{}'",
                        call.name
                    )))
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{BuiltinRunner, ToolCall, ToolRunner, parse_tool_call};

    #[test]
    fn parse_tool_call_reads_name() {
        let call = parse_tool_call(r#"{"tool_call":{"name":"time.now"}}"#)
            .expect("tool call should parse");
        assert_eq!(call.name, "time.now");
    }

    #[test]
    fn parse_tool_call_rejects_other_text() {
        assert!(parse_tool_call("hello").is_none());
    }

    #[test]
    fn parse_tool_call_rejects_legacy_format() {
        assert!(parse_tool_call("TOOL_CALL:time.now").is_none());
    }

    #[test]
    fn parse_tool_call_rejects_invalid_json() {
        assert!(parse_tool_call(r#"{"tool_call":{"name":}}"#).is_none());
    }

    #[test]
    fn parse_tool_call_rejects_unknown_shape() {
        assert!(parse_tool_call(r#"{"name":"time.now"}"#).is_none());
    }

    #[test]
    fn parse_tool_call_rejects_empty_name() {
        assert!(parse_tool_call(r#"{"tool_call":{"name":"   "}}"#).is_none());
    }

    #[tokio::test]
    async fn execute_time_now_returns_readable_and_unix() {
        let output = BuiltinRunner
            .execute(&ToolCall {
                name: "time.now".to_string(),
            })
            .await
            .expect("time.now should work")
            .content;
        assert!(output.contains("T"));
        assert!(output.contains("Z"));
        assert!(output.contains("(unix: "));
        assert!(output.ends_with(')'));
    }

    #[tokio::test]
    async fn execute_unknown_tool_returns_error() {
        let result = BuiltinRunner
            .execute(&ToolCall {
                name: "missing.tool".to_string(),
            })
            .await;
        assert!(result.is_err());
    }
}
