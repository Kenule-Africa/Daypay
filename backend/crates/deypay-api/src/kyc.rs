//! Customer KYC API (IMP-P4-005, API §17).
//!
//! Every route takes the customer from a [`CustomerAuthenticator`], never from
//! a request field, header or path. No authenticator exists yet: login and
//! session validation are Phase 3 work (IMP-P3-005/006). Without one, every
//! route answers 503 before reading the request, so the API binary serves no
//! operational KYC route.
//!
//! With an authenticator:
//!
//! - `GET /api/v1/kyc/status` returns the customer's own case.
//! - `POST /api/v1/kyc/start` opens the customer's first case in `DRAFT`.
//! - `POST /api/v1/kyc/submit` submits it, but no submission requirements are
//!   approved, so a submittable case is refused with 503
//!   ([`NoApprovedRequirements`]).
//! - `POST /api/v1/kyc/details` and `POST /api/v1/kyc/documents/upload-request`
//!   always answer 503 without reading the body: no identity fields, document
//!   storage, upload authorization or retention policy is approved.
//!
//! Before an authenticator is wired in, these must also be in place: approved
//! public status mapping, compliance audit events (IMP-P4-008) and the
//! `kyc-grants.psql` runtime grants.
//!
//! Responses never include reviewer notes, provider data or documents.
//! `financial_access_enabled` is always `false`: no financial operation is
//! enabled, and no eligibility policy is approved (IMP-P4-006).
use axum::{
    Router,
    body::{Body, to_bytes},
    extract::{Request, State},
    http::{HeaderMap, StatusCode, header, request::Parts},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use deypay_application::kyc::{
    CustomerKycCases, KycCaseStatus, KycCaseStore, KycCaseView, KycServiceError,
    KycTransitionError, NoApprovedRequirements, current_case, open_first_case, submit_current_case,
};
use std::{future::Future, pin::Pin, sync::Arc};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthenticationFailure {
    /// No valid session: missing, unknown, expired or revoked.
    Unauthenticated,
    /// Sessions could not be checked.
    Unavailable,
}

pub type AuthenticationFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Uuid, AuthenticationFailure>> + Send + 'a>>;

/// Resolves the customer behind a request from trusted server-side session
/// state.
///
/// An implementation must validate the opaque session cookie against the
/// stored session, reject expired and revoked sessions (API §6.1) and apply
/// CSRF protection to state-changing requests (API §6.2). It must never trust
/// a customer ID supplied by the client. This is the KYC routes' requirement
/// only; the Phase 3 session work owns the shared authentication type, which
/// should implement or replace this port.
pub trait CustomerAuthenticator: Send + Sync {
    fn authenticate<'a>(&'a self, request: &'a Parts) -> AuthenticationFuture<'a>;
}

struct Endpoint<S> {
    store: S,
    authenticator: Option<Arc<dyn CustomerAuthenticator>>,
}

/// Mounts the five API §17 routes over `store`.
pub fn routes<S, T>(store: S, authenticator: Option<Arc<dyn CustomerAuthenticator>>) -> Router<T>
where
    S: KycCaseStore + CustomerKycCases + Send + Sync + 'static,
    T: Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/api/v1/kyc/status", get(status::<S>))
        .route("/api/v1/kyc/start", post(start::<S>))
        .route("/api/v1/kyc/details", post(unavailable::<S>))
        .route(
            "/api/v1/kyc/documents/upload-request",
            post(unavailable::<S>),
        )
        .route("/api/v1/kyc/submit", post(submit::<S>))
        .with_state(Arc::new(Endpoint {
            store,
            authenticator,
        }))
}

type Shared<S> = State<Arc<Endpoint<S>>>;

async fn status<S>(State(endpoint): Shared<S>, request: Request) -> Response
where
    S: KycCaseStore + CustomerKycCases + Send + Sync,
{
    let (parts, _) = request.into_parts();
    let customer = match authenticate(&endpoint, &parts).await {
        Ok(customer) => customer,
        Err(refusal) => return refusal.respond(),
    };
    match current_case(&endpoint.store, customer).await {
        Ok(Some(view)) => case(StatusCode::OK, view),
        Ok(None) => error(StatusCode::NOT_FOUND, "NOT_FOUND", "No KYC case"),
        Err(failure) => service_error(failure),
    }
}

async fn start<S>(State(endpoint): Shared<S>, request: Request) -> Response
where
    S: KycCaseStore + CustomerKycCases + Send + Sync,
{
    let (parts, body) = request.into_parts();
    let customer = match authenticate(&endpoint, &parts).await {
        Ok(customer) => customer,
        Err(refusal) => return refusal.respond(),
    };
    if !empty_command(&parts.headers, body).await {
        return invalid_request();
    }
    match open_first_case(&endpoint.store, customer).await {
        Ok(view) => case(StatusCode::CREATED, view),
        Err(failure) => service_error(failure),
    }
}

async fn submit<S>(State(endpoint): Shared<S>, request: Request) -> Response
where
    S: KycCaseStore + CustomerKycCases + Send + Sync,
{
    let (parts, body) = request.into_parts();
    let customer = match authenticate(&endpoint, &parts).await {
        Ok(customer) => customer,
        Err(refusal) => return refusal.respond(),
    };
    if !empty_command(&parts.headers, body).await {
        return invalid_request();
    }
    match submit_current_case(&endpoint.store, &NoApprovedRequirements, customer).await {
        Ok(view) => case(StatusCode::OK, view),
        Err(failure) => service_error(failure),
    }
}

