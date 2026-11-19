use axum::{
    Router,
    body::to_bytes,
    extract::{Request, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::post,
};
use deypay_application::registration::{
    PasswordEnrollment, Registration, RegistrationError, register,
};
use deypay_infrastructure::{database::DatabasePool, registration::PgRegistrationRepository};
use serde::Deserialize;
use std::{
    num::NonZeroU32,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistrationBody {
    first_name: String,
    last_name: String,
    country_code: String,
    email: String,
    phone: String,
    username: String,
    password: String,
    terms_version: String,
    privacy_version: String,
    accept_terms: bool,
    acknowledge_privacy_notice: bool,
}

pub struct RegistrationSettings {
    pub passwords: Arc<dyn PasswordEnrollment + Send>,
    pub attempts_per_minute: NonZeroU32,
}

struct RegistrationEndpoint {
    settings: RegistrationSettings,
    window: Mutex<(Instant, u32)>,
}

pub fn routes(
    environment: deypay_common::config::Environment,
    settings: Option<RegistrationSettings>,
) -> Router<DatabasePool> {
    use deypay_common::config::Environment;
    let settings =
        settings.filter(|_| matches!(environment, Environment::Local | Environment::Test));
    let scope = if settings.is_some() {
        "sandbox"
    } else {
        "published"
    };
    let endpoint = settings.map(|settings| {
        Arc::new(RegistrationEndpoint {
            settings,
            window: Mutex::new((Instant::now(), 0)),
        })
    });
    crate::legal::routes(scope).route(
        "/api/v1/auth/register",
        post(move |State(pool), req| {
            let endpoint = endpoint.clone();
            async move { submit(pool, endpoint, req).await }
        }),
    )
}

async fn submit(
    pool: DatabasePool,
    endpoint: Option<Arc<RegistrationEndpoint>>,
    req: Request,
) -> Response {
    let Some(endpoint) = endpoint else {
        return reply(Err(RegistrationError::Unavailable));
    };
    // Admit before reading the body or performing password work.
    {
        let Ok(mut window) = endpoint.window.lock() else {
            return reply(Err(RegistrationError::Unavailable));
        };
        if window.0.elapsed() >= Duration::from_secs(60) {
            *window = (Instant::now(), 0);
        }
        if window.1 >= endpoint.settings.attempts_per_minute.get() {
            return reply(Err(RegistrationError::Busy));
        }
        window.1 += 1;
    }
    if !req
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|s| {
            s.split(';')
                .next()
                .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"))
        })
    {
        return invalid_request();
    }
    let Ok(bytes) = to_bytes(req.into_body(), 64 * 1024).await else {
        return invalid_request();
    };
    let Ok(body) = serde_json::from_slice::<RegistrationBody>(&bytes) else {
        return invalid_request();
    };
    let input = Registration {
        email: &body.email,
        phone: &body.phone,
        username: &body.username,
        first_name: &body.first_name,
        last_name: &body.last_name,
        country_code: &body.country_code,
        terms_version: &body.terms_version,
        privacy_version: &body.privacy_version,
        accept_terms: body.accept_terms,
        acknowledge_privacy_notice: body.acknowledge_privacy_notice,
    };
    reply(
        register(
            &PgRegistrationRepository(pool),
            endpoint.settings.passwords.as_ref(),
            &input,
            &body.password,
        )
        .await,
    )
}

const INVALID: &str = r#"{"error":{"code":"VALIDATION_ERROR","message":"Invalid registration request","details":{}}}"#;

fn invalid_request() -> Response {
    response(StatusCode::BAD_REQUEST, INVALID)
}

fn reply(result: Result<(), RegistrationError>) -> Response {
    let (status, body) = match result {
        Ok(()) => (StatusCode::ACCEPTED, r#"{"data":{}}"#),
        Err(RegistrationError::InvalidInput) => (StatusCode::UNPROCESSABLE_ENTITY, INVALID),
        Err(RegistrationError::Busy) => (
            StatusCode::TOO_MANY_REQUESTS,
            r#"{"error":{"code":"RATE_LIMITED","message":"Try again later","details":{}}}"#,
        ),
        Err(RegistrationError::Unavailable) => (
            StatusCode::SERVICE_UNAVAILABLE,
            r#"{"error":{"code":"SERVICE_UNAVAILABLE","message":"Registration unavailable","details":{}}}"#,
        ),
    };
    response(status, body)
}

fn response(status: StatusCode, body: &'static str) -> Response {
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
