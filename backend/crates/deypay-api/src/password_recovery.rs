//! Reset page served by the general API (IMP-P3-008, owner decision of
//! 2026-11-25). Reset emails link to `/reset-password#<token>` on the configured
//! application origin. The page removes the token from history and posts it,
//! with the new password, to `/api/v1/auth/reset-password`, which the trusted
//! ingress routes to the isolated verifier. The general API never handles either.
use axum::{
    Router,
    http::header,
    response::{IntoResponse, Response},
    routing::get,
};

pub fn page_routes() -> Router {
    Router::new()
        .route(
            "/reset-password",
            get(|| async {
                asset(
                    "text/html; charset=utf-8",
                    include_str!("reset_password.html"),
                )
            }),
        )
        .route(
            "/reset-password.js",
            get(|| async {
                asset(
                    "text/javascript; charset=utf-8",
                    include_str!("reset_password.js"),
                )
            }),
        )
}

fn asset(mime: &'static str, body: &'static str) -> Response {
    (
        [
            (header::CONTENT_TYPE, mime),
            (header::CACHE_CONTROL, "no-store"),
            (header::REFERRER_POLICY, "no-referrer"),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            (
                header::CONTENT_SECURITY_POLICY,
                "default-src 'none'; script-src 'self'; connect-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
            ),
        ],
        body,
    )
        .into_response()
}
