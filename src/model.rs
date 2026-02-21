use anyhow::{Result, anyhow};
use reqwest::Client;

use crate::config::Config;
use crate::providers;

pub async fn chat_once(client: &Client, cfg: &Config, prompt: &str) -> Result<String> {
    match cfg.model_provider.to_ascii_lowercase().as_str() {
        "ollama" => providers::ollama::chat_once(client, cfg, prompt).await,
        other => Err(anyhow!(
            "Unsupported MODEL_PROVIDER='{}'. Supported providers: ollama.",
            other
        )),
    }
}
