use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use reqwest::StatusCode;

use super::http::UsageHttp;
use super::{CodexProvider, OpenCodeGoProvider, ProviderError, UsageProvider};
use crate::config::SecretString;

const CODEX_BODY: &str = r#"{"rate_limit":{"primary_window":{"used_percent":32,"limit_window_seconds":18000,"reset_at":1700000000},"secondary_window":{"used_percent":71,"limit_window_seconds":604800,"reset_at":1700100000}}}"#;
const GO_BODY: &str = "id,created_at,provider,model,input_tokens,output_tokens,cache_read_tokens,cost_micro_cents,service\n1,2026-09-30T12:00:00Z,anthropic,claude-sonnet,100,20,50,25000000,\n2,2026-09-30T13:00:00Z,opencode,glm-5,200,30,70,5000000,\n3,2026-09-30T14:00:00Z,,,,,,1000000,web-search\n";

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

fn go_provider(reply: Reply) -> OpenCodeGoProvider {
    let http = Arc::new(FakeHttp {
        expected_token: SecretString::new("oc_sk_test-secret"),
        expected_account: None,
        reply,
    });
    OpenCodeGoProvider::with_http(SecretString::new("oc_sk_test-secret"), http)
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
    let provider = go_provider(Reply::Body(GO_BODY));
    let usage = provider.collect().await.expect("usage");
    assert_eq!(usage.windows.len(), 1);
    assert_eq!(usage.provider_id, "opencode-go");
    assert_eq!(usage.windows[0].used_percent, None);
    assert_eq!(usage.windows[0].requests, Some(2));
    assert_eq!(usage.windows[0].cost_usd, Some(0.3));
    assert_eq!(usage.models.as_ref().expect("models").len(), 2);
    assert!(!format!("{provider:?}").contains("oc_sk_test-secret"));
    assert!(
        !serde_json::to_string(&usage)
            .expect("json")
            .contains("oc_sk_test-secret")
    );
    for reply in [
        Reply::Status(StatusCode::UNAUTHORIZED),
        Reply::Status(StatusCode::FORBIDDEN),
        Reply::Timeout,
        Reply::Body("not csv"),
    ] {
        let provider = go_provider(reply);
        let error = provider.collect().await.expect_err("upstream failure");
        assert!(!format!("{error:?}").contains("oc_sk_test-secret"));
    }
}
