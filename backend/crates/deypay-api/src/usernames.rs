//! `GET /api/v1/usernames/availability?username=<handle>` (IMP-P3-009).
//!
//! Owner decision of 2026-11-25: public and advisory. A handle in the approved
//! format returns `{"available": true|false}`; reserved and claimed handles are
//! indistinguishable, and nothing about an owner is returned. Registration's
//! unique keys remain the authority. The IMP-P3-012 admission contract applies:
//! the deployment budget before the query is parsed, then the canonical
//! handle's budget before the lookup.
use axum::{
    Router,
    extract::{Request, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use deypay_application::usernames::{AvailabilityError, availability};
use deypay_infrastructure::{
    database::DatabasePool,
    rate_limit::{self, AdmissionError, AuthOperation},
    registration::PgRegistrationRepository,
};
use percent_encoding::percent_decode_str;
use serde_json::json;

/// Ample for `username=` and a percent-encoded 31-byte handle.
const MAX_QUERY_BYTES: usize = 256;

pub fn routes() -> Router<DatabasePool> {
    Router::new().route("/api/v1/usernames/availability", get(check))
}

async fn check(State(pool): State<DatabasePool>, req: Request) -> Response {
    match rate_limit::admit(&pool, AuthOperation::UsernameAvailability).await {
        Ok(()) => {}
        Err(AdmissionError::Limited) => return refused(StatusCode::TOO_MANY_REQUESTS),
        Err(AdmissionError::Unavailable) => return refused(StatusCode::SERVICE_UNAVAILABLE),
    }
    let Some(username) = username_param(req.uri().query()) else {
        return refused(StatusCode::BAD_REQUEST);
    };
    match availability(&PgRegistrationRepository(pool), &username).await {
        Ok(available) => respond(StatusCode::OK, json!({"data": {"available": available}})),
        Err(AvailabilityError::InvalidFormat) => respond(
            StatusCode::UNPROCESSABLE_ENTITY,
            json!({"error": {"code": "VALIDATION_ERROR", "message": "Invalid username",
                "details": {"fields": ["username"]}}}),
        ),
        Err(AvailabilityError::Limited) => refused(StatusCode::TOO_MANY_REQUESTS),
        Err(AvailabilityError::Unavailable) => refused(StatusCode::SERVICE_UNAVAILABLE),
    }
}

/// Exactly one `username` parameter; anything else is an invalid request.
fn username_param(query: Option<&str>) -> Option<String> {
    let query = query.filter(|q| q.len() <= MAX_QUERY_BYTES)?;
    let (key, value) = query.split_once('=')?;
    if key != "username" || value.contains(['&', '=', ';']) {
        return None;
    }
    let value = value.replace('+', " ");
    percent_decode_str(&value)
        .decode_utf8()
        .ok()
        .map(|v| v.into_owned())
}

fn refused(status: StatusCode) -> Response {
    let (code, message) = match status {
        StatusCode::TOO_MANY_REQUESTS => ("RATE_LIMITED", "Try again later"),
        StatusCode::SERVICE_UNAVAILABLE => ("SERVICE_UNAVAILABLE", "Username check unavailable"),
        _ => ("VALIDATION_ERROR", "Invalid request"),
    };
    respond(
        status,
        json!({"error": {"code": code, "message": message, "details": {}}}),
    )
}

fn respond(status: StatusCode, body: serde_json::Value) -> Response {
    (
        status,
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        body.to_string(),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::username_param;

    #[test]
    fn only_a_single_bounded_username_parameter_is_accepted() {
        assert_eq!(username_param(Some("username=ada")).as_deref(), Some("ada"));
        assert_eq!(
            username_param(Some("username=%40Ada_Okafor")).as_deref(),
            Some("@Ada_Okafor")
        );
        assert_eq!(username_param(Some("username=")).as_deref(), Some(""));
        for query in [
            None,
            Some(""),
            Some("username"),
            Some("user=ada"),
            Some("username=ada&username=bob"),
            Some("username=ada&extra=1"),
            Some("username=a=b"),
            Some("username=%FF"),
        ] {
            assert_eq!(username_param(query), None, "{query:?}");
        }
        let long = format!("username={}", "a".repeat(300));
        assert_eq!(username_param(Some(&long)), None);
    }
}
