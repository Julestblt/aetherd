//! Background telemetry sampling and the shared in-memory snapshot.

mod builder;
mod sampler;
mod shutdown;
mod snapshot;

pub use builder::SnapshotBuilder;
pub use sampler::spawn_sampler;
pub(crate) use shutdown::Shutdown;
pub(crate) use snapshot::Section;
pub use snapshot::SystemSnapshot;
