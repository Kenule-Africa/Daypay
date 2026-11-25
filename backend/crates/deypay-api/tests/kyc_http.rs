//! KYC HTTP contract (IMP-P4-005) over an in-memory store. The PostgreSQL store
//! is exercised by the infrastructure `persistence` suite.
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header, request::Parts},
};
use deypay_api::kyc::{AuthenticationFailure, AuthenticationFuture, CustomerAuthenticator, routes};
use deypay_application::kyc::{
    CustomerKycCases, KycCase, KycCaseStore, KycCaseTransition, NewKycCase,
};
use deypay_application::repositories::{KycRepository, Lookup, RepositoryError};
use serde_json::Value;
use std::future::{Future, ready};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;
use uuid::Uuid;

const ALICE: Uuid = Uuid::from_u128(0xA11CE);
const BOB: Uuid = Uuid::from_u128(0xB0B);

/// Stands in for the Phase 3 session check: an opaque cookie maps to a
/// customer held server-side.
struct Sessions(Result<(), AuthenticationFailure>);

impl CustomerAuthenticator for Sessions {
    fn authenticate<'a>(&'a self, request: &'a Parts) -> AuthenticationFuture<'a> {
        let session = request
            .headers
            .get(header::COOKIE)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("session="));
        let result = self.0.and(match session {
            Some("alice-session") => Ok(ALICE),
            Some("bob-session") => Ok(BOB),
            _ => Err(AuthenticationFailure::Unauthenticated),
        });
        Box::pin(ready(result))
    }
}

#[derive(Default)]
struct Inner {
    cases: Mutex<Vec<KycCase>>,
    calls: AtomicUsize,
    writes: AtomicUsize,
    failure: Option<RepositoryError>,
    /// Another request commits this status right after the first read.
    changes_after_read: Mutex<Option<&'static str>>,
}

#[derive(Clone, Default)]
struct Store(Arc<Inner>);

impl Store {
    fn failing(error: RepositoryError) -> Self {
        Self(Arc::new(Inner {
            failure: Some(error),
            ..Inner::default()
        }))
    }

    fn with(owner: Uuid, status: &str) -> (Self, Uuid) {
        let store = Self::default();
        let id = Uuid::from_u128(0xCA5E);
        store.0.cases.lock().unwrap().push(KycCase {
            id,
            user_id: owner,
            status: status.to_owned(),
        });
        (store, id)
    }

    fn calls(&self) -> usize {
        self.0.calls.load(Ordering::SeqCst)
    }

    fn statuses(&self) -> Vec<(Uuid, String)> {
        let cases = self.0.cases.lock().unwrap();
        cases
            .iter()
            .map(|c| (c.user_id, c.status.clone()))
            .collect()
    }

    fn enter(&self) -> Result<(), RepositoryError> {
        self.0.calls.fetch_add(1, Ordering::SeqCst);
        self.0.failure.map_or(Ok(()), Err)
    }
}

impl KycRepository for Store {
    fn kyc_case(
        &self,
        customer_id: Uuid,
        case_id: Uuid,
    ) -> impl Future<Output = Lookup<KycCase>> + Send {
        let found = self.enter().map(|()| {
            let cases = self.0.cases.lock().unwrap();
            cases
                .iter()
                .find(|c| c.id == case_id && c.user_id == customer_id)
                .cloned()
        });
        ready(found)
    }
}

impl CustomerKycCases for Store {
    fn customer_kyc_case(&self, customer_id: Uuid) -> impl Future<Output = Lookup<KycCase>> + Send {
        let found = self.enter().map(|()| {
            let mut cases = self.0.cases.lock().unwrap();
            let case = cases.iter_mut().find(|c| c.user_id == customer_id)?;
            let seen = case.clone();
            if let Some(status) = self.0.changes_after_read.lock().unwrap().take() {
                case.status = status.to_owned();
            }
            Some(seen)
        });
        ready(found)
    }
}

impl KycCaseStore for Store {
    fn insert_first_case(
        &self,
        request: NewKycCase,
    ) -> impl Future<Output = Lookup<KycCase>> + Send {
        let inserted = self.enter().map(|()| {
            let mut cases = self.0.cases.lock().unwrap();
            if cases.iter().any(|c| c.user_id == request.owner_id()) {
                return None;
            }
            self.0.writes.fetch_add(1, Ordering::SeqCst);
            let case = KycCase {
                id: Uuid::from_u128(0x1000 + cases.len() as u128),
                user_id: request.owner_id(),
                status: request.status().as_str().to_owned(),
            };
            cases.push(case.clone());
            Some(case)
        });
        ready(inserted)
    }

