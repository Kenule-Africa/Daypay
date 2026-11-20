use axum::{
    Router,
    body::to_bytes,
    extract::{Request, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use deypay_application::{email_verification::EmailProof, registration::valid_email};
use deypay_common::{
    config::Environment,
    email::{enabled, number, scope},
};
use deypay_infrastructure::{database::DatabasePool, email_verification as db};
use serde::Deserialize;

#[derive(Clone, Copy)]
pub struct Policy {
    pub scope: &'static str,
    pub per_minute: i32,
    pub cooldown_seconds: i32,
    pub resends_per_hour: i32,
    pub guesses: i32,
}
impl Policy {
    pub fn from_env(env: Environment) -> Result<Option<Self>, &'static str> {
        if !enabled()? {
            return Ok(None);
        }
        Ok(Some(Self {
            scope: scope(env),
            per_minute: number("EMAIL_VERIFY_REQUESTS_PER_MINUTE", 120, 1, 100000)?,
            cooldown_seconds: number("EMAIL_RESEND_COOLDOWN_SECONDS", 60, 1, 86400)?,
            resends_per_hour: number("EMAIL_RESEND_LIMIT_PER_HOUR", 5, 1, 100)?,
            guesses: number("EMAIL_VERIFY_MAX_GUESSES", 5, 1, 50)?,
        }))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Verify {
    token: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Resend {
    email: String,
}

pub fn routes(policy: Option<Policy>) -> Router<DatabasePool> {
    Router::new()
        .route(
            "/api/v1/auth/verify-email",
            post(move |State(pool), req| submit(pool, policy, Operation::Verify, req)),
        )
        .route(
            "/api/v1/auth/resend-verification",
            post(move |State(pool), req| submit(pool, policy, Operation::Resend, req)),
        )
        .route(
            "/verify-email",
            get(|| async {
                asset(
                    "text/html; charset=utf-8",
                    include_str!("verify_email.html"),
                )
            }),
        )
        .route(
            "/verify-email.js",
            get(|| async {
                asset(
                    "text/javascript; charset=utf-8",
                    include_str!("verify_email.js"),
                )
            }),
        )
}

enum Operation {
    Verify,
    Resend,
}
async fn submit(
    pool: DatabasePool,
    policy: Option<Policy>,
    operation: Operation,
    req: Request,
) -> Response {
    let Some(policy) = policy else {
        return reply(StatusCode::SERVICE_UNAVAILABLE);
    };
    let op = match operation {
        Operation::Verify => "verify",
        Operation::Resend => "resend",
    };
    match db::admit(&pool, op, policy.per_minute).await {
        Ok(true) => {}
        Ok(false) => return reply(StatusCode::TOO_MANY_REQUESTS),
        Err(_) => return reply(StatusCode::SERVICE_UNAVAILABLE),
    }
    if !req
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| {
            v.split(';')
                .next()
                .is_some_and(|s| s.trim().eq_ignore_ascii_case("application/json"))
        })
    {
        return reply(StatusCode::BAD_REQUEST);
    }
    let Ok(body) = to_bytes(req.into_body(), 2048).await else {
        return reply(StatusCode::BAD_REQUEST);
    };
    match operation {
        Operation::Verify => {
            let Ok(input) = serde_json::from_slice::<Verify>(&body) else {
                return reply(StatusCode::BAD_REQUEST);
            };
            let Some(proof) = EmailProof::parse(&input.token) else {
                return reply(StatusCode::UNPROCESSABLE_ENTITY);
            };
            match db::consume(
                &pool,
                proof.selector,
                &proof.digest,
                policy.scope,
                policy.guesses,
            )
            .await
            {
                Ok(true) => reply(StatusCode::OK),
                Ok(false) => reply(StatusCode::UNPROCESSABLE_ENTITY),
                Err(_) => reply(StatusCode::SERVICE_UNAVAILABLE),
            }
        }
        Operation::Resend => {
            let Ok(input) = serde_json::from_slice::<Resend>(&body) else {
                return reply(StatusCode::BAD_REQUEST);
            };
            if !valid_email(&input.email) {
                return reply(StatusCode::UNPROCESSABLE_ENTITY);
            }
            match db::resend(
                &pool,
                &input.email,
                policy.scope,
                policy.cooldown_seconds,
                policy.resends_per_hour,
            )
            .await
            {
                Ok(()) => reply(StatusCode::ACCEPTED),
                Err(_) => reply(StatusCode::SERVICE_UNAVAILABLE),
            }
        }
    }
}
fn reply(status: StatusCode) -> Response {
    let body = match status {
        StatusCode::OK | StatusCode::ACCEPTED => r#"{"data":{}}"#,
        StatusCode::SERVICE_UNAVAILABLE => {
            r#"{"error":{"code":"SERVICE_UNAVAILABLE","message":"Verification unavailable","details":{}}}"#
        }
        StatusCode::TOO_MANY_REQUESTS => {
            r#"{"error":{"code":"RATE_LIMITED","message":"Try again later","details":{}}}"#
        }
        _ => {
            r#"{"error":{"code":"VALIDATION_ERROR","message":"Invalid verification request","details":{}}}"#
        }
    };
    (
        status,
        [
            (header::CACHE_CONTROL, "no-store"),
            (header::CONTENT_TYPE, "application/json"),
        ],
        body,
    )
        .into_response()
}
fn asset(mime: &'static str, body: &'static str) -> Response {
    ([(header::CONTENT_TYPE,mime),(header::CACHE_CONTROL,"no-store"),(header::REFERRER_POLICY,"no-referrer"),
      (header::X_CONTENT_TYPE_OPTIONS,"nosniff"),(header::CONTENT_SECURITY_POLICY,
      "default-src 'none'; script-src 'self'; connect-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'")],body).into_response()
}
