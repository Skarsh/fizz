use std::env;

const DEFAULT_MODEL_PROVIDER: &str = "ollama";
const DEFAULT_MODEL: &str = "qwen2.5:3b";
const DEFAULT_MODEL_BASE_URL: &str = "http://localhost:11434";
const DEFAULT_SYSTEM_PROMPT: &str = "You are a helpful assistant.";
const DEFAULT_MODEL_TIMEOUT_SECS: u64 = 60;

#[derive(Debug, Clone)]
pub struct Config {
    pub model_provider: String,
    pub model: String,
    pub model_base_url: String,
    pub system_prompt: String,
    pub model_timeout_secs: u64,
}

impl Config {
    pub fn from_env() -> Self {
        let model_base_url =
            env::var("MODEL_BASE_URL").unwrap_or_else(|_| DEFAULT_MODEL_BASE_URL.to_string());
        let model_timeout_secs = parse_timeout_secs(env::var("MODEL_TIMEOUT_SECS").ok().as_deref());

        Self {
            model_provider: env::var("MODEL_PROVIDER")
                .unwrap_or_else(|_| DEFAULT_MODEL_PROVIDER.to_string()),
            model: env::var("MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string()),
            model_base_url,
            system_prompt: env::var("SYSTEM_PROMPT")
                .unwrap_or_else(|_| DEFAULT_SYSTEM_PROMPT.to_string()),
            model_timeout_secs,
        }
    }
}

fn parse_timeout_secs(raw: Option<&str>) -> u64 {
    raw.and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_MODEL_TIMEOUT_SECS)
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_MODEL_TIMEOUT_SECS, parse_timeout_secs};

    #[test]
    fn parse_timeout_secs_uses_default_for_missing_or_invalid_values() {
        assert_eq!(parse_timeout_secs(None), DEFAULT_MODEL_TIMEOUT_SECS);
        assert_eq!(parse_timeout_secs(Some("")), DEFAULT_MODEL_TIMEOUT_SECS);
        assert_eq!(
            parse_timeout_secs(Some("not-a-number")),
            DEFAULT_MODEL_TIMEOUT_SECS
        );
        assert_eq!(parse_timeout_secs(Some("0")), DEFAULT_MODEL_TIMEOUT_SECS);
    }

    #[test]
    fn parse_timeout_secs_accepts_positive_integer() {
        assert_eq!(parse_timeout_secs(Some("45")), 45);
        assert_eq!(parse_timeout_secs(Some("  90  ")), 90);
    }
}
