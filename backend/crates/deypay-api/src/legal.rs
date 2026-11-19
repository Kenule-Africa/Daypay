use axum::{
    Router,
    extract::{Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use deypay_infrastructure::{database::DatabasePool, legal};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use serde_json::json;

const PATH_SEGMENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

fn status_label(scope: &str) -> &'static str {
    if scope == "sandbox" {
        "sandbox_draft"
    } else {
        "published"
    }
}

pub fn routes(scope: &'static str) -> Router<DatabasePool> {
    Router::new()
        .route(
            "/api/v1/legal/documents",
            get(move |State(pool)| index(pool, scope)),
        )
        .route(
            "/api/v1/legal/documents/{id}/{version}",
            get(move |State(pool), Path((id, version))| content(pool, scope, id, version)),
        )
}

async fn index(pool: DatabasePool, scope: &str) -> Response {
    let Ok(rows) = legal::current(&pool, scope).await else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let docs: Vec<_> = rows
        .into_iter()
        .map(|d| {
            json!({
                "document_id":d.document_id,"title":d.title,"version":d.version,"sha256":d.sha256,
                "status":status_label(scope),
                "url":format!("/api/v1/legal/documents/{}/{}",d.document_id,utf8_percent_encode(&d.version, PATH_SEGMENT))
            })
        })
        .collect();
    (
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        json!({"data":{"scope":scope,"documents":docs}}).to_string(),
    )
        .into_response()
}

async fn content(pool: DatabasePool, scope: &str, id: String, version: String) -> Response {
    match legal::revision(&pool, scope, &id, &version).await {
        Ok(Some(body)) => (
            [
                (
                    header::CONTENT_TYPE.as_str(),
                    "text/markdown; charset=utf-8",
                ),
                ("x-deypay-legal-status", status_label(scope)),
                ("x-content-type-options", "nosniff"),
                (header::CACHE_CONTROL.as_str(), "no-store"),
            ],
            body,
        )
            .into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}
