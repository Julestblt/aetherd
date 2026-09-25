use utoipa::OpenApi;

use crate::api::error::{ErrorCode, ErrorDetail, ErrorResponse};
use crate::api::health::{HealthResponse, HealthStatus};

/// `OpenAPI` document generated from the Rust types and handler annotations.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "aetherd API",
        version = env!("CARGO_PKG_VERSION"),
        description = "Linux system telemetry and, later, normalized AI provider usage.",
        license(name = "MIT OR Apache-2.0")
    ),
    paths(crate::api::health::health),
    components(schemas(
        HealthResponse,
        HealthStatus,
        ErrorResponse,
        ErrorDetail,
        ErrorCode
    ))
)]
pub(crate) struct ApiDoc;
