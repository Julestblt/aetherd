use std::path::PathBuf;
use std::sync::Arc;

use serde::Deserialize;
use time::OffsetDateTime;

use super::auth::{read_opencode_auth, resolve_opencode_auth};
use super::http::{HttpUsageClient, UsageHttp};
use super::{ProviderError, ProviderUsage, UsageProvider, UsageWindow, WindowKind};
use crate::config::OpenCodeConfig;

const USAGE_URL: &str = "https://opencode.ai/zen/go/v1/usage";

#[derive(Debug)]
pub(crate) struct OpenCodeGoProvider {
    auth_file: PathBuf,
    http: Arc<dyn UsageHttp>,
}

#[derive(Debug, Deserialize)]
struct Response {
    usage: Windows,
}

#[derive(Debug, Deserialize)]
struct Windows {
    rolling: Window,
    weekly: Window,
    monthly: Window,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Window {
    status: String,
    percent: f64,
    resets_at: String,
}

impl OpenCodeGoProvider {
    pub(crate) fn new(config: &OpenCodeConfig) -> Result<Self, ProviderError> {
        let auth_file = resolve_opencode_auth(
            config.auth_file.as_deref(),
            std::env::var_os("XDG_DATA_HOME")
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
            let key = read_opencode_auth(&self.auth_file)?;
            let body = self.http.get(&key, None).await?;
            let payload: Response =
                serde_json::from_slice(&body).map_err(|_| ProviderError::InvalidData)?;
            normalize(&payload)
        })
    }
}

fn normalize(payload: &Response) -> Result<ProviderUsage, ProviderError> {
    let windows = vec![
        normalize_window(
            &payload.usage.rolling,
            WindowKind::Rolling,
            "5h",
            Some(18_000),
        )?,
        normalize_window(&payload.usage.weekly, WindowKind::Weekly, "Weekly", None)?,
        normalize_window(&payload.usage.monthly, WindowKind::Monthly, "Monthly", None)?,
    ];
    Ok(ProviderUsage {
        provider_id: "opencode-go".to_owned(),
        display_name: "OpenCode Go".to_owned(),
        account_label: None,
        windows,
        totals: None,
        models: None,
    })
}

fn normalize_window(
    window: &Window,
    kind: WindowKind,
    label: &str,
    duration_seconds: Option<u64>,
) -> Result<UsageWindow, ProviderError> {
    if !matches!(window.status.as_str(), "ok" | "rate-limited")
        || !window.percent.is_finite()
        || !(0.0..=100.0).contains(&window.percent)
    {
        return Err(ProviderError::InvalidData);
    }
    let resets_at = OffsetDateTime::parse(
        &window.resets_at,
        &time::format_description::well_known::Rfc3339,
    )
    .map_err(|_| ProviderError::InvalidData)?;
    Ok(UsageWindow {
        kind,
        label: label.to_owned(),
        duration_seconds,
        used_percent: Some(window.percent),
        remaining_percent: Some(100.0 - window.percent),
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
    fn maps_three_go_windows() {
        let payload: Response = serde_json::from_str(r#"{"usage":{"rolling":{"status":"ok","percent":5,"resetsAt":"2026-09-19T19:10:07.880Z"},"weekly":{"status":"ok","percent":46,"resetsAt":"2026-09-21T00:00:00.880Z"},"monthly":{"status":"rate-limited","percent":100,"resetsAt":"2026-10-08T05:27:23.880Z"}}}"#).expect("fixture");
        let usage = normalize(&payload).expect("usage");
        assert_eq!(usage.windows.len(), 3);
        assert_eq!(usage.windows[0].duration_seconds, Some(18_000));
        assert_eq!(usage.windows[1].remaining_percent, Some(54.0));
        assert_eq!(usage.windows[2].remaining_percent, Some(0.0));
    }

    #[test]
    fn rejects_invalid_go_payloads() {
        for body in [
            "not json",
            r#"{"usage":{}}"#,
            r#"{"usage":{"rolling":{"status":"unknown","percent":5,"resetsAt":"2026-09-19T19:10:07Z"},"weekly":{"status":"ok","percent":46,"resetsAt":"2026-09-21T00:00:00Z"},"monthly":{"status":"ok","percent":41,"resetsAt":"2026-10-08T05:27:23Z"}}}"#,
        ] {
            let result = serde_json::from_str::<Response>(body)
                .map_err(|_| ProviderError::InvalidData)
                .and_then(|payload| normalize(&payload));
            assert!(matches!(result, Err(ProviderError::InvalidData)));
        }
    }
}
