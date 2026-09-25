//! Integration tests for the generated `OpenAPI` document.

mod common;

use axum::http::StatusCode;

#[tokio::test]
async fn openapi_documents_health_path_and_schema() {
    let (status, body) = common::get("/openapi.json").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["info"]["title"], "aetherd API");
    assert!(body["paths"]["/health"].is_object());
    assert!(body["paths"]["/v1/system"].is_object());
    assert!(body["paths"]["/v1/system/cpu"].is_object());
    assert!(body["paths"]["/v1/system/memory"].is_object());
    assert!(body["paths"]["/v1/system/disks"].is_object());
    assert!(body["paths"]["/v1/system/network"].is_object());
    assert!(body["paths"]["/v1/system/stream"].is_object());
    assert!(
        body["paths"].get("/cpu").is_none(),
        "unprefixed system paths must not be advertised"
    );
    assert!(body["components"]["schemas"]["HealthResponse"].is_object());
    assert!(body["components"]["schemas"]["CpuMetrics"].is_object());
    assert!(body["components"]["schemas"]["MemoryMetrics"].is_object());
    assert!(body["components"]["schemas"]["DisksMetrics"].is_object());
    assert!(body["components"]["schemas"]["ErrorResponse"].is_object());
}

#[tokio::test]
async fn openapi_document_matches_snapshot() {
    let (_, body) = common::get("/openapi.json").await;
    insta::assert_json_snapshot!(body);
}
