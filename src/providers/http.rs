use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use crate::config::SecretString;

use super::ProviderError;

const MAX_CSV_BYTES: usize = 16 * 1024 * 1024;

pub(crate) trait UsageHttp: fmt::Debug + Send + Sync {
    fn get<'a>(
        &'a self,
        token: &'a SecretString,
        account_id: Option<&'a str>,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, ProviderError>> + Send + 'a>>;
}

#[derive(Debug)]
pub(crate) struct HttpUsageClient {
    client: reqwest::Client,
    url: String,
    accept_csv: bool,
}

impl HttpUsageClient {
    pub(crate) fn new(url: &str) -> Result<Self, ProviderError> {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|_| ProviderError::Request)?;
        Ok(Self {
            client,
            url: url.to_owned(),
            accept_csv: false,
        })
    }

    pub(crate) fn csv(url: &str) -> Result<Self, ProviderError> {
        let mut client = Self::new(url)?;
        client.accept_csv = true;
        Ok(client)
    }

    fn request(
        &self,
        token: &SecretString,
        account_id: Option<&str>,
    ) -> Result<reqwest::Request, ProviderError> {
        let mut request = self.client.get(&self.url).bearer_auth(token.expose());
        if self.accept_csv {
            request = request.header(reqwest::header::ACCEPT, "text/csv");
        }
        if let Some(account_id) = account_id {
            request = request.header("ChatGPT-Account-Id", account_id);
        }
        request.build().map_err(|_| ProviderError::Request)
    }
}

impl UsageHttp for HttpUsageClient {
    fn get<'a>(
        &'a self,
        token: &'a SecretString,
        account_id: Option<&'a str>,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            let mut response = self
                .client
                .execute(self.request(token, account_id)?)
                .await
                .map_err(|_| ProviderError::Request)?;
            if !response.status().is_success() {
                return Err(ProviderError::Status(response.status()));
            }
            if self.accept_csv {
                let mut body = Vec::new();
                while let Some(chunk) =
                    response.chunk().await.map_err(|_| ProviderError::Request)?
                {
                    if chunk.len() > MAX_CSV_BYTES - body.len() {
                        return Err(ProviderError::InvalidData);
                    }
                    body.extend_from_slice(&chunk);
                }
                return Ok(body);
            }
            response
                .bytes()
                .await
                .map(|bytes| bytes.to_vec())
                .map_err(|_| ProviderError::Request)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_uses_bearer_auth_and_optional_account_id() {
        let codex =
            HttpUsageClient::new("https://chatgpt.com/backend-api/wham/usage").expect("client");
        let token = SecretString::new("test-secret");
        let request = codex.request(&token, Some("workspace-1")).expect("request");
        assert_eq!(request.method(), reqwest::Method::GET);
        assert_eq!(request.headers()["authorization"], "Bearer test-secret");
        assert_eq!(request.headers()["chatgpt-account-id"], "workspace-1");
        let go = HttpUsageClient::csv(
            "https://opencode.ai/console/api/v1/usage/export?scope=organization&range=7d",
        )
        .expect("client");
        let request = go.request(&token, None).expect("request");
        assert!(request.headers().get("chatgpt-account-id").is_none());
        assert_eq!(request.headers()[reqwest::header::ACCEPT], "text/csv");
        assert!(!format!("{codex:?}").contains("test-secret"));
    }
}
