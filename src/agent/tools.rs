use anyhow::{Result, anyhow};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct ToolCall {
    pub name: String,
}

pub fn usage_instructions() -> &'static str {
    "Tools are available.
Available tools:
- time.now: returns current unix time in seconds.
If a tool is needed, reply with exactly: TOOL_CALL:time.now
After receiving tool results, respond normally to the user."
}

pub fn parse_tool_call(text: &str) -> Option<ToolCall> {
    let trimmed = text.trim();
    let name = trimmed.strip_prefix("TOOL_CALL:")?.trim();
    if name.is_empty() {
        return None;
    }
    Some(ToolCall {
        name: name.to_string(),
    })
}

pub fn execute(call: &ToolCall) -> Result<String> {
    match call.name.as_str() {
        "time.now" => {
            let secs = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|err| anyhow!("time.now failed: {err}"))?
                .as_secs();
            Ok(secs.to_string())
        }
        _ => Err(anyhow!("unknown tool '{}'", call.name)),
    }
}

#[cfg(test)]
mod tests {
    use super::{ToolCall, execute, parse_tool_call};

    #[test]
    fn parse_tool_call_reads_name() {
        let call = parse_tool_call("TOOL_CALL:time.now").expect("tool call should parse");
        assert_eq!(call.name, "time.now");
    }

    #[test]
    fn parse_tool_call_rejects_other_text() {
        assert!(parse_tool_call("hello").is_none());
    }

    #[test]
    fn execute_time_now_returns_numeric_string() {
        let output = execute(&ToolCall {
            name: "time.now".to_string(),
        })
        .expect("time.now should work");
        assert!(!output.is_empty());
        assert!(output.chars().all(|ch| ch.is_ascii_digit()));
    }
}
