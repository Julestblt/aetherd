use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::config::SecretString;

use super::ProviderError;

#[derive(Debug, Deserialize)]
pub(crate) struct CodexAuth {
    pub(crate) tokens: Option<CodexTokens>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CodexTokens {
    pub(crate) access_token: SecretString,
    pub(crate) account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OpenCodeAuth {
    #[serde(rename = "opencode-go")]
    pub(crate) go: Option<OpenCodeGoAuth>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OpenCodeGoAuth {
    #[serde(rename = "type")]
    pub(crate) kind: String,
    pub(crate) key: SecretString,
}

pub(crate) fn resolve_codex_auth(
    explicit: Option<&Path>,
    codex_home: Option<&Path>,
    home: Option<&Path>,
) -> Result<PathBuf, ProviderError> {
    resolve(
        explicit,
        [
            codex_home.map(|path| path.join("auth.json")),
            home.map(|path| path.join(".codex/auth.json")),
        ],
        "Codex provider enabled but no auth file could be resolved",
    )
}

pub(crate) fn resolve_opencode_auth(
    explicit: Option<&Path>,
    xdg_data_home: Option<&Path>,
    home: Option<&Path>,
) -> Result<PathBuf, ProviderError> {
    resolve(
        explicit,
        [
            xdg_data_home.map(|path| path.join("opencode/auth.json")),
            home.map(|path| path.join(".local/share/opencode/auth.json")),
        ],
        "OpenCode Go provider enabled but no auth file could be resolved",
    )
}

fn resolve(
    explicit: Option<&Path>,
    fallbacks: [Option<PathBuf>; 2],
    message: &'static str,
) -> Result<PathBuf, ProviderError> {
    if let Some(path) = explicit {
        return path
            .is_file()
            .then(|| path.to_path_buf())
            .ok_or(ProviderError::AuthFile(message));
    }
    fallbacks
        .into_iter()
        .flatten()
        .find(|path| path.is_file())
        .ok_or(ProviderError::AuthFile(message))
}

pub(crate) fn read_codex_auth(path: &Path) -> Result<CodexTokens, ProviderError> {
    let auth: CodexAuth = read_auth(path)?;
    auth.tokens
        .filter(|tokens| !tokens.access_token.is_empty())
        .ok_or(ProviderError::AuthFile(
            "Codex auth file has no OAuth access token",
        ))
}

pub(crate) fn read_opencode_auth(path: &Path) -> Result<SecretString, ProviderError> {
    let auth: OpenCodeAuth = read_auth(path)?;
    auth.go
        .filter(|entry| entry.kind == "api" && !entry.key.is_empty())
        .map(|entry| entry.key)
        .ok_or(ProviderError::AuthFile(
            "OpenCode auth file has no OpenCode Go API key",
        ))
}

fn read_auth<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, ProviderError> {
    let content =
        std::fs::read(path).map_err(|_| ProviderError::AuthFile("Auth file could not be read"))?;
    serde_json::from_slice(&content)
        .map_err(|_| ProviderError::AuthFile("Auth file contains invalid JSON"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_resolution_prefers_explicit_then_codex_home_then_home() {
        let directory = tempfile::tempdir().expect("tempdir");
        let codex = directory.path().join("codex");
        let home = directory.path().join("home");
        std::fs::create_dir_all(&codex).expect("codex dir");
        std::fs::create_dir_all(home.join(".codex")).expect("home dir");
        std::fs::write(codex.join("auth.json"), "{}").expect("codex auth");
        std::fs::write(home.join(".codex/auth.json"), "{}").expect("home auth");
        assert_eq!(
            resolve_codex_auth(None, Some(&codex), Some(&home)).expect("codex home"),
            codex.join("auth.json")
        );
        assert_eq!(
            resolve_codex_auth(
                Some(&home.join(".codex/auth.json")),
                Some(&codex),
                Some(&home)
            )
            .expect("explicit"),
            home.join(".codex/auth.json")
        );
        std::fs::remove_file(codex.join("auth.json")).expect("remove fixture");
        assert_eq!(
            resolve_codex_auth(None, Some(&codex), Some(&home)).expect("home"),
            home.join(".codex/auth.json")
        );
        assert!(
            resolve_codex_auth(Some(&codex.join("auth.json")), Some(&codex), Some(&home)).is_err()
        );
        assert!(resolve_codex_auth(None, None, None).is_err());
    }

    #[test]
    fn auth_tokens_are_redacted() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("auth.json");
        std::fs::write(
            &path,
            r#"{"tokens":{"access_token":"secret-access","account_id":"workspace"}}"#,
        )
        .expect("auth fixture");
        let tokens = read_codex_auth(&path).expect("valid auth");
        assert!(!format!("{tokens:?}").contains("secret-access"));
        assert!(
            !serde_json::to_string(&tokens.access_token)
                .expect("serialize")
                .contains("secret-access")
        );
        std::fs::write(&path, "not json").expect("bad fixture");
        assert!(read_codex_auth(&path).is_err());
    }

    #[test]
    fn opencode_go_key_is_read_from_the_exact_auth_entry() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("auth.json");
        std::fs::write(&path, r#"{"opencode":{"type":"api","key":"zen-secret"},"opencode-go":{"type":"api","key":"go-secret"}}"#).expect("auth fixture");
        let key = read_opencode_auth(&path).expect("go key");
        assert_eq!(key.expose(), "go-secret");
        assert!(!format!("{key:?}").contains("go-secret"));
        std::fs::write(&path, r#"{"opencode":{"type":"api","key":"zen-secret"}}"#)
            .expect("zen only");
        assert!(read_opencode_auth(&path).is_err());
    }
}