    fn apply_transition(
        &self,
        owner_id: Uuid,
        case_id: Uuid,
        transition: KycCaseTransition,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send {
        let applied = self.enter().map(|()| {
            let mut cases = self.0.cases.lock().unwrap();
            let Some(case) = cases.iter_mut().find(|c| {
                c.id == case_id && c.user_id == owner_id && c.status == transition.from().as_str()
            }) else {
                return false;
            };
            self.0.writes.fetch_add(1, Ordering::SeqCst);
            case.status = transition.to().as_str().to_owned();
            true
        });
        ready(applied)
    }
}

fn app(store: &Store, auth: Option<Result<(), AuthenticationFailure>>) -> Router {
    routes(
        store.clone(),
        auth.map(|result| Arc::new(Sessions(result)) as Arc<dyn CustomerAuthenticator>),
    )
}

fn signed_in(store: &Store) -> Router {
    app(store, Some(Ok(())))
}

const ROUTES: [(&str, &str); 5] = [
    ("GET", "/api/v1/kyc/status"),
    ("POST", "/api/v1/kyc/start"),
    ("POST", "/api/v1/kyc/details"),
    ("POST", "/api/v1/kyc/documents/upload-request"),
    ("POST", "/api/v1/kyc/submit"),
];

fn request(method: &str, path: &str, session: Option<&str>, body: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(path);
    if let Some(session) = session {
        builder = builder.header(header::COOKIE, format!("session={session}"));
    }
    match body {
        Some(body) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_owned())),
        None => builder.body(Body::empty()),
    }
    .unwrap()
}

