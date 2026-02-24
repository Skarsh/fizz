use anyhow::anyhow;
use std::error::Error as StdError;
use std::io::ErrorKind;

fn error_chain_has_connection_refused(err: &(dyn StdError + 'static)) -> bool {
    let mut current: Option<&(dyn StdError + 'static)> = Some(err);
    while let Some(source) = current {
        if let Some(io_err) = source.downcast_ref::<std::io::Error>()
            && io_err.kind() == ErrorKind::ConnectionRefused
        {
            return true;
        }

        if source
            .to_string()
            .to_ascii_lowercase()
            .contains("connection refused")
        {
            return true;
        }

        current = source.source();
    }

    false
}

fn error_chain_has_timeout(err: &(dyn StdError + 'static)) -> bool {
    let mut current: Option<&(dyn StdError + 'static)> = Some(err);
    while let Some(source) = current {
        if let Some(io_err) = source.downcast_ref::<std::io::Error>()
            && io_err.kind() == ErrorKind::TimedOut
        {
            return true;
        }

        if source
            .to_string()
            .to_ascii_lowercase()
            .contains("timed out")
        {
            return true;
        }

        current = source.source();
    }

    false
}

pub(crate) fn model_api_request_error(
    err: reqwest::Error,
    api_url: &str,
    timeout_secs: u64,
) -> anyhow::Error {
    if err.is_timeout() || error_chain_has_timeout(&err) {
        return anyhow!(
            "Model request timed out after {}s while calling '{}'. \
             Increase MODEL_TIMEOUT_SECS or check model responsiveness.",
            timeout_secs,
            api_url
        );
    }

    if err.is_connect() {
        if error_chain_has_connection_refused(&err) {
            return anyhow!(
                "Connection refused by model API at '{}'. \
                 Ensure the model provider is running and MODEL_BASE_URL is correct.",
                api_url
            );
        }

        return anyhow!(
            "Failed to connect to model API at '{}'. \
             Check MODEL_BASE_URL and network connectivity.",
            api_url
        );
    }

    anyhow!("Failed to call model API at '{}': {}", api_url, err)
}

#[cfg(test)]
mod tests {
    use super::{error_chain_has_timeout, model_api_request_error};
    use reqwest::Client;
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;

    fn free_local_addr() -> std::net::SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind should succeed");
        let addr = listener.local_addr().expect("address should be available");
        drop(listener);
        addr
    }

    #[tokio::test]
    async fn maps_connection_refused_errors_to_actionable_message() {
        let addr = free_local_addr();
        let api_url = format!("http://{}/api/chat", addr);
        let client = Client::builder()
            .timeout(Duration::from_millis(300))
            .build()
            .expect("client should build");

        let req_err = client
            .post(&api_url)
            .send()
            .await
            .expect_err("request should fail with connection-refused");
        let mapped = model_api_request_error(req_err, &api_url, 1);
        let msg = format!("{mapped:#}");

        assert!(
            msg.contains("Connection refused by model API"),
            "unexpected message: {msg}"
        );
        assert!(msg.contains("MODEL_BASE_URL"), "unexpected message: {msg}");
    }

    #[tokio::test]
    async fn maps_timeout_errors_to_actionable_message() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind should succeed");
        let addr = listener.local_addr().expect("address should be available");
        let server = thread::spawn(move || {
            let (_stream, _) = listener.accept().expect("accept should succeed");
            thread::sleep(Duration::from_secs(1));
        });

        let api_url = format!("http://{}/api/chat", addr);
        let client = Client::builder()
            .timeout(Duration::from_millis(100))
            .build()
            .expect("client should build");

        let req_err = client
            .post(&api_url)
            .send()
            .await
            .expect_err("request should fail with timeout");
        let mapped = model_api_request_error(req_err, &api_url, 2);
        let msg = format!("{mapped:#}");

        assert!(
            msg.contains("Model request timed out after 2s"),
            "unexpected message: {msg}"
        );
        assert!(
            msg.contains("MODEL_TIMEOUT_SECS"),
            "unexpected message: {msg}"
        );

        server.join().expect("server thread should join");
    }

    #[test]
    fn detects_timeout_from_error_kind() {
        let err = std::io::Error::new(std::io::ErrorKind::TimedOut, "timed out");
        assert!(error_chain_has_timeout(&err));
    }
}
