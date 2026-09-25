//! Availability wrapper for overview sections.

use serde::Serialize;
use utoipa::ToSchema;

/// A metric that may or may not be available on the current host.
///
/// The aggregate overview uses this so a single failing collector never fails
/// the whole response.
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum Section<T> {
    /// The metric was collected.
    Available {
        /// The collected metric.
        value: T,
    },
    /// The metric could not be collected on this host.
    Unavailable {
        /// Why the metric is unavailable.
        reason: String,
    },
}
