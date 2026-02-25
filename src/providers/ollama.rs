use anyhow::{Context, Result, anyhow};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::config::Config;
use crate::model::Message;
use crate::providers::http_errors::model_api_request_error;

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
    let api_url = chat_url(&cfg.model_base_url);
    let body = OllamaChatRequest {
        model: cfg.model.clone(),
        stream: false,
        messages: to_ollama_messages(messages),
    };
    debug!(
        api_url = %api_url,
        model = %cfg.model,
        message_count = messages.len(),
        "sending ollama chat request"
    );

    let response = client
        .post(&api_url)
        .json(&body)
        .send()
        .await
        .map_err(|err| {
            warn!(
                api_url = %api_url,
                model = %cfg.model,
                error = %err,
                "ollama request failed"
            );
            model_api_request_error(err, &api_url, cfg.model_timeout_secs)
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let response_body = response
            .text()
            .await
            .unwrap_or_else(|_| "<failed to read response body>".to_string());
        warn!(
            api_url = %api_url,
            model = %cfg.model,
            status = %status,
            response_body_len = response_body.len(),
            "ollama returned non-success status"
        );
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
    debug!(
        model = %cfg.model,
        response_len = parsed.message.content.len(),
        "received ollama chat response"
    );
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
