use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use reqwest::StatusCode;

use super::http::UsageHttp;
use super::{CodexProvider, OpenCodeGoProvider, ProviderError, UsageProvider};
use crate::config::SecretString;

const CODEX_BODY: &str = r#"{"rate_limit":{"primary_window":{"used_percent":32,"limit_window_seconds":18000,"reset_at":1700000000},"secondary_window":{"used_percent":71,"limit_window_seconds":604800,"reset_at":1700100000}}}"#;
const GO_BODY: &str = r#"{"usage":{"rolling":{"status":"ok","percent":5,"resetsAt":"2026-09-19T19:10:07.880Z"},"weekly":{"status":"ok","percent":46,"resetsAt":"2026-09-21T00:00:00.880Z"},"monthly":{"status":"ok","percent":41,"resetsAt":"2026-10-08T05:27:23.880Z"}}}"#;

#[derive(Clone, Copy, Debug)]
enum Reply {
    Body(&'static str),
    Status(StatusCode),
    Timeout,
}

#[derive(Debug)]
struct FakeHttp {
    expected_token: SecretString,
    expected_account: Option<&'static str>,
    reply: Reply,
}

impl UsageHttp for FakeHttp {
    fn get<'a>(
        &'a self,
        token: &'a SecretString,
        account_id: Option<&'a str>,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            assert_eq!(token.expose(), self.expected_token.expose());
            assert_eq!(account_id, self.expected_account);
            match self.reply {
                Reply::Body(body) => Ok(body.as_bytes().to_vec()),
                Reply::Status(status) => Err(ProviderError::Status(status)),
                Reply::Timeout => Err(ProviderError::Request),
            }
        })
    }
}

fn codex_provider(reply: Reply) -> (tempfile::TempDir, CodexProvider) {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = directory.path().join("auth.json");
    std::fs::write(
        &path,
        r#"{"tokens":{"access_token":"secret-access","account_id":"workspace-1"}}"#,
    )
    .expect("auth fixture");
    let http = Arc::new(FakeHttp {
        expected_token: SecretString::new("secret-access"),
        expected_account: Some("workspace-1"),
        reply,
    });
    (directory, CodexProvider::with_http(path, http))
}

fn go_provider(reply: Reply) -> (tempfile::TempDir, OpenCodeGoProvider) {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = directory.path().join("auth.json");
    std::fs::write(&path, r#"{"opencode":{"type":"api","key":"zen-secret"},"opencode-go":{"type":"api","key":"go-secret"}}"#).expect("auth fixture");
    let http = Arc::new(FakeHttp {
        expected_token: SecretString::new("go-secret"),
        expected_account: None,
        reply,
    });
    (directory, OpenCodeGoProvider::with_http(path, http))
}

#[tokio::test]
async fn codex_collects_and_redacts_the_auth_file() {
    let (_directory, provider) = codex_provider(Reply::Body(CODEX_BODY));
    let usage = provider.collect().await.expect("usage");
    assert_eq!(usage.windows.len(), 2);
    assert_eq!(usage.windows[0].remaining_percent, Some(68.0));
    assert!(!format!("{provider:?}").contains("secret-access"));
    assert!(
        !serde_json::to_string(&usage)
            .expect("json")
            .contains("secret-access")
    );
}

#[tokio::test]
async fn codex_auth_failure_timeout_and_malformed_json_are_isolated() {
    for (reply, expected) in [
        (
            Reply::Status(StatusCode::UNAUTHORIZED),
            "provider returned HTTP 401 Unauthorized",
        ),
        (Reply::Timeout, "provider request failed or timed out"),
        (
            Reply::Body("not json"),
            "provider returned invalid usage data",
        ),
    ] {
        let (_directory, provider) = codex_provider(reply);
        let error = provider.collect().await.expect_err("failure");
        assert_eq!(error.to_string(), expected);
        assert!(!format!("{error:?}").contains("secret-access"));
    }
}

#[tokio::test]
async fn opencode_go_collects_and_isolates_failures() {
    let (_directory, provider) = go_provider(Reply::Body(GO_BODY));
    let usage = provider.collect().await.expect("usage");
    assert_eq!(usage.windows.len(), 3);
    assert_eq!(usage.provider_id, "opencode-go");
    assert!(
        !serde_json::to_string(&usage)
            .expect("json")
            .contains("go-secret")
    );
    for reply in [
        Reply::Status(StatusCode::FORBIDDEN),
        Reply::Timeout,
        Reply::Body("not json"),
    ] {
        let (_directory, provider) = go_provider(reply);
        assert!(provider.collect().await.is_err());
    }
}
