use axum::Router;
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::AppState;
use crate::api::{ApiDoc, error, health, providers as providers_api, system as system_api};

/// Builds the complete HTTP router with `OpenAPI` and request tracing.
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
        .routes(routes!(system_api::network))
        .routes(routes!(system_api::tailscale))
        .routes(routes!(system_api::stream));

    OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(health::health))
        .routes(routes!(providers_api::providers))
        .routes(routes!(providers_api::usage))
        .nest("/v1/system", system)
        .split_for_parts()
}
