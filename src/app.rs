use std::sync::Arc;

use axum::Router;
use time::OffsetDateTime;
use tokio::sync::watch;
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::api::{ApiDoc, error, health, system as system_api};
use crate::config::SamplingConfig;
use crate::sampling::{Shutdown, SystemSnapshot};
use crate::system::{MountStats, RealMountStats, SystemPaths};

/// Shared state handed to every request handler.
#[derive(Clone, Debug)]
pub struct AppState {
    pub(crate) started_at: OffsetDateTime,
    pub(crate) paths: SystemPaths,
    pub(crate) mount_stats: Arc<dyn MountStats>,
    pub(crate) sampling: SamplingConfig,
    snapshots: watch::Sender<Option<Arc<SystemSnapshot>>>,
    shutdown: watch::Sender<bool>,
}

impl AppState {
    /// Creates application state with real filesystem statistics and default
    /// sampling settings.
    #[must_use]
    pub fn new(paths: SystemPaths) -> Self {
        Self::with_sampling(paths, SamplingConfig::default())
    }

    /// Creates application state with real filesystem statistics and the given
    /// sampling settings.
    #[must_use]
    pub fn with_sampling(paths: SystemPaths, sampling: SamplingConfig) -> Self {
        Self::with_mount_stats_and_sampling(paths, Arc::new(RealMountStats), sampling)
    }

    /// Creates application state with an injected filesystem-statistics source
    /// and default sampling settings.
    #[must_use]
    pub fn with_mount_stats(paths: SystemPaths, mount_stats: Arc<dyn MountStats>) -> Self {
        Self::with_mount_stats_and_sampling(paths, mount_stats, SamplingConfig::default())
    }

    /// Creates application state with injected disk statistics and sampling
    /// settings.
    #[must_use]
    pub fn with_mount_stats_and_sampling(
        paths: SystemPaths,
        mount_stats: Arc<dyn MountStats>,
        sampling: SamplingConfig,
    ) -> Self {
        let (snapshots, _) = watch::channel(None);
        let (shutdown, _) = watch::channel(false);
        Self {
            started_at: OffsetDateTime::now_utc(),
            paths,
            mount_stats,
            sampling,
            snapshots,
            shutdown,
        }
    }

    /// Publishes a snapshot as the latest sample, replacing any previous one.
    pub fn publish(&self, snapshot: SystemSnapshot) {
        self.snapshots.send_replace(Some(Arc::new(snapshot)));
    }

    /// Requests cooperative shutdown of the sampler and streaming handlers.
    pub fn shutdown(&self) {
        self.shutdown.send_replace(true);
    }

    pub(crate) fn snapshot_sender(&self) -> watch::Sender<Option<Arc<SystemSnapshot>>> {
        self.snapshots.clone()
    }

    pub(crate) fn snapshot(&self) -> watch::Ref<'_, Option<Arc<SystemSnapshot>>> {
        self.snapshots.borrow()
    }

    pub(crate) fn shutdown_receiver(&self) -> Shutdown {
        Shutdown::receiver(&self.shutdown)
    }
}

/// Builds the complete HTTP router, including the `OpenAPI` document route and
/// the request tracing layer.
pub fn build_router(state: AppState) -> Router {
    let (router, api) = build_parts();

    let router = router
        .fallback(error::not_found)
        .method_not_allowed_fallback(error::method_not_allowed)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    with_api_docs(router, api)
}

#[cfg(feature = "swagger-ui")]
fn with_api_docs(router: Router, api: utoipa::openapi::OpenApi) -> Router {
    router.merge(utoipa_swagger_ui::SwaggerUi::new("/swagger-ui").url("/openapi.json", api))
}

#[cfg(not(feature = "swagger-ui"))]
fn with_api_docs(router: Router, api: utoipa::openapi::OpenApi) -> Router {
    router.route(
        "/openapi.json",
        axum::routing::get(move || {
            let api = api.clone();
            async move { axum::Json(api) }
        }),
    )
}

fn build_parts() -> (Router<AppState>, utoipa::openapi::OpenApi) {
    let system = OpenApiRouter::new()
        .routes(routes!(system_api::overview))
        .routes(routes!(system_api::cpu))
        .routes(routes!(system_api::memory))
        .routes(routes!(system_api::host))
        .routes(routes!(system_api::load))
        .routes(routes!(system_api::uptime))
        .routes(routes!(system_api::disks))
        .routes(routes!(system_api::network));

    OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(health::health))
        .nest("/v1/system", system)
        .split_for_parts()
}