/// Sends a request and returns the status and JSON body, checking the shared
/// response headers.
async fn send(app: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = app.clone().oneshot(request).await.unwrap();
    let headers = response.headers();
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    assert_eq!(headers[header::CONTENT_TYPE], "application/json");
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

async fn call(app: &Router, method: &str, path: &str, session: &str) -> (StatusCode, Value) {
    send(app, request(method, path, Some(session), None)).await
}

fn code(body: &Value) -> &str {
    body["error"]["code"].as_str().unwrap()
}

#[tokio::test]
async fn without_an_authenticator_every_route_is_unavailable() {
    let (store, _) = Store::with(ALICE, "DRAFT");
    let app = app(&store, None);
    for (method, path) in ROUTES {
        for body in [
            None,
            Some(r#"{"customer_id":"00000000-0000-0000-0000-000000000001"}"#),
        ] {
            let (status, body) =
                send(&app, request(method, path, Some("alice-session"), body)).await;
            assert_eq!(
                (status, code(&body)),
                (StatusCode::SERVICE_UNAVAILABLE, "SERVICE_UNAVAILABLE"),
                "{path}"
            );
        }
    }
    assert_eq!(store.calls(), 0);
}

#[tokio::test]
async fn unauthenticated_requests_are_refused_before_any_work() {
    let (store, _) = Store::with(ALICE, "DRAFT");
    let router = signed_in(&store);
    for (method, path) in ROUTES {
        // No session, an unknown session, or a customer ID offered by the client.
        let forged = Request::builder()
            .method(method)
            .uri(path)
            .header("x-customer-id", ALICE.to_string())
            .header(header::AUTHORIZATION, format!("Bearer {ALICE}"))
            .body(Body::empty())
            .unwrap();
        for request in [
            request(method, path, None, None),
            request(method, path, Some("guessed"), None),
            forged,
        ] {
            let (status, body) = send(&router, request).await;
            assert_eq!(
                (status, code(&body)),
                (StatusCode::UNAUTHORIZED, "AUTHENTICATION_REQUIRED"),
                "{path}"
            );
        }
    }
    let unavailable = app(&store, Some(Err(AuthenticationFailure::Unavailable)));
    for (method, path) in ROUTES {
        let (status, _) = call(&unavailable, method, path, "alice-session").await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{path}");
    }
    assert_eq!(store.calls(), 0);
    assert_eq!(store.statuses(), [(ALICE, "DRAFT".to_owned())]);
}

#[tokio::test]
async fn customers_start_and_see_only_their_own_case() {
    let store = Store::default();
    let app = signed_in(&store);
    let (status, body) = call(&app, "GET", "/api/v1/kyc/status", "alice-session").await;
    assert_eq!((status, code(&body)), (StatusCode::NOT_FOUND, "NOT_FOUND"));

    let (status, started) = call(&app, "POST", "/api/v1/kyc/start", "alice-session").await;
    assert_eq!(status, StatusCode::CREATED);
    let case_id = started["data"]["kyc_case_id"].as_str().unwrap().to_owned();
    assert_eq!(
        started,
        serde_json::json!({"data": {"kyc_case_id": case_id, "status": "draft", "financial_access_enabled": false}})
    );
    let (status, body) = call(&app, "POST", "/api/v1/kyc/start", "alice-session").await;
    assert_eq!(
        (status, code(&body)),
        (StatusCode::CONFLICT, "DUPLICATE_REQUEST")
    );
    let (status, body) = call(&app, "GET", "/api/v1/kyc/status", "alice-session").await;
    assert_eq!((status, body), (StatusCode::OK, started));

    // Bob neither sees nor submits Alice's case.
    let (status, body) = call(&app, "GET", "/api/v1/kyc/status", "bob-session").await;
    assert_eq!((status, code(&body)), (StatusCode::NOT_FOUND, "NOT_FOUND"));
    assert!(!body.to_string().contains(&case_id));
    let (status, body) = call(&app, "POST", "/api/v1/kyc/submit", "bob-session").await;
    assert_eq!((status, code(&body)), (StatusCode::NOT_FOUND, "NOT_FOUND"));
    let (status, bobs) = call(&app, "POST", "/api/v1/kyc/start", "bob-session").await;
    assert_eq!(status, StatusCode::CREATED);
    assert_ne!(bobs["data"]["kyc_case_id"], case_id.as_str());
    assert_eq!(
        store.statuses(),
        [(ALICE, "DRAFT".to_owned()), (BOB, "DRAFT".to_owned())]
    );
}

#[tokio::test]
async fn request_data_cannot_choose_the_customer_or_case() {
    let store = Store::default();
    let app = signed_in(&store);
    for path in ["/api/v1/kyc/start", "/api/v1/kyc/submit"] {
        for body in [
            r#"{"customer_id":"00000000-0000-0000-0000-000000000b0b"}"#,
            r#"{"kyc_case_id":"00000000-0000-0000-0000-00000000ca5e"}"#,
            r#"{"status":"approved"}"#,
            "not json",
            "[]",
        ] {
            let (status, response) = send(
                &app,
                request("POST", path, Some("alice-session"), Some(body)),
            )
            .await;
            assert_eq!(
                (status, code(&response)),
                (StatusCode::BAD_REQUEST, "VALIDATION_ERROR"),
                "{path} {body}"
            );
        }
        let wrong_type = Request::post(path)
            .header(header::COOKIE, "session=alice-session")
            .header(header::CONTENT_TYPE, "text/plain")
            .body(Body::from("{}"))
            .unwrap();
        let oversized = request(
            "POST",
            path,
            Some("alice-session"),
            Some(&format!("{{\"pad\":\"{}\"}}", "x".repeat(4096))),
        );
        for invalid in [wrong_type, oversized] {
            let (status, _) = send(&app, invalid).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{path}");
        }
    }
    assert_eq!(store.calls(), 0);

    // A forged customer header is ignored: the case belongs to the session.
    let forged = Request::post("/api/v1/kyc/start")
        .header(header::COOKIE, "session=alice-session")
        .header("x-customer-id", BOB.to_string())
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{}"))
        .unwrap();
    assert_eq!(send(&app, forged).await.0, StatusCode::CREATED);
    assert_eq!(store.statuses(), [(ALICE, "DRAFT".to_owned())]);
}

#[tokio::test]
async fn submission_fails_closed_without_approved_requirements() {
    for status in ["DRAFT", "INFORMATION_REQUESTED"] {
        let (store, _) = Store::with(ALICE, status);
        let app = signed_in(&store);
        let (code_status, body) = call(&app, "POST", "/api/v1/kyc/submit", "alice-session").await;
        assert_eq!(
            (code_status, code(&body), body["error"]["message"].as_str()),
            (
                StatusCode::SERVICE_UNAVAILABLE,
                "SERVICE_UNAVAILABLE",
                Some("KYC submission unavailable")
            )
        );
        assert_eq!(store.statuses(), [(ALICE, status.to_owned())]);
        assert_eq!(store.0.writes.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn invalid_and_stale_submissions_are_refused() {
    for status in [
        "SUBMITTED",
        "PENDING_PROVIDER",
        "MANUAL_REVIEW",
        "APPROVED",
        "REJECTED",
        "EXPIRED",
    ] {
        let (store, _) = Store::with(ALICE, status);
        let (code_status, body) = call(
            &signed_in(&store),
            "POST",
            "/api/v1/kyc/submit",
            "alice-session",
        )
        .await;
        assert_eq!(
            (code_status, code(&body)),
            (StatusCode::UNPROCESSABLE_ENTITY, "VALIDATION_ERROR"),
            "{status}"
        );
        assert_eq!(store.statuses(), [(ALICE, status.to_owned())]);
    }
    // The case changes between the customer's read and the service's read.
    let (store, _) = Store::with(ALICE, "DRAFT");
    *store.0.changes_after_read.lock().unwrap() = Some("INFORMATION_REQUESTED");
    let (status, body) = call(
        &signed_in(&store),
        "POST",
        "/api/v1/kyc/submit",
        "alice-session",
    )
    .await;
    assert_eq!(
        (status, code(&body)),
        (StatusCode::CONFLICT, "VALIDATION_ERROR")
    );
    assert_eq!(store.0.writes.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn details_and_documents_stay_disabled() {
    let (store, _) = Store::with(ALICE, "DRAFT");
    let app = signed_in(&store);
    for path in [
        "/api/v1/kyc/details",
        "/api/v1/kyc/documents/upload-request",
    ] {
        for body in [
            None,
            Some(r#"{"nin":"12345678901","document":"base64..."}"#),
            Some("not json"),
        ] {
            let (status, response) =
                send(&app, request("POST", path, Some("alice-session"), body)).await;
            assert_eq!(
                (status, code(&response)),
                (StatusCode::SERVICE_UNAVAILABLE, "SERVICE_UNAVAILABLE"),
                "{path}"
            );
        }
        let (status, _) = send(&app, request("POST", path, None, None)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
    assert_eq!(store.calls(), 0);
}

#[tokio::test]
async fn storage_failures_fail_closed() {
    for (error, status, expected) in [
        (
            RepositoryError::StorageUnavailable,
            StatusCode::SERVICE_UNAVAILABLE,
            "SERVICE_UNAVAILABLE",
        ),
        (
            RepositoryError::InvalidRecord,
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
        ),
    ] {
        let store = Store::failing(error);
        let app = signed_in(&store);
        for (method, path) in [
            ("GET", "/api/v1/kyc/status"),
            ("POST", "/api/v1/kyc/start"),
            ("POST", "/api/v1/kyc/submit"),
        ] {
            let (actual, body) = call(&app, method, path, "alice-session").await;
            assert_eq!((actual, code(&body)), (status, expected), "{path}");
        }
        assert!(store.statuses().is_empty());
    }
}

#[tokio::test]
async fn statuses_are_reported_without_invention_or_financial_access() {
    for (stored, public) in [
        ("DRAFT", Some("draft")),
        ("SUBMITTED", Some("submitted")),
        ("PENDING_PROVIDER", Some("pending_provider")),
        ("MANUAL_REVIEW", Some("manual_review")),
        ("APPROVED", Some("approved")),
        ("REJECTED", Some("rejected")),
        ("EXPIRED", Some("expired")),
        // No API value is approved for this status, so none is invented.
        ("INFORMATION_REQUESTED", None),
    ] {
        let (store, id) = Store::with(ALICE, stored);
        let (status, body) = call(
            &signed_in(&store),
            "GET",
            "/api/v1/kyc/status",
            "alice-session",
        )
        .await;
        match public {
            Some(public) => {
                assert_eq!(status, StatusCode::OK);
                assert_eq!(
                    body,
                    serde_json::json!({"data": {"kyc_case_id": id.to_string(), "status": public, "financial_access_enabled": false}})
                );
            }
            None => assert_eq!(
                (status, code(&body)),
                (StatusCode::SERVICE_UNAVAILABLE, "SERVICE_UNAVAILABLE")
            ),
        }
    }
}

#[tokio::test]
async fn no_route_decides_a_case() {
    let (store, _) = Store::with(ALICE, "MANUAL_REVIEW");
    let app = signed_in(&store);
    for _ in 0..2 {
        for (method, path) in ROUTES {
            for body in [None, Some(r#"{"status":"approved","decision":"approve"}"#)] {
                let _ = send(&app, request(method, path, Some("alice-session"), body)).await;
            }
        }
    }
    assert_eq!(store.statuses(), [(ALICE, "MANUAL_REVIEW".to_owned())]);
    assert_eq!(store.0.writes.load(Ordering::SeqCst), 0);
}
