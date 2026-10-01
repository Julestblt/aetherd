use std::path::PathBuf;
use std::sync::Arc;

use serde::Deserialize;
use time::OffsetDateTime;

use super::auth::{read_codex_auth, resolve_codex_auth};
use super::http::{HttpUsageClient, UsageHttp};
use super::{ProviderError, ProviderUsage, UsageProvider, UsageWindow, WindowKind};
use crate::config::CodexConfig;

const USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";

#[derive(Debug)]
pub(crate) struct CodexProvider {
    auth_file: PathBuf,
    http: Arc<dyn UsageHttp>,
}

#[derive(Debug, Deserialize)]
struct Response {
    rate_limit: Option<RateLimit>,
}

#[derive(Debug, Deserialize)]
struct RateLimit {
    primary_window: Option<Window>,
    secondary_window: Option<Window>,
}

#[derive(Debug, Deserialize)]
struct Window {
    used_percent: f64,
    limit_window_seconds: u64,
    reset_at: i64,
}

impl CodexProvider {
    pub(crate) fn new(config: &CodexConfig) -> Result<Self, ProviderError> {
        let auth_file = resolve_codex_auth(
            config.auth_file.as_deref(),
            std::env::var_os("CODEX_HOME")
                .as_deref()
                .map(std::path::Path::new),
            std::env::var_os("HOME")
                .as_deref()
                .map(std::path::Path::new),
        )?;
        Ok(Self {
            auth_file,
            http: Arc::new(HttpUsageClient::new(USAGE_URL)?),
        })
    }

    #[cfg(test)]
    pub(crate) fn with_http(auth_file: PathBuf, http: Arc<dyn UsageHttp>) -> Self {
        Self { auth_file, http }
    }
}

impl UsageProvider for CodexProvider {
    fn id(&self) -> &'static str {
        "codex"
    }

    fn collect(
        &self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<ProviderUsage, ProviderError>> + Send + '_>,
    > {
        Box::pin(async move {
            let auth = read_codex_auth(&self.auth_file)?;
            let body = self
                .http
                .get(&auth.access_token, auth.account_id.as_deref())
                .await?;
            let payload: Response =
                serde_json::from_slice(&body).map_err(|_| ProviderError::InvalidData)?;
            normalize(payload)
        })
    }
}

fn normalize(payload: Response) -> Result<ProviderUsage, ProviderError> {
    let rate_limit = payload.rate_limit.ok_or(ProviderError::InvalidData)?;
    let mut windows = Vec::new();
    if let Some(window) = rate_limit.primary_window {
        windows.push(normalize_window(&window)?);
    }
    if let Some(window) = rate_limit.secondary_window {
        windows.push(normalize_window(&window)?);
    }
    if windows.is_empty() {
        return Err(ProviderError::InvalidData);
    }
    Ok(ProviderUsage {
        provider_id: "codex".to_owned(),
        display_name: "Codex".to_owned(),
        account_label: None,
        windows,
        totals: None,
        models: None,
    })
}

fn normalize_window(window: &Window) -> Result<UsageWindow, ProviderError> {
    if !window.used_percent.is_finite()
        || !(0.0..=100.0).contains(&window.used_percent)
        || window.limit_window_seconds == 0
    {
        return Err(ProviderError::InvalidData);
    }
    let resets_at = OffsetDateTime::from_unix_timestamp(window.reset_at)
        .map_err(|_| ProviderError::InvalidData)?;
    let (kind, label) = match window.limit_window_seconds {
        18_000 => (WindowKind::Session, "5h".to_owned()),
        604_800 => (WindowKind::Weekly, "Weekly".to_owned()),
        seconds => (WindowKind::Custom, format!("{}h", seconds / 3600)),
    };
    Ok(UsageWindow {
        kind,
        label,
        duration_seconds: Some(window.limit_window_seconds),
        used_percent: Some(window.used_percent),
        remaining_percent: Some(100.0 - window.used_percent),
        resets_at: Some(resets_at),
        tokens: None,
        requests: None,
        cost_usd: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_short_and_weekly_windows() {
        let payload: Response = serde_json::from_str(r#"{"rate_limit":{"primary_window":{"used_percent":32,"limit_window_seconds":18000,"reset_at":1700000000},"secondary_window":{"used_percent":71,"limit_window_seconds":604800,"reset_at":1700100000}}}"#).expect("fixture");
        let usage = normalize(payload).expect("usage");
        assert_eq!(usage.windows.len(), 2);
        assert!(matches!(usage.windows[0].kind, WindowKind::Session));
        assert!(matches!(usage.windows[1].kind, WindowKind::Weekly));
        assert_eq!(usage.windows[0].remaining_percent, Some(68.0));
        assert!(usage.totals.is_none());
    }

    #[test]
    fn rejects_malformed_or_absent_quota_windows() {
        for body in [
            "not json",
            r#"{"rate_limit":null}"#,
            r#"{"rate_limit":{"primary_window":{"used_percent":101,"limit_window_seconds":18000,"reset_at":1700000000}}}"#,
        ] {
            let result = serde_json::from_str::<Response>(body)
                .map_err(|_| ProviderError::InvalidData)
                .and_then(normalize);
            assert!(matches!(result, Err(ProviderError::InvalidData)));
        }
    }
}
