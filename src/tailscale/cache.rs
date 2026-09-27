use tokio::sync::watch;

use super::model::TailscaleSnapshot;
use crate::sampling::Section;

/// Latest-value cache of the Tailscale section shared by the refresher and the
/// sampler.
///
/// The cache keeps only the most recent section, so a slow consumer can never
/// grow memory: readers always observe the latest value.
#[derive(Clone, Debug)]
pub(crate) struct TailscaleCache {
    section: watch::Sender<Section<TailscaleSnapshot>>,
}

impl TailscaleCache {
    /// Creates a cache seeded with `section`.
    pub(crate) fn new(section: Section<TailscaleSnapshot>) -> Self {
        let (section, _) = watch::channel(section);
        Self { section }
    }

    /// Creates a cache whose section is unavailable for `reason`.
    pub(crate) fn unavailable(reason: &str) -> Self {
        Self::new(Section::Unavailable {
            reason: reason.to_owned(),
        })
    }

    /// Replaces the cached section with the latest refresh result.
    pub(crate) fn publish(&self, section: Section<TailscaleSnapshot>) {
        self.section.send_replace(section);
    }

    /// Subscribes to the cached section for reading in a snapshot.
    pub(crate) fn subscribe(&self) -> watch::Receiver<Section<TailscaleSnapshot>> {
        self.section.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;

    use super::*;

    fn snapshot() -> TailscaleSnapshot {
        TailscaleSnapshot {
            collected_at: OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("timestamp"),
            tailnet: "example.ts.net".to_owned(),
            devices: Vec::new(),
        }
    }

    #[test]
    fn starts_unavailable_with_the_given_reason() {
        let cache = TailscaleCache::unavailable("not collected");

        let section = cache.subscribe().borrow().clone();
        assert_eq!(
            section,
            Section::Unavailable {
                reason: "not collected".to_owned()
            }
        );
    }

    #[test]
    fn publishes_the_latest_value_to_new_and_existing_subscribers() {
        let cache = TailscaleCache::unavailable("not collected");
        let existing = cache.subscribe();

        cache.publish(Section::Available { value: snapshot() });

        assert!(matches!(
            &*cache.subscribe().borrow(),
            Section::Available { .. }
        ));
        assert!(matches!(&*existing.borrow(), Section::Available { .. }));
    }

    #[test]
    fn replaces_a_previous_value() {
        let cache = TailscaleCache::unavailable("not collected");
        cache.publish(Section::Available { value: snapshot() });
        cache.publish(Section::Unavailable {
            reason: "tailscale API returned HTTP 500".to_owned(),
        });

        assert_eq!(
            cache.subscribe().borrow().clone(),
            Section::Unavailable {
                reason: "tailscale API returned HTTP 500".to_owned()
            }
        );
    }
}
