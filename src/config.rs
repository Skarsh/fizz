use std::env;

const DEFAULT_MODEL_PROVIDER: &str = "ollama";
const DEFAULT_MODEL: &str = "qwen2.5:3b";
const DEFAULT_MODEL_BASE_URL: &str = "http://localhost:11434";
const DEFAULT_SYSTEM_PROMPT: &str = "You are a helpful assistant.";

#[derive(Debug, Clone)]
pub struct Config {
    pub model_provider: String,
    pub model: String,
    pub model_base_url: String,
    pub system_prompt: String,
}

impl Config {
    pub fn from_env() -> Self {
        let model_base_url =
            env::var("MODEL_BASE_URL").unwrap_or_else(|_| DEFAULT_MODEL_BASE_URL.to_string());

        Self {
            model_provider: env::var("MODEL_PROVIDER")
                .unwrap_or_else(|_| DEFAULT_MODEL_PROVIDER.to_string()),
            model: env::var("MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string()),
            model_base_url,
            system_prompt: env::var("SYSTEM_PROMPT")
                .unwrap_or_else(|_| DEFAULT_SYSTEM_PROMPT.to_string()),
        }
    }
}
