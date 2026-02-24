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
        Self::from_env_with(|key| env::var(key).ok())
    }

    fn from_env_with(mut get_var: impl FnMut(&str) -> Option<String>) -> Self {
        let model_base_url =
            get_var("MODEL_BASE_URL").unwrap_or_else(|| DEFAULT_MODEL_BASE_URL.to_string());
        let model_timeout_secs = parse_timeout_secs(get_var("MODEL_TIMEOUT_SECS").as_deref());

        Self {
            model_provider: get_var("MODEL_PROVIDER")
                .unwrap_or_else(|| DEFAULT_MODEL_PROVIDER.to_string()),
            model: get_var("MODEL").unwrap_or_else(|| DEFAULT_MODEL.to_string()),
            model_base_url,
            system_prompt: get_var("SYSTEM_PROMPT")
                .unwrap_or_else(|| DEFAULT_SYSTEM_PROMPT.to_string()),
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
    use std::collections::HashMap;

    use super::{
        Config, DEFAULT_MODEL, DEFAULT_MODEL_BASE_URL, DEFAULT_MODEL_PROVIDER,
        DEFAULT_MODEL_TIMEOUT_SECS, DEFAULT_SYSTEM_PROMPT, parse_timeout_secs,
    };

    fn config_from_pairs(pairs: &[(&str, &str)]) -> Config {
        let vars: HashMap<String, String> = pairs
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect();
        Config::from_env_with(|key| vars.get(key).cloned())
    }

    #[test]
    fn from_env_uses_defaults_when_vars_are_missing() {
        let cfg = config_from_pairs(&[]);
        assert_eq!(cfg.model_provider, DEFAULT_MODEL_PROVIDER);
        assert_eq!(cfg.model, DEFAULT_MODEL);
        assert_eq!(cfg.model_base_url, DEFAULT_MODEL_BASE_URL);
        assert_eq!(cfg.system_prompt, DEFAULT_SYSTEM_PROMPT);
        assert_eq!(cfg.model_timeout_secs, DEFAULT_MODEL_TIMEOUT_SECS);
    }

    #[test]
    fn from_env_reads_configured_values() {
        let cfg = config_from_pairs(&[
            ("MODEL_PROVIDER", "custom"),
            ("MODEL", "some-model:1"),
            ("MODEL_BASE_URL", "http://localhost:9999"),
            ("SYSTEM_PROMPT", "Be concise."),
            ("MODEL_TIMEOUT_SECS", "15"),
        ]);

        assert_eq!(cfg.model_provider, "custom");
        assert_eq!(cfg.model, "some-model:1");
        assert_eq!(cfg.model_base_url, "http://localhost:9999");
        assert_eq!(cfg.system_prompt, "Be concise.");
        assert_eq!(cfg.model_timeout_secs, 15);
    }

    #[test]
    fn from_env_uses_default_timeout_when_timeout_is_invalid() {
        let cfg = config_from_pairs(&[("MODEL_TIMEOUT_SECS", "0")]);
        assert_eq!(cfg.model_timeout_secs, DEFAULT_MODEL_TIMEOUT_SECS);
    }

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
