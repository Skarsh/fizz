use anyhow::{Context, Result, anyhow};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::model::Message;

#[derive(Debug, Serialize)]
struct OllamaChatRequest {
    model: String,
    stream: bool,
    messages: Vec<ChatMessage>,
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OllamaChatResponse {
    message: ChatMessageResponse,
}

#[derive(Debug, Deserialize)]
struct ChatMessageResponse {
    content: String,
}

fn chat_url(base_url: &str) -> String {
    format!("{}/api/chat", base_url.trim_end_matches('/'))
}

fn to_ollama_messages(messages: &[Message]) -> Vec<ChatMessage> {
    messages
        .iter()
        .map(|msg| ChatMessage {
            role: msg.role.as_str().to_string(),
            content: msg.content.clone(),
        })
        .collect()
}

pub async fn chat(client: &Client, cfg: &Config, messages: &[Message]) -> Result<String> {
    let body = OllamaChatRequest {
        model: cfg.model.clone(),
        stream: false,
        messages: to_ollama_messages(messages),
    };

    let response = client
        .post(chat_url(&cfg.model_base_url))
        .json(&body)
        .send()
        .await
        .context("Failed to call model API")?;

    if !response.status().is_success() {
        let status = response.status();
        let response_body = response
            .text()
            .await
            .unwrap_or_else(|_| "<failed to read response body>".to_string());
        return Err(anyhow!(
            "Model request failed with status {}: {}",
            status,
            response_body
        ));
    }

    let parsed: OllamaChatResponse = response
        .json()
        .await
        .context("Failed to parse model chat response")?;
    Ok(parsed.message.content)
}

#[cfg(test)]
mod tests {
    use super::chat_url;

    #[test]
    fn chat_url_trims_trailing_slash() {
        assert_eq!(
            chat_url("http://localhost:11434/"),
            "http://localhost:11434/api/chat"
        );
    }
}
