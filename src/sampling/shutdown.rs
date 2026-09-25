use tokio::sync::watch;

/// Cooperative shutdown signal shared by the sampler and streaming handlers.
#[derive(Clone, Debug)]
pub(crate) struct Shutdown(watch::Receiver<bool>);

impl Shutdown {
    /// Subscribes to the given shutdown channel.
    pub(crate) fn receiver(sender: &watch::Sender<bool>) -> Self {
        Self(sender.subscribe())
    }

    /// Resolves once shutdown has been requested.
    pub(crate) async fn recv(&mut self) {
        if *self.0.borrow() {
            return;
        }
        let _ = self.0.changed().await;
    }
}
