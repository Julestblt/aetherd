use axum::Router;
use time::OffsetDateTime;
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::api::{ApiDoc, error, health};

/// Shared state handed to every request handler.
#[derive(Clone, Debug)]
pub struct AppState {
    pub(crate) started_at: OffsetDateTime,
}

impl AppState {
    /// Creates application state with the daemon start time set to now.
    #[must_use]
    pub fn new() -> Self {
        Self {
            started_at: OffsetDateTime::now_utc(),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

/// Builds the complete HTTP router, including the `OpenAPI` document route.
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
        .with_state(state)
}

fn build_parts() -> (Router<AppState>, utoipa::openapi::OpenApi) {
    OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(health::health))
        .split_for_parts()
}
