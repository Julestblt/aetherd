use std::sync::Arc;

use time::OffsetDateTime;
use tokio::sync::watch;

use super::{ProviderError, ProviderState, ProviderStatus, ProviderUsage, UsageSnapshot};

#[derive(Clone, Debug)]
struct Entry {
    status: ProviderStatus,
    usage: Option<ProviderUsage>,
}

/// Latest successful provider values and current refresh status.
#[derive(Clone, Debug)]
pub struct ProviderCache {
    entries: watch::Sender<Arc<Vec<Entry>>>,
}

impl Default for ProviderCache {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderCache {
    /// Creates an empty provider cache.
    #[must_use]
    pub fn new() -> Self {
        let (entries, _) = watch::channel(Arc::new(Vec::new()));
        Self { entries }
    }

    pub(crate) fn configure(&self, id: &str, name: &str, enabled: bool, error: Option<String>) {
        let status = if enabled {
            ProviderState::Unavailable
        } else {
            ProviderState::Disabled
        };
        let error = if enabled {
            error.or_else(|| Some("provider data has not been collected yet".to_owned()))
        } else {
            None
        };
        self.update(id, |entries| {
            entries.push(Entry {
                status: ProviderStatus {
                    id: id.to_owned(),
                    display_name: name.to_owned(),
                    enabled,
                    status,
                    last_updated_at: None,
                    error,
                },
                usage: None,
            });
        });
    }

    pub(crate) fn publish(
        &self,
        id: &str,
        result: Result<ProviderUsage, ProviderError>,
        at: OffsetDateTime,
    ) {
        self.update(id, |entries| {
            if let Some(entry) = entries.iter_mut().find(|entry| entry.status.id == id) {
                match result {
                    Ok(usage) => {
                        entry.usage = Some(usage);
                        entry.status.status = ProviderState::Available;
                        entry.status.last_updated_at = Some(at);
                        entry.status.error = None;
                    }
                    Err(error) => {
                        entry.status.status = ProviderState::Unavailable;
                        entry.status.error = Some(error.to_string());
                    }
                }
            }
        });
    }

    pub(crate) fn mark_unavailable(&self, id: &str, error: String) {
        self.update(id, |entries| {
            if let Some(entry) = entries.iter_mut().find(|entry| entry.status.id == id) {
                entry.status.status = ProviderState::Unavailable;
                entry.status.error = Some(error);
            }
        });
    }

    fn update(&self, _id: &str, change: impl FnOnce(&mut Vec<Entry>)) {
        self.entries
            .send_modify(|entries| change(Arc::make_mut(entries)));
    }

    /// Returns status for all configured provider slots.
    #[must_use]
    pub fn statuses(&self) -> Vec<ProviderStatus> {
        self.entries
            .borrow()
            .iter()
            .map(|entry| entry.status.clone())
            .collect()
    }

    /// Returns the latest successful results, even when a later refresh failed.
    #[must_use]
    pub fn usage(&self) -> UsageSnapshot {
        let entries = self.entries.borrow();
        UsageSnapshot {
            collected_at: entries
                .iter()
                .filter_map(|entry| entry.status.last_updated_at)
                .max(),
            providers: entries
                .iter()
                .filter_map(|entry| entry.usage.clone())
                .collect(),
        }
    }
}
