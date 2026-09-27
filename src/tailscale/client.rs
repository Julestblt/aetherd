use std::fmt;
use std::future::Future;
use std::time::Duration;

use reqwest::StatusCode;
use time::OffsetDateTime;

use super::model::{DevicesResponse, TailscaleSnapshot};
use crate::config::SecretString;

const DEFAULT_BASE_URL: &str = "https://api.tailscale.com";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Failures raised while talking to the Tailscale API.
///
/// Messages never include request headers, so an authentication failure cannot
/// leak the API key.
#[derive(Debug, thiserror::Error)]
pub(crate) enum TailscaleError {
    /// The HTTP request could not be completed.
    #[error("tailscale request failed: {0}")]
    Request(#[source] reqwest::Error),
    /// The API answered with a non-success status.
    #[error("tailscale API returned HTTP {0}")]
    Status(StatusCode),
    /// The API answered with a body that does not match the documented schema.
    #[error("tailscale API returned an invalid payload: {0}")]
    Decode(#[source] serde_json::Error),
}

/// Source of tailnet devices, injected so tests never reach the real API.
pub(crate) trait DevicesClient: fmt::Debug + Send + Sync {
    /// Fetches the current devices for `tailnet`.
    fn fetch(
        &self,
        tailnet: &str,
    ) -> impl Future<Output = Result<TailscaleSnapshot, TailscaleError>> + Send;
}

/// Tailscale HTTP API client authenticated with a server-side API key.
#[derive(Clone, Debug)]
pub(crate) struct HttpTailscaleClient {
    http: reqwest::Client,
    api_key: SecretString,
    base_url: String,
}

impl HttpTailscaleClient {
    pub(crate) fn new(api_key: SecretString) -> Result<Self, TailscaleError> {
        Self::with_base_url(api_key, DEFAULT_BASE_URL.to_owned())
    }

    pub(crate) fn with_base_url(
        api_key: SecretString,
        base_url: String,
    ) -> Result<Self, TailscaleError> {
        let http = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(TailscaleError::Request)?;
        Ok(Self {
            http,
            api_key,
            base_url,
        })
    }

    fn devices_url(&self, tailnet: &str) -> String {
        format!("{}/api/v2/tailnet/{tailnet}/devices", self.base_url)
    }
}

impl DevicesClient for HttpTailscaleClient {
    async fn fetch(&self, tailnet: &str) -> Result<TailscaleSnapshot, TailscaleError> {
        let response = self
            .http
            .get(self.devices_url(tailnet))
            .bearer_auth(self.api_key.expose())
            .send()
            .await
            .map_err(TailscaleError::Request)?;

        let status = response.status();
        if !status.is_success() {
            return Err(TailscaleError::Status(status));
        }

        let body = response.bytes().await.map_err(TailscaleError::Request)?;
        let payload: DevicesResponse =
            serde_json::from_slice(&body).map_err(TailscaleError::Decode)?;
        Ok(payload.into_snapshot(tailnet, OffsetDateTime::now_utc()))
    }
}

#[cfg(test)]
mod tests {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::task::JoinHandle;

    use super::*;

    const DEVICES: &str = r#"{"devices":[{"id":"node-1","hostname":"homelab","name":"homelab.example.ts.net","os":"linux","addresses":["100.64.0.1"],"authorized":true,"tags":["tag:homelab"]}]}"#;

    async fn mock_server(status: &str, body: &str) -> (String, JoinHandle<String>) {
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("binds");
        let address = listener.local_addr().expect("local address");
        let handle = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accepts");
            let mut request = Vec::new();
            let mut chunk = [0_u8; 1024];
            loop {
                let read = socket.read(&mut chunk).await.expect("reads");
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..read]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            socket.write_all(response.as_bytes()).await.expect("writes");
            socket.flush().await.expect("flushes");
            String::from_utf8_lossy(&request).into_owned()
        });
        (format!("http://{address}"), handle)
    }

    async fn fetch(
        status: &str,
        body: &str,
    ) -> (Result<TailscaleSnapshot, TailscaleError>, String) {
        let (base_url, request) = mock_server(status, body).await;
        let client =
            HttpTailscaleClient::with_base_url(SecretString::new("tskey-secret"), base_url)
                .expect("client builds");
        let result = client.fetch("example.ts.net").await;
        (result, request.await.expect("server task joins"))
    }

    #[tokio::test]
    async fn sends_a_bearer_token_and_returns_typed_devices() {
        let (result, request) = fetch("200 OK", DEVICES).await;
        let snapshot = result.expect("fetch succeeds");

        assert_eq!(snapshot.devices.len(), 1);
        assert_eq!(snapshot.devices[0].hostname, "homelab");
        assert_eq!(snapshot.devices[0].dns_name, "homelab.example.ts.net");
        assert_eq!(snapshot.devices[0].addresses, vec!["100.64.0.1"]);

        let request = request.to_lowercase();
        assert!(request.contains("/api/v2/tailnet/example.ts.net/devices"));
        assert!(request.contains("authorization: bearer tskey-secret"));
    }

    #[tokio::test]
    async fn maps_a_non_success_status_without_leaking_the_key() {
        let (result, _) = fetch("401 Unauthorized", "{}").await;
        let error = result.expect_err("401 is an error");

        assert!(matches!(
            error,
            TailscaleError::Status(StatusCode::UNAUTHORIZED)
        ));
        assert!(error.to_string().contains("401"));
        assert!(!error.to_string().contains("tskey-secret"));
    }

    #[tokio::test]
    async fn maps_a_malformed_payload_to_a_decode_error() {
        let (result, _) = fetch("200 OK", "{\"devices\":[}").await;

        assert!(matches!(
            result.expect_err("malformed payload is an error"),
            TailscaleError::Decode(_)
        ));
    }

    #[test]
    fn debug_output_never_contains_the_api_key() {
        let client =
            HttpTailscaleClient::new(SecretString::new("tskey-secret")).expect("client builds");

        assert!(!format!("{client:?}").contains("tskey-secret"));
    }
}
