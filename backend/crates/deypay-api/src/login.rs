//! Mount customer handlers through `auth.protect(routes)` and extract `AuthenticatedCustomer`.
//! The principal contains an authoritative identity and account status at request admission.
//! Recheck action-specific authorization in the transaction performing the protected operation.
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
    login::{LoginStore, SessionIdentity, SessionStore, login},
    passwords::{CredentialError, CredentialVerifier, MAX_PASSWORD_BYTES},
    registration::valid_email,
};
use deypay_common::config::Environment;
use deypay_infrastructure::{
    database as sqlx_pool,
    password_hashing::Argon2idHasher,
    sessions::{PgSessionReader, PgSessions, SessionToken, environment_name},
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

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

pub struct BrowserAuth<R, V> {
    guard: Arc<SessionGuard<R>>,
    verifier: V,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LoginRequest {
    identifier: String,
    password: String,
}

impl Drop for LoginRequest {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.password.zeroize();
    }
}
pub struct SessionGuard<R> {
    store: Arc<R>,
    origin: BrowserOrigin,
}
pub type CustomerAuth = SessionGuard<PgSessionReader>;
pub type VerifierAuth = BrowserAuth<PgSessions, Argon2idHasher>;

impl CustomerAuth {
    pub fn from_env(
        environment: Environment,
        pool: sqlx_pool::DatabasePool,
    ) -> Result<Option<Arc<Self>>, &'static str> {
        if std::env::var_os("DEYPAY_AUTH_SIGNING_KEY_FILE").is_some()
            || std::env::var_os("DEYPAY_AUTH_DATABASE_URL").is_some()
        {
            return Err("Verifier secrets must be isolated from the general API");
        }
        match std::env::var("DEYPAY_LOGIN_MODE").as_deref() {
            Err(std::env::VarError::NotPresent) | Ok("off") => return Ok(None),
            Ok("enabled") => {}
            Ok("sandbox") if matches!(environment, Environment::Local | Environment::Test) => {}
            Ok("sandbox") => return Ok(None),
            _ => return Err("Invalid authentication mode"),
        }
        let release = std::env::var("DEYPAY_AUTH_RELEASE")
            .ok()
            .and_then(|v| v.parse().ok())
            .ok_or("Missing authentication release")?;
        let origin = std::env::var("DEYPAY_LOGIN_ORIGIN").map_err(|_| "Missing browser origin")?;
        let browser = BrowserOrigin::parse(&origin).ok_or("Invalid browser origin")?;
        Ok(Some(Arc::new(Self {
            store: Arc::new(PgSessionReader {
                pool,
                release,
                environment: environment_name(environment),
                origin,
            }),
            origin: browser,
        })))
    }
    pub async fn ready(&self) -> bool {
        self.store.ready().await
    }
}
impl VerifierAuth {
    pub async fn ready(&self) -> bool {
        self.guard.store.ready().await
    }
    pub async fn from_env(
        settings: &deypay_common::config::Settings,
    ) -> Result<Arc<Self>, &'static str> {
        if std::env::var("DEYPAY_LOGIN_MODE").as_deref() != Ok("enabled") {
            return Err("Authentication verifier requires explicit enablement");
        }
        let (store, policy) = PgSessions::from_env(settings).await?;
        let browser =
            BrowserOrigin::parse(&policy.origin).ok_or("Invalid approved browser origin")?;
        let [memory, iterations, lanes, slots] = policy.hash_costs;
        let hasher = Argon2idHasher::new(memory, iterations, lanes, slots as usize)
            .map_err(|_| "Invalid verifier hash budget")?;
        Ok(Self::new(store, hasher, browser))
    }
}

/// Credential traffic must be routed directly to the isolated verifier listener.
pub fn routes(_: Option<&Arc<CustomerAuth>>) -> Router {
    let closed = post(|| async { reply(StatusCode::SERVICE_UNAVAILABLE) });
    Router::new()
        .route("/api/v1/auth/login", closed.clone())
        .route("/api/v1/auth/refresh", closed)
}

impl<R: SessionStore + 'static> SessionGuard<R> {
    pub fn protect(self: &Arc<Self>, routes: Router) -> Router {
        routes.route_layer(middleware::from_fn_with_state(
            self.clone(),
            Self::authenticate,
        ))
    }
    async fn authenticate(State(auth): State<Arc<Self>>, mut req: Request, next: Next) -> Response {
        if !auth.origin.permits(req.method(), req.headers()) {
            return reply(StatusCode::FORBIDDEN);
        }
        let Ok(Some(token)) = request_token(req.headers()) else {
            return reply(StatusCode::UNAUTHORIZED);
        };
        match auth.store.validate(&token.digest(), None).await {
            Ok(Some(id)) => {
                req.extensions_mut().insert(AuthenticatedCustomer(id));
                next.run(req).await
            }
            Ok(None) => reply(StatusCode::UNAUTHORIZED),
            Err(e) => failure(e),
        }
    }
}
impl<R, V> BrowserAuth<R, V>
where
    R: LoginStore + SessionStore + 'static,
    V: CredentialVerifier + Send + 'static,
{
    pub fn new(store: R, verifier: V, origin: BrowserOrigin) -> Arc<Self> {
        Arc::new(Self {
            guard: Arc::new(SessionGuard {
                store: Arc::new(store),
                origin,
            }),
            verifier,
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
        self.guard.protect(routes)
    }

    async fn submit(State(auth): State<Arc<Self>>, req: Request) -> Response {
        if !auth.guard.origin.permits(req.method(), req.headers()) {
            return reply(StatusCode::FORBIDDEN);
        }
        if let Err(e) = auth.guard.store.admit(None).await {
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
        if let Err(e) = auth.guard.store.admit(Some(email)).await {
            return failure(e);
        }
        let Ok(token) = SessionToken::generate() else {
            return reply(StatusCode::SERVICE_UNAVAILABLE);
        };
        match login(
            &*auth.guard.store,
            &auth.verifier,
            email,
            &input.password,
            &token.digest(),
            previous.map(|t| t.digest()).as_deref(),
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
        if !auth.guard.origin.permits(req.method(), req.headers()) {
            return reply(StatusCode::FORBIDDEN);
        }
        let Ok(Some(current)) = request_token(req.headers()) else {
            return reply(StatusCode::UNAUTHORIZED);
        };
        let Ok(next) = SessionToken::generate() else {
            return reply(StatusCode::SERVICE_UNAVAILABLE);
        };
        match auth
            .guard
            .store
            .validate(&current.digest(), Some(&next.digest()))
            .await
        {
            Ok(Some(_)) => success(&next),
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