/// Details and document uploads: authenticated, then refused without reading
/// the body, because nothing they would accept is approved.
async fn unavailable<S>(State(endpoint): Shared<S>, request: Request) -> Response
where
    S: Send + Sync,
{
    let (parts, _) = request.into_parts();
    if let Err(refusal) = authenticate(&endpoint, &parts).await {
        return refusal.respond();
    }
    error(
        StatusCode::SERVICE_UNAVAILABLE,
        "SERVICE_UNAVAILABLE",
        "KYC operation unavailable",
    )
}

/// Why a request was refused before any KYC work. Kept small so that refusals
/// are cheap to return; [`Refusal::respond`] builds the response.
#[derive(Debug, Clone, Copy)]
enum Refusal {
    /// No authenticator is configured, or sessions could not be checked.
    Unavailable,
    Unauthenticated,
}

impl Refusal {
    fn respond(self) -> Response {
        match self {
            Self::Unavailable => error(
                StatusCode::SERVICE_UNAVAILABLE,
                "SERVICE_UNAVAILABLE",
                "KYC unavailable",
            ),
            Self::Unauthenticated => error(
                StatusCode::UNAUTHORIZED,
                "AUTHENTICATION_REQUIRED",
                "Authentication required",
            ),
        }
    }
}

async fn authenticate<S>(endpoint: &Endpoint<S>, parts: &Parts) -> Result<Uuid, Refusal> {
    let Some(authenticator) = &endpoint.authenticator else {
        return Err(Refusal::Unavailable);
    };
    authenticator
        .authenticate(parts)
        .await
        .map_err(|failure| match failure {
            AuthenticationFailure::Unauthenticated => Refusal::Unauthenticated,
            AuthenticationFailure::Unavailable => Refusal::Unavailable,
        })
}

/// Start and submit take no input (API §17). An empty body or an empty JSON
/// object is accepted; anything else, such as a customer or case ID, is not.
async fn empty_command(headers: &HeaderMap, body: Body) -> bool {
    let Ok(bytes) = to_bytes(body, 1024).await else {
        return false;
    };
    if bytes.is_empty() {
        return true;
    }
    let json = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"));
    json && matches!(
        serde_json::from_slice::<serde_json::Value>(&bytes),
        Ok(serde_json::Value::Object(fields)) if fields.is_empty()
    )
}

/// API §17.7 values for the case statuses whose names match exactly.
/// `INFORMATION_REQUESTED` has no API value, and when `not_started` applies is
/// undecided, so neither is mapped until Product and Compliance approve it.
const fn public_status(status: KycCaseStatus) -> Option<&'static str> {
    match status {
        KycCaseStatus::Draft => Some("draft"),
        KycCaseStatus::Submitted => Some("submitted"),
        KycCaseStatus::PendingProvider => Some("pending_provider"),
        KycCaseStatus::ManualReview => Some("manual_review"),
        KycCaseStatus::Approved => Some("approved"),
        KycCaseStatus::Rejected => Some("rejected"),
        KycCaseStatus::Expired => Some("expired"),
        KycCaseStatus::InformationRequested => None,
    }
}

fn case(status: StatusCode, view: KycCaseView) -> Response {
    let Some(public) = public_status(view.status) else {
        return error(
            StatusCode::SERVICE_UNAVAILABLE,
            "SERVICE_UNAVAILABLE",
            "KYC status unavailable",
        );
    };
    let body = serde_json::json!({
        "data": {
            "kyc_case_id": view.case_id.to_string(),
            "status": public,
            "financial_access_enabled": false,
        }
    });
    respond(status, body.to_string())
}

fn service_error(failure: KycServiceError) -> Response {
    use KycServiceError as E;
    match failure {
        E::NotFound | E::Rejected(KycTransitionError::Unauthorized { .. }) => {
            error(StatusCode::NOT_FOUND, "NOT_FOUND", "No KYC case")
        }
        E::CaseAlreadyExists => error(
            StatusCode::CONFLICT,
            "DUPLICATE_REQUEST",
            "KYC already started",
        ),
        E::Rejected(KycTransitionError::StaleStatus { .. }) | E::ConcurrentModification => {
            error(StatusCode::CONFLICT, "VALIDATION_ERROR", "KYC case changed")
        }
        E::Rejected(_) => error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "VALIDATION_ERROR",
            "KYC case cannot be submitted",
        ),
        E::SubmissionIncomplete => error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "VALIDATION_ERROR",
            "KYC submission incomplete",
        ),
        E::SubmissionRequirementsUndefined => error(
            StatusCode::SERVICE_UNAVAILABLE,
            "SERVICE_UNAVAILABLE",
            "KYC submission unavailable",
        ),
        E::Unavailable => error(
            StatusCode::SERVICE_UNAVAILABLE,
            "SERVICE_UNAVAILABLE",
            "KYC unavailable",
        ),
        E::InvalidRecord => {
            tracing::warn!("KYC case record failed validation");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "KYC unavailable",
            )
        }
    }
}

fn invalid_request() -> Response {
    error(
        StatusCode::BAD_REQUEST,
        "VALIDATION_ERROR",
        "Invalid KYC request",
    )
}

fn error(status: StatusCode, code: &str, message: &str) -> Response {
    let body = serde_json::json!({
        "error": { "code": code, "message": message, "details": {} }
    });
    respond(status, body.to_string())
}

fn respond(status: StatusCode, body: String) -> Response {
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
