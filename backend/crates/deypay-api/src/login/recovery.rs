//! Password recovery on the isolated verifier (IMP-P3-008).
//!
//! Route `POST /api/v1/auth/forgot-password` and `POST /api/v1/auth/reset-password`
//! directly here at the trusted HTTPS ingress, like login and refresh; the
//! general API refuses both. Reset tokens and new passwords therefore reach only
//! this process. Both routes require the approved browser origin and follow the
//! IMP-P3-012 admission contract: the operation's deployment budget before the
//! body is read, then the subject budget (canonical email or link selector)
//! before any lookup. New passwords pass the registration policy and
//! compromised-password screening and are hashed with the release's Argon2id
//! profile and worker slots.
use super::{VerifierAuth, reply};
use crate::sessions::expired_session_cookie;
use axum::{
    Router,
    body::to_bytes,
    extract::{Request, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::post,
};
use deypay_application::{
    password_recovery::{RecoveryError, request_reset, reset_password},
    passwords::{MAX_PASSWORD_BYTES, PasswordPolicy, ScreenedPasswords},
    registration::valid_email,
};
use deypay_common::email::number;
use deypay_infrastructure::{
    password_hashing::Argon2idHasher,
    password_recovery::recovery_enabled,
    password_screening::LocalCompromisedPasswords,
    rate_limit::{AdmissionError, AuthOperation},
    sessions::recovery::{PgRecovery, RecoveryLimits},
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

const FORGOT: &str = "/api/v1/auth/forgot-password";
const RESET: &str = "/api/v1/auth/reset-password";

/// Recovery configuration for the verifier. Screening is mandatory: without a
/// valid dataset the verifier does not start with recovery enabled.
pub struct RecoverySettings {
    limits: RecoveryLimits,
    screening: LocalCompromisedPasswords,
}

impl RecoverySettings {
    pub fn new(limits: RecoveryLimits, screening: LocalCompromisedPasswords) -> Self {
        Self { limits, screening }
    }

    /// `None` unless `PASSWORD_RECOVERY_ENABLED=true`.
    pub fn from_env() -> Result<Option<Self>, &'static str> {
        if !recovery_enabled()? {
            return Ok(None);
        }
        Ok(Some(Self::new(
            RecoveryLimits {
                cooldown_seconds: number("PASSWORD_RESET_COOLDOWN_SECONDS", 60, 1, 86400)?,
                requests_per_hour: number("PASSWORD_RESET_LIMIT_PER_HOUR", 5, 1, 100)?,
                guesses: number("PASSWORD_RESET_MAX_GUESSES", 5, 1, 50)?,
            },
            crate::registration_config::screening_from_env()?,
        )))
    }
}

struct Recovery {
    auth: Arc<VerifierAuth>,
    store: PgRecovery,
    passwords: ScreenedPasswords<LocalCompromisedPasswords, Argon2idHasher>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ForgotRequest {
    email: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResetRequest {
    token: String,
    password: String,
}

impl Drop for ResetRequest {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.token.zeroize();
        self.password.zeroize();
    }
}

impl VerifierAuth {
    /// Recovery routes for the verifier listener. Without settings both routes
    /// return 503, so a disabled deployment never accepts recovery traffic.
    pub fn recovery_routes(self: &Arc<Self>, settings: Option<RecoverySettings>) -> Router {
        let Some(settings) = settings else {
            let closed = post(|| async { reply(StatusCode::SERVICE_UNAVAILABLE) });
            return Router::new()
                .route(FORGOT, closed.clone())
                .route(RESET, closed);
        };
        let recovery = Arc::new(Recovery {
            store: PgRecovery::new(self.guard.store.clone(), settings.limits),
            passwords: ScreenedPasswords {
                policy: PasswordPolicy::PROVISIONAL,
                screening: settings.screening,
                // Shares the login verifier's Argon2id slots and cost profile.
                hasher: self.verifier.clone(),
            },
            auth: self.clone(),
        });
        Router::new()
            .route(FORGOT, post(forgot))
            .route(RESET, post(reset))
            .with_state(recovery)
    }
}

/// Origin, deployment admission and JSON checks shared by both routes, in the
/// login order, before the body is read.
async fn admit(
    recovery: &Recovery,
    operation: AuthOperation,
    req: Request,
) -> Result<Request, StatusCode> {
    if !recovery
        .auth
        .guard
        .origin
        .permits(req.method(), req.headers())
    {
        return Err(StatusCode::FORBIDDEN);
    }
    match recovery.store.admit_operation(operation).await {
        Ok(()) => {}
        Err(AdmissionError::Limited) => return Err(StatusCode::TOO_MANY_REQUESTS),
        Err(AdmissionError::Unavailable) => return Err(StatusCode::SERVICE_UNAVAILABLE),
    }
    let json = req
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .is_some_and(|m| m.trim().eq_ignore_ascii_case("application/json"));
    if json {
        Ok(req)
    } else {
        Err(StatusCode::BAD_REQUEST)
    }
}

async fn forgot(State(recovery): State<Arc<Recovery>>, req: Request) -> Response {
    let req = match admit(&recovery, AuthOperation::ForgotPassword, req).await {
        Ok(req) => req,
        Err(status) => return reply(status),
    };
    let Ok(body) = to_bytes(req.into_body(), 2048).await else {
        return reply(StatusCode::BAD_REQUEST);
    };
    let Ok(input) = serde_json::from_slice::<ForgotRequest>(&body) else {
        return reply(StatusCode::BAD_REQUEST);
    };
    if !valid_email(input.email.trim()) {
        return invalid(None);
    }
    match request_reset(&recovery.store, &input.email).await {
        // Known, unknown, unverified, ineligible, throttled and over-budget
        // addresses all receive this acknowledgement.
        Ok(()) => respond(StatusCode::ACCEPTED, json!({"data": {}})),
        Err(error) => refused(error),
    }
}

async fn reset(State(recovery): State<Arc<Recovery>>, req: Request) -> Response {
    let req = match admit(&recovery, AuthOperation::ResetPassword, req).await {
        Ok(req) => req,
        Err(status) => return reply(status),
    };
    let Ok(body) = to_bytes(req.into_body(), MAX_PASSWORD_BYTES + 1024).await else {
        return reply(StatusCode::BAD_REQUEST);
    };
    let Ok(input) = serde_json::from_slice::<ResetRequest>(&body) else {
        return reply(StatusCode::BAD_REQUEST);
    };
    drop(body);
    match reset_password(
        &recovery.store,
        &recovery.passwords,
        &input.token,
        &input.password,
    )
    .await
    {
        Ok(()) => {
            // Every session is revoked; drop any cookie this browser still holds.
            let mut response = respond(StatusCode::OK, json!({"data": {}}));
            response
                .headers_mut()
                .insert(header::SET_COOKIE, expired_session_cookie());
            response
        }
        Err(error) => refused(error),
    }
}

fn refused(error: RecoveryError) -> Response {
    match error {
        RecoveryError::InvalidRequest => invalid(None),
        RecoveryError::InvalidLink => invalid(Some("invalid_link")),
        RecoveryError::PasswordRejected => invalid(Some("password_rejected")),
        RecoveryError::Busy => reply(StatusCode::TOO_MANY_REQUESTS),
        RecoveryError::Unavailable => {
            tracing::warn!(
                event = "password_recovery_unavailable",
                "Password recovery unavailable"
            );
            reply(StatusCode::SERVICE_UNAVAILABLE)
        }
    }
}

fn invalid(reason: Option<&str>) -> Response {
    let details = reason.map_or_else(|| json!({}), |reason| json!({"reason": reason}));
    respond(
        StatusCode::UNPROCESSABLE_ENTITY,
        json!({"error": {"code": "VALIDATION_ERROR", "message": "Invalid recovery request", "details": details}}),
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
