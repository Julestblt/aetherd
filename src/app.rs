use axum::Router;
use time::OffsetDateTime;
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::api::{ApiDoc, error, health, system as system_api};
use crate::system::SystemPaths;

/// Shared state handed to every request handler.
#[derive(Clone, Debug)]
pub struct AppState {
    pub(crate) started_at: OffsetDateTime,
    pub(crate) paths: SystemPaths,
}

impl AppState {
    /// Creates application state with the daemon start time set to now and the
    /// given telemetry roots.
    #[must_use]
    pub fn new(paths: SystemPaths) -> Self {
        Self {
            started_at: OffsetDateTime::now_utc(),
            paths,
        }
    }
}

/// Builds the complete HTTP router, including the `OpenAPI` document route and
/// the request tracing layer.
pub fn build_router(state: AppState) -> Router {
    let (router, api) = build_parts();
    router
        .route(
            "/openapi.json",
            axum::routing::get(move || {
                let api = api.clone();
                async move { axum::Json(api) }
            }),
        )
        .fallback(error::not_found)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

fn build_parts() -> (Router<AppState>, utoipa::openapi::OpenApi) {
    let system = OpenApiRouter::new()
        .routes(routes!(system_api::cpu))
        .routes(routes!(system_api::memory));

    OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(health::health))
        .nest("/v1/system", system)
        .split_for_parts()
}
