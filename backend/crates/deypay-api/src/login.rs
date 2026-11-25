//! For a customer handler, apply `auth.protect(customer_routes)` to its Axum
//! router and extract `AuthenticatedCustomer`. Use `customer.id()` to scope all
//! reads and writes. The extractor supplies no KYC decision or financial authority.
//! Recheck any action-specific eligibility inside the protected write transaction.
//!
//! Configure this only in local/test after applying the SQLx migrations and
//! `infrastructure/postgres/sandbox-auth-grants.psql` as the migration owner.
//! Set `DEYPAY_LOGIN_MODE=sandbox`, the exact HTTPS `DEYPAY_LOGIN_ORIGIN`,
//! both `DEYPAY_SESSION_IDLE_SECONDS` and `DEYPAY_SESSION_ABSOLUTE_SECONDS`,
//! and `DEYPAY_LOGIN_REQUESTS_PER_MINUTE` and `DEYPAY_LOGIN_PER_IDENTIFIER_PER_MINUTE`.
//! The admission windows are process-local. Registration and login share the configured
//! Argon2 work slots. Hosted authentication stays disabled pending policy approval.
use crate::sessions::{BrowserOrigin, request_token, session_cookie};
use axum::{
    Router,
    body::to_bytes,
    extract::{FromRequestParts, Request, State},
    http::{StatusCode, header, request::Parts},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::post,
};
use deypay_application::{
    login::{LoginStore, SessionIdentity, SessionLifetime, SessionStore, login},
    passwords::{CredentialError, CredentialVerifier, MAX_PASSWORD_BYTES},
    registration::valid_email,
};
use deypay_common::config::Environment;
use deypay_infrastructure::{
    database::DatabasePool,
    password_hashing::Argon2idHasher,
    sessions::{PgSessions, SessionToken},
};
use serde::Deserialize;
use std::{
    collections::{HashMap, hash_map::RandomState},
    hash::BuildHasher,
    num::NonZeroU32,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use uuid::Uuid;
use zeroize::Zeroize;

/// Handlers may extract this type only after the session middleware validates
/// the cookie against the store. It provides identity, without KYC or financial authorization.
#[derive(Clone)]
pub struct AuthenticatedCustomer(SessionIdentity);

impl AuthenticatedCustomer {
    pub fn id(&self) -> Uuid {
        self.0.customer_id
    }

    pub fn account_status(&self) -> &str {
        &self.0.account_status
    }
}

impl<S: Send + Sync> FromRequestParts<S> for AuthenticatedCustomer {
    type Rejection = Response;
    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Response> {
        parts
            .extensions
            .get::<Self>()
            .cloned()
            .ok_or_else(|| reply(StatusCode::UNAUTHORIZED))
    }
}

pub struct SandboxAuth<R, V> {
    store: R,
    verifier: V,
    origin: BrowserOrigin,
    lifetime: SessionLifetime,
    limits: LoginLimits,
}

pub struct LoginLimits {
    total: NonZeroU32,
    per_identifier: NonZeroU32,
    keys: RandomState,
    window: Mutex<LoginWindow>,
}

struct LoginWindow {
    start: Instant,
    count: u64,
    identifiers: HashMap<u64, u64>,
}

impl LoginLimits {
    pub fn new(total: NonZeroU32, per_identifier: NonZeroU32) -> Self {
        Self {
            total,
            per_identifier,
            keys: RandomState::new(),
            window: Mutex::new(LoginWindow {
                start: Instant::now(),
                count: 0,
                identifiers: HashMap::new(),
            }),
        }
    }
    fn admit(&self, email: Option<&str>) -> Result<(), CredentialError> {
        let mut window = self
            .window
            .lock()
            .map_err(|_| CredentialError::Unavailable)?;
        if window.start.elapsed() >= Duration::from_secs(60) {
            window.start = Instant::now();
            window.count = 0;
            window.identifiers.clear();
        }
        let (count, cap) = if let Some(email) = email {
            let key = self.keys.hash_one(email);
            if window.identifiers.len() >= self.total.get() as usize
                && !window.identifiers.contains_key(&key)
            {
                return Err(CredentialError::Busy);
            }
            (
                window.identifiers.entry(key).or_default(),
                self.per_identifier,
            )
        } else {
            (&mut window.count, self.total)
        };
        let limit = u64::from(cap.get());
        if *count >= limit {
            if *count == limit {
                *count += 1;
                tracing::warn!(
                    per_identifier = email.is_some(),
                    "Sandbox login admission limit reached"
                );
            }
            return Err(CredentialError::Busy);
        }
        *count += 1;
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LoginRequest {
    identifier: String,
    password: String,
}

impl Drop for LoginRequest {
    fn drop(&mut self) {
        self.password.zeroize();
    }
}

pub type CustomerAuth = SandboxAuth<PgSessions, Argon2idHasher>;

impl CustomerAuth {
    pub fn from_env(
        environment: Environment,
        pool: DatabasePool,
    ) -> Result<Option<Arc<Self>>, &'static str> {
        if !matches!(environment, Environment::Local | Environment::Test) {
            return Ok(None);
        }
        match std::env::var("DEYPAY_LOGIN_MODE").as_deref() {
            Err(std::env::VarError::NotPresent) | Ok("off") => return Ok(None),
            Ok("sandbox") => {}
            _ => return Err("Invalid sandbox login mode"),
        }
        let origin = std::env::var("DEYPAY_LOGIN_ORIGIN")
            .ok()
            .and_then(|s| BrowserOrigin::parse(&s))
            .ok_or("Invalid sandbox login origin")?;
        let number = |key| {
            std::env::var(key)
                .ok()
                .and_then(|s| s.parse::<NonZeroU32>().ok())
                .ok_or("Missing or invalid sandbox login limit")
        };
        let seconds =
            |key| i32::try_from(number(key)?.get()).map_err(|_| "Invalid sandbox session lifetime");
        let lifetime = SessionLifetime::new(
            seconds("DEYPAY_SESSION_IDLE_SECONDS")?,
            seconds("DEYPAY_SESSION_ABSOLUTE_SECONDS")?,
        )
        .ok_or("Invalid sandbox session lifetime")?;
        let limits = LoginLimits::new(
            number("DEYPAY_LOGIN_REQUESTS_PER_MINUTE")?,
            number("DEYPAY_LOGIN_PER_IDENTIFIER_PER_MINUTE")?,
        );
        Ok(Self::new(
            environment,
            PgSessions(pool),
            crate::registration_config::sandbox_hasher()?,
            origin,
            lifetime,
            limits,
        ))
    }
}

pub fn routes(auth: Option<&Arc<CustomerAuth>>) -> Router {
    match auth {
        Some(auth) => auth.routes(),
        None => {
            let closed = post(|| async { reply(StatusCode::SERVICE_UNAVAILABLE) });
            Router::new()
                .route("/api/v1/auth/login", closed.clone())
                .route("/api/v1/auth/refresh", closed)
        }
    }
}

impl<R, V> SandboxAuth<R, V>
where
    R: LoginStore + SessionStore + 'static,
    V: CredentialVerifier + Send + 'static,
{
    pub fn new(
        environment: Environment,
        store: R,
        verifier: V,
        origin: BrowserOrigin,
        lifetime: SessionLifetime,
        limits: LoginLimits,
    ) -> Option<Arc<Self>> {
        matches!(environment, Environment::Local | Environment::Test).then(|| {
            Arc::new(Self {
                store,
                verifier,
                origin,
                lifetime,
                limits,
            })
        })
    }

    pub fn routes(self: &Arc<Self>) -> Router {
        Router::new()
            .route("/api/v1/auth/login", post(Self::submit))
            .route("/api/v1/auth/refresh", post(Self::refresh))
            .with_state(self.clone())
    }

    /// Apply after registering customer routes. Extracting AuthenticatedCustomer
    /// on a route without this layer fails closed.
    pub fn protect(self: &Arc<Self>, routes: Router) -> Router {
        routes.route_layer(middleware::from_fn_with_state(
            self.clone(),
            Self::authenticate,
        ))
    }

    async fn submit(State(auth): State<Arc<Self>>, req: Request) -> Response {
        if !auth.origin.permits(req.method(), req.headers()) {
            return reply(StatusCode::FORBIDDEN);
        }
        if let Err(e) = auth.limits.admit(None) {
            return failure(e);
        }
        if !req
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| {
                v.split(';')
                    .next()
                    .is_some_and(|m| m.trim().eq_ignore_ascii_case("application/json"))
            })
        {
            return reply(StatusCode::BAD_REQUEST);
        }
        let Ok(previous) = request_token(req.headers()) else {
            return reply(StatusCode::UNAUTHORIZED);
        };
        let Ok(body) = to_bytes(req.into_body(), MAX_PASSWORD_BYTES).await else {
            return reply(StatusCode::BAD_REQUEST);
        };
        let Ok(input) = serde_json::from_slice::<LoginRequest>(&body) else {
            return reply(StatusCode::BAD_REQUEST);
        };
        drop(body);
        let email = input.identifier.trim();
        if !valid_email(email) || input.password.is_empty() {
            return reply(StatusCode::BAD_REQUEST);
        };
        if let Err(e) = auth.limits.admit(Some(&email.to_lowercase())) {
            return failure(e);
        }
        let Ok(token) = SessionToken::generate() else {
            return reply(StatusCode::SERVICE_UNAVAILABLE);
        };
        match login(
            &auth.store,
            &auth.verifier,
            email,
            &input.password,
            &token.digest(),
            previous.map(|t| t.digest()).as_deref(),
            auth.lifetime,
        )
        .await
        {
            Ok(()) => success(&token),
            Err(CredentialError::Rejected) => (
                StatusCode::UNAUTHORIZED, [(header::CONTENT_TYPE, "application/json"),(header::CACHE_CONTROL, "no-store")],
                r#"{"error":{"code":"INVALID_CREDENTIALS","message":"Invalid credentials","details":{}}}"#,
            ).into_response(),
            Err(e) => failure(e),
        }
    }

    async fn refresh(State(auth): State<Arc<Self>>, req: Request) -> Response {
        if !auth.origin.permits(req.method(), req.headers()) {
            return reply(StatusCode::FORBIDDEN);
        }
        let Ok(Some(current)) = request_token(req.headers()) else {
            return reply(StatusCode::UNAUTHORIZED);
        };
        let Ok(next) = SessionToken::generate() else {
            return reply(StatusCode::SERVICE_UNAVAILABLE);
        };
        match auth
            .store
            .validate(
                &current.digest(),
                Some(&next.digest()),
                auth.lifetime.idle_seconds(),
            )
            .await
        {
            Ok(Some(_)) => success(&next),
            Ok(None) => reply(StatusCode::UNAUTHORIZED),
            Err(e) => failure(e),
        }
    }

    async fn authenticate(State(auth): State<Arc<Self>>, mut req: Request, next: Next) -> Response {
        if !auth.origin.permits(req.method(), req.headers()) {
            return reply(StatusCode::FORBIDDEN);
        }
        let Ok(Some(token)) = request_token(req.headers()) else {
            return reply(StatusCode::UNAUTHORIZED);
        };
        match auth
            .store
            .validate(&token.digest(), None, auth.lifetime.idle_seconds())
            .await
        {
            Ok(Some(id)) => {
                req.extensions_mut().insert(AuthenticatedCustomer(id));
                next.run(req).await
            }
            Ok(None) => reply(StatusCode::UNAUTHORIZED),
            Err(e) => failure(e),
        }
    }
}

fn success(token: &SessionToken) -> Response {
    let mut response = reply(StatusCode::OK);
    response
        .headers_mut()
        .insert(header::SET_COOKIE, session_cookie(token));
    response
}

fn failure(error: CredentialError) -> Response {
    reply(match error {
        CredentialError::Busy => StatusCode::TOO_MANY_REQUESTS,
        CredentialError::Unavailable => {
            tracing::warn!(
                event = "auth_dependency_unavailable",
                "Authentication unavailable"
            );
            StatusCode::SERVICE_UNAVAILABLE
        }
        CredentialError::Rejected => StatusCode::UNAUTHORIZED,
    })
}

fn reply(status: StatusCode) -> Response {
    let body = match status {
        StatusCode::OK => r#"{"data":{}}"#,
        StatusCode::BAD_REQUEST => {
            r#"{"error":{"code":"VALIDATION_ERROR","message":"Invalid request","details":{}}}"#
        }
        StatusCode::FORBIDDEN => {
            r#"{"error":{"code":"VALIDATION_ERROR","message":"Request denied","details":{}}}"#
        }
        StatusCode::TOO_MANY_REQUESTS => {
            r#"{"error":{"code":"RATE_LIMITED","message":"Try again later","details":{}}}"#
        }
        StatusCode::SERVICE_UNAVAILABLE => {
            r#"{"error":{"code":"SERVICE_UNAVAILABLE","message":"Authentication unavailable","details":{}}}"#
        }
        _ => {
            r#"{"error":{"code":"AUTHENTICATION_REQUIRED","message":"Authentication required","details":{}}}"#
        }
    };
    (
        status,
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        body,
    )
        .into_response()
}
