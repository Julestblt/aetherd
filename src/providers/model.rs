use serde::Serialize;
use time::OffsetDateTime;
use utoipa::ToSchema;

/// Availability of a configured provider.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProviderState {
    /// A successful refresh is cached.
    Available,
    /// Collection is pending, failed, or unsupported.
    Unavailable,
    /// The provider is disabled in configuration.
    Disabled,
}

/// Cached state of one provider.
#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct ProviderStatus {
    /// Stable provider identifier.
    pub id: String,
    /// Human readable provider name.
    pub display_name: String,
    /// Whether collection is configured.
    pub enabled: bool,
    /// Current collection state.
    pub status: ProviderState,
    /// Time of the last successful refresh, in RFC3339 UTC.
    #[schema(value_type = Option<String>, format = DateTime)]
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_updated_at: Option<OffsetDateTime>,
    /// Secret-free reason when unavailable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Last successfully collected provider usage values.
#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct UsageSnapshot {
    /// Time the latest successful provider refresh completed, in RFC3339 UTC.
    #[schema(value_type = Option<String>, format = DateTime)]
    #[serde(with = "time::serde::rfc3339::option")]
    pub collected_at: Option<OffsetDateTime>,
    /// Successful provider results; unavailable providers are listed by /v1/providers.
    pub providers: Vec<ProviderUsage>,
}

/// Provider-neutral account usage.
#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct ProviderUsage {
    /// Stable provider identifier.
    pub provider_id: String,
    /// Human readable provider name.
    pub display_name: String,
    /// Safe account or plan label, when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_label: Option<String>,
    /// Quota windows reported by the source.
    pub windows: Vec<UsageWindow>,
    /// Aggregate counters, when reported by the source.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub totals: Option<ModelUsage>,
    /// Per-model counters, when reported by the source.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<ModelUsage>>,
}

/// Period represented by a quota window.
#[derive(Clone, Copy, Debug, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum WindowKind {
    /// Short session window.
    Session,
    /// Rolling interval not tied to a calendar boundary.
    Rolling,
    /// Daily quota.
    Daily,
    /// Weekly quota.
    Weekly,
    /// Monthly quota.
    Monthly,
    /// Other source-defined interval.
    Custom,
}

/// One quota or accounting window.
#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct UsageWindow {
    /// Classification of the window.
    pub kind: WindowKind,
    /// Short display label.
    pub label: String,
    /// Window duration in seconds, when reported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<u64>,
    /// Percentage consumed, from 0 to 100.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub used_percent: Option<f64>,
    /// Percentage left, from 0 to 100.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remaining_percent: Option<f64>,
    /// Quota reset time, in RFC3339 UTC.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, format = DateTime)]
    #[serde(with = "time::serde::rfc3339::option")]
    pub resets_at: Option<OffsetDateTime>,
    /// Token count, when reported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens: Option<u64>,
    /// Request count, when reported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requests: Option<u64>,
    /// USD cost, when reported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
}

/// Counters attributed to a model or all models.
#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct ModelUsage {
    /// Model identifier, or "all" for totals.
    pub model: String,
    /// Input token count.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u64>,
    /// Cached input token count.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_input_tokens: Option<u64>,
    /// Output token count.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,
    /// Request count.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requests: Option<u64>,
    /// Cost in USD.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
}
