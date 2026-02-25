use anyhow::{Result, anyhow};
use reqwest::Client;
use tracing::{debug, warn};

use crate::config::Config;
use crate::providers;

#[derive(Debug, Clone)]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

impl MessageRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
        }
    }
}

pub async fn chat(client: &Client, cfg: &Config, messages: &[Message]) -> Result<String> {
    let provider = cfg.model_provider.to_ascii_lowercase();

    match provider.as_str() {
        "ollama" => {
            debug!(
                provider = "ollama",
                model = %cfg.model,
                message_count = messages.len(),
                "dispatching model chat request"
            );
            providers::ollama::chat(client, cfg, messages).await
        }
        other => {
            warn!(provider = %other, "unsupported model provider configured");
            Err(anyhow!(
                "Unsupported MODEL_PROVIDER='{}'. Supported providers: ollama.",
                other
            ))
        }
    }
}
