use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use utoipa::ToSchema;

/// Errors surfaced by HTTP handlers.
///
/// Variants are split by the status a caller can react to, not by where the
/// failure was raised.
#[derive(Debug, thiserror::Error)]
pub(crate) enum ApiError {
    /// The requested resource does not exist.
    #[error("resource not found")]
    NotFound,
}

impl ApiError {
    fn status(&self) -> StatusCode {
        match self {
            Self::NotFound => StatusCode::NOT_FOUND,
        }
    }

    fn code(&self) -> ErrorCode {
        match self {
            Self::NotFound => ErrorCode::NotFound,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ErrorResponse {
            error: ErrorDetail {
                code: self.code(),
                message: self.to_string(),
            },
        };
        (self.status(), Json(body)).into_response()
    }
}

/// Fallback handler for unmatched routes.
pub(crate) async fn not_found() -> ApiError {
    ApiError::NotFound
}

/// Machine-readable error envelope returned by every failing endpoint.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct ErrorResponse {
    /// The error payload.
    pub error: ErrorDetail,
}

/// Details of a single error.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct ErrorDetail {
    /// Stable, machine-readable error code.
    pub code: ErrorCode,
    /// Human-readable description that never exposes internals.
    pub message: String,
}

/// Stable error codes clients may match on.
#[derive(Clone, Copy, Debug, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ErrorCode {
    /// The requested resource does not exist.
    NotFound,
}
