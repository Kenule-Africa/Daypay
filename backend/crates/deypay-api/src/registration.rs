use axum::{
    Router,
    body::to_bytes,
    extract::{Request, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::post,
};
use deypay_application::registration::{
    NoticeVersions, PasswordEnrollment, Registration, RegistrationError, register,
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
    email: String,
    phone: String,
    username: String,
    password: String,
}

/// Supplied by a trusted server-side consent flow, never deserialized from HTTP.
/// The captured contact and handle bind both affirmative acceptances to this submission.
pub struct AcceptedRegistration {
    pub email: String,
    pub phone: String,
    pub username: String,
    pub first_name: String,
    pub last_name: String,
    pub country_code: String,
    pub terms_version: String,
    pub privacy_version: String,
    pub accepted_at_unix: i64,
}

pub struct RegistrationSettings {
    pub passwords: Arc<dyn PasswordEnrollment + Send>,
    pub terms_version: String,
    pub privacy_version: String,
    pub attempts_per_minute: NonZeroU32,
}

struct RegistrationEndpoint {
    settings: RegistrationSettings,
    window: Mutex<(Instant, u32)>,
}

pub fn routes(settings: Option<RegistrationSettings>) -> Router<DatabasePool> {
    let endpoint = settings.map(|settings| {
        Arc::new(RegistrationEndpoint {
            settings,
            window: Mutex::new((Instant::now(), 0)),
        })
    });
    Router::new().route(
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
    mut req: Request,
) -> Response {
    let Some(endpoint) = endpoint else {
        return reply(Err(RegistrationError::Unavailable));
    };
    // Admit before reading the body or performing password work. The public router has no enabled configuration.
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
    let Some(evidence) = req.extensions_mut().remove::<Arc<AcceptedRegistration>>() else {
        return reply(Err(RegistrationError::Unavailable));
    };
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
    if evidence.email != body.email
        || evidence.phone != body.phone
        || evidence.username != body.username
    {
        return reply(Err(RegistrationError::InvalidInput));
    }
    let input = Registration {
        email: &body.email,
        phone: &body.phone,
        username: &body.username,
        first_name: &evidence.first_name,
        last_name: &evidence.last_name,
        country_code: &evidence.country_code,
        terms_version: &evidence.terms_version,
        privacy_version: &evidence.privacy_version,
        accepted_at_unix: evidence.accepted_at_unix,
    };
    let notices = NoticeVersions {
        terms: &endpoint.settings.terms_version,
        privacy: &endpoint.settings.privacy_version,
    };
    reply(
        register(
            &PgRegistrationRepository(pool),
            endpoint.settings.passwords.as_ref(),
            &notices,
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
