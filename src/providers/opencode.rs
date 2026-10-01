use std::sync::Arc;

use super::http::{HttpUsageClient, UsageHttp};
use super::{ProviderError, ProviderUsage, UsageProvider};
use crate::config::{OpenCodeConfig, SecretString};

mod export;

const USAGE_URL: &str =
    "https://opencode.ai/console/api/v1/usage/export?scope=organization&range=7d";

#[derive(Debug)]
pub(crate) struct OpenCodeGoProvider {
    api_key: SecretString,
    http: Arc<dyn UsageHttp>,
}

impl OpenCodeGoProvider {
    pub(crate) fn new(config: &OpenCodeConfig) -> Result<Self, ProviderError> {
        if config.api_key.is_empty() {
            return Err(ProviderError::Configuration(
                "OpenCode provider enabled but API key is not configured",
            ));
        }
        Ok(Self {
            api_key: config.api_key.clone(),
            http: Arc::new(HttpUsageClient::csv(USAGE_URL)?),
        })
    }

    #[cfg(test)]
    pub(crate) fn with_http(api_key: SecretString, http: Arc<dyn UsageHttp>) -> Self {
        Self { api_key, http }
    }
}

impl UsageProvider for OpenCodeGoProvider {
    fn id(&self) -> &'static str {
        "opencode-go"
    }

    fn collect(
        &self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<ProviderUsage, ProviderError>> + Send + '_>,
    > {
        Box::pin(async move {
            let body = self.http.get(&self.api_key, None).await?;
            export::normalize(&body)
        })
    }
}
