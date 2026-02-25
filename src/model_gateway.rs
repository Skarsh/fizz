use anyhow::Result;
use reqwest::Client;
use std::future::Future;
use std::pin::Pin;

use crate::config::Config;
use crate::model::{self, Message};

pub struct ModelGatewayRequest {
    pub messages: Vec<Message>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelGatewayResponse {
    pub content: String,
}

pub type ModelGatewayFuture<'a> = Pin<Box<dyn Future<Output = Result<ModelGatewayResponse>> + 'a>>;

pub trait ModelGateway {
    fn chat<'a>(&'a self, request: ModelGatewayRequest) -> ModelGatewayFuture<'a>;
}

type ModelChatFuture<'a> = Pin<Box<dyn Future<Output = Result<String>> + 'a>>;

trait ChatBackend {
    fn chat<'a>(
        &'a self,
        client: &'a Client,
        cfg: &'a Config,
        messages: &'a [Message],
    ) -> ModelChatFuture<'a>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ProviderChatBackend;

impl ChatBackend for ProviderChatBackend {
    fn chat<'a>(
        &'a self,
        client: &'a Client,
        cfg: &'a Config,
        messages: &'a [Message],
    ) -> ModelChatFuture<'a> {
        Box::pin(async move { model::chat(client, cfg, messages).await })
    }
}

pub struct HostModelGateway<'a, B = ProviderChatBackend> {
    client: &'a Client,
    cfg: &'a Config,
    backend: B,
}

impl<'a> HostModelGateway<'a, ProviderChatBackend> {
    pub fn new(client: &'a Client, cfg: &'a Config) -> Self {
        Self {
            client,
            cfg,
            backend: ProviderChatBackend,
        }
    }
}

impl<'a, B> HostModelGateway<'a, B> {
    pub fn with_backend(client: &'a Client, cfg: &'a Config, backend: B) -> Self {
        Self {
            client,
            cfg,
            backend,
        }
    }
}

impl<'a, B> ModelGateway for HostModelGateway<'a, B>
where
    B: ChatBackend,
{
    fn chat<'b>(&'b self, request: ModelGatewayRequest) -> ModelGatewayFuture<'b> {
        Box::pin(async move {
            let content = self
                .backend
                .chat(self.client, self.cfg, &request.messages)
                .await?;
            Ok(ModelGatewayResponse { content })
        })
    }
}

#[cfg(test)]
mod tests {
    use anyhow::anyhow;
    use std::cell::RefCell;

    use super::{
        ChatBackend, HostModelGateway, ModelChatFuture, ModelGateway, ModelGatewayRequest,
    };
    use crate::config::{Config, ToolPolicy, ToolResourceLimits, ToolRuntime, WorkspaceFsMode};
    use crate::model::Message;

    #[derive(Debug)]
    enum StubOutcome {
        Ok(String),
        Err(String),
    }

    #[derive(Debug)]
    struct StubBackend {
        calls: RefCell<Vec<Vec<Message>>>,
        outcome: StubOutcome,
    }

    impl StubBackend {
        fn ok(content: impl Into<String>) -> Self {
            Self {
                calls: RefCell::new(Vec::new()),
                outcome: StubOutcome::Ok(content.into()),
            }
        }

        fn err(message: impl Into<String>) -> Self {
            Self {
                calls: RefCell::new(Vec::new()),
                outcome: StubOutcome::Err(message.into()),
            }
        }
    }

    impl ChatBackend for StubBackend {
        fn chat<'a>(
            &'a self,
            _client: &'a reqwest::Client,
            _cfg: &'a Config,
            messages: &'a [Message],
        ) -> ModelChatFuture<'a> {
            self.calls.borrow_mut().push(messages.to_vec());
            let result = match &self.outcome {
                StubOutcome::Ok(content) => Ok(content.clone()),
                StubOutcome::Err(message) => Err(anyhow!(message.clone())),
            };
            Box::pin(async move { result })
        }
    }

    fn test_config() -> Config {
        Config {
            model_provider: "ollama".to_string(),
            model: "qwen2.5:3b".to_string(),
            model_base_url: "http://localhost:11434".to_string(),
            system_prompt: "You are a helpful assistant.".to_string(),
            model_timeout_secs: 60,
            tool_runtime: ToolRuntime::Builtin,
            workspace_fs_mode: WorkspaceFsMode::Host,
            tool_policy: ToolPolicy {
                allow_direct_network: false,
                resource_limits: ToolResourceLimits {
                    timeout_secs: 30,
                    memory_mb: 256,
                },
            },
        }
    }

    #[tokio::test]
    async fn host_gateway_maps_request_messages_and_response_content() {
        let client = reqwest::Client::new();
        let cfg = test_config();
        let gateway = HostModelGateway::with_backend(&client, &cfg, StubBackend::ok("hello"));
        let request_messages = vec![
            Message::system("sys"),
            Message::user("hi"),
            Message::assistant("hello"),
        ];

        let response = gateway
            .chat(ModelGatewayRequest {
                messages: request_messages.clone(),
            })
            .await
            .expect("gateway chat should succeed");

        assert_eq!(response.content, "hello");
        let calls = gateway.backend.calls.borrow();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].len(), request_messages.len());
        assert_eq!(calls[0][0].role.as_str(), "system");
        assert_eq!(calls[0][1].content, "hi");
        assert_eq!(calls[0][2].role.as_str(), "assistant");
    }

    #[tokio::test]
    async fn host_gateway_preserves_backend_errors() {
        let client = reqwest::Client::new();
        let cfg = test_config();
        let gateway =
            HostModelGateway::with_backend(&client, &cfg, StubBackend::err("backend failure"));

        let err = gateway
            .chat(ModelGatewayRequest {
                messages: vec![Message::user("ping")],
            })
            .await
            .expect_err("gateway chat should fail");

        let msg = format!("{err:#}");
        assert!(
            msg.contains("backend failure"),
            "unexpected error message: {msg}"
        );
        assert_eq!(gateway.backend.calls.borrow().len(), 1);
    }
}
