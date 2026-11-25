//! Customer profile APIs (API §15, IMP-P3-010) and the owner-approved
//! profile-image extension.
//!
//! Mount these routes only through `auth.protect(routes(..))`. Every handler
//! takes the customer from `AuthenticatedCustomer`; no path, query, header or
//! body field can name a customer. State-changing methods inherit the session
//! guard's exact-origin check.
use crate::login::AuthenticatedCustomer;
use axum::{
    Router,
    body::{Body, to_bytes},
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{any, get},
};
use deypay_application::profile::{
    AccountStatus, CustomerProfile, ImageMetadata, ImageNormalizer, ImageRejection, ImageRemoval,
    MAX_CONCURRENT_IMAGE_JOBS, MAX_UPLOAD_BYTES, Outcome, PreferenceChange, Preferences,
    ProfileError, ProfileImageStore, ProfileStore, RenameError, StoredImage, UploadFormat, rename,
};
use deypay_infrastructure::{
    database::DatabasePool, profile_images::JpegNormalizer, profiles::PgProfiles,
};
use serde_json::{Map, Value, json};
use std::{sync::Arc, time::Duration};
use tokio::sync::Semaphore;
use uuid::Uuid;

const ME: &str = "/api/v1/me";
const PREFERENCES: &str = "/api/v1/me/preferences";
const STATUS: &str = "/api/v1/me/status";
const IMAGE: &str = "/api/v1/me/profile-image";
const MAX_JSON_BYTES: usize = 16 * 1024;
/// How long an upload waits for one of the bounded decoding slots.
const IMAGE_SLOT_WAIT: Duration = Duration::from_secs(5);
/// A slow sender cannot hold a decoding slot longer than this.
const UPLOAD_READ_TIMEOUT: Duration = Duration::from_secs(30);

pub struct ProfileApi {
    store: PgProfiles,
    normalizer: Arc<JpegNormalizer>,
    image_slots: Arc<Semaphore>,
}

impl ProfileApi {
    pub fn new(pool: DatabasePool) -> Arc<Self> {
        Arc::new(Self {
            store: PgProfiles(pool),
            normalizer: Arc::new(JpegNormalizer),
            image_slots: Arc::new(Semaphore::new(MAX_CONCURRENT_IMAGE_JOBS)),
        })
    }
}

type Api = State<Arc<ProfileApi>>;

/// Customer routes. Unprotected use fails closed: the handlers' principal
/// extractor rejects every request that the session guard did not admit.
pub fn routes(api: Arc<ProfileApi>) -> Router {
    Router::new()
        .route(ME, get(me).patch(update_me))
        .route(PREFERENCES, get(preferences).patch(update_preferences))
        .route(STATUS, get(status))
        .route(IMAGE, get(image).put(upload).delete(remove))
        .with_state(api)
}

/// Mounted instead of [`routes`] when customer authentication is disabled.
pub fn unavailable() -> Router {
    let closed = any(|| async {
        error(
            StatusCode::SERVICE_UNAVAILABLE,
            "SERVICE_UNAVAILABLE",
            "Profile unavailable",
            json!({}),
        )
    });
    [ME, PREFERENCES, STATUS, IMAGE]
        .into_iter()
        .fold(Router::new(), |router, path| {
            router.route(path, closed.clone())
        })
}

fn json_response(status: StatusCode, body: Value) -> Response {
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

fn error(status: StatusCode, code: &str, message: &str, details: Value) -> Response {
    json_response(
        status,
        json!({"error":{"code":code,"message":message,"details":details}}),
    )
}

fn invalid_request() -> Response {
    error(
        StatusCode::BAD_REQUEST,
        "VALIDATION_ERROR",
        "Invalid request",
        json!({}),
    )
}

/// The account can no longer hold a customer session.
fn unauthenticated() -> Response {
    error(
        StatusCode::UNAUTHORIZED,
        "AUTHENTICATION_REQUIRED",
        "Authentication required",
        json!({}),
    )
}

fn failure(problem: ProfileError) -> Response {
    match problem {
        ProfileError::Unavailable => {
            tracing::warn!(event = "profile_store_unavailable", "Profile unavailable");
            error(
                StatusCode::SERVICE_UNAVAILABLE,
                "SERVICE_UNAVAILABLE",
                "Profile unavailable",
                json!({}),
            )
        }
        ProfileError::Rejected => error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "VALIDATION_ERROR",
            "Invalid profile update",
            json!({}),
        ),
        ProfileError::Corrupt => {
            tracing::error!(event = "profile_integrity_failure", "Profile data invalid");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "Internal error",
                json!({}),
            )
        }
    }
}

fn image_json(image: &ImageMetadata) -> Value {
    json!({
        "version": image.version.to_string(),
        "content_type": "image/jpeg",
        "width": image.width,
        "height": image.height,
        "updated_at": image.updated_at,
    })
}

fn profile_json(customer: Uuid, profile: &CustomerProfile) -> Value {
    json!({"data":{
        "id": customer.to_string(),
        "username": profile.username,
        "first_name": profile.first_name,
        "last_name": profile.last_name,
        "email": profile.email,
        "email_verified": profile.email_verified,
        "phone": profile.phone,
        "phone_verified": profile.phone_verified,
        "country_code": profile.country_code,
        "account_status": profile.account_status.public_name(),
        "names_editable": profile.names_editable,
        "profile_image": profile.image.as_ref().map(image_json),
        "created_at": profile.created_at,
        "updated_at": profile.updated_at,
    }})
}

fn preferences_json(preferences: Preferences, updated_at: Option<String>) -> Value {
    json!({"data":{
        "phone_discoverable": preferences.phone_discoverable,
        "email_notifications": preferences.email_notifications,
        "push_notifications": preferences.push_notifications,
        "marketing_consent": preferences.marketing_consent,
        "updated_at": updated_at,
    }})
}

fn single_header(headers: &HeaderMap, name: header::HeaderName) -> Option<&str> {
    let mut values = headers.get_all(name).iter();
    match (values.next(), values.next()) {
        (Some(value), None) => value.to_str().ok(),
        _ => None,
    }
}

/// A non-empty JSON object of at most 16 KiB with an `application/json` type.
/// Anything else is an invalid request.
async fn json_object(req: Request) -> Option<Map<String, Value>> {
    let is_json = single_header(req.headers(), header::CONTENT_TYPE)
        .and_then(|v| v.split(';').next())
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"));
    if !is_json {
        return None;
    }
    let bytes = to_bytes(req.into_body(), MAX_JSON_BYTES).await.ok()?;
    match serde_json::from_slice(&bytes) {
        Ok(Value::Object(fields)) if !fields.is_empty() => Some(fields),
        _ => None,
    }
}

async fn me(State(api): Api, customer: AuthenticatedCustomer) -> Response {
    match api.store.profile(customer.id()).await {
        Ok(Some(profile)) => json_response(StatusCode::OK, profile_json(customer.id(), &profile)),
        Ok(None) => failure(ProfileError::Corrupt),
        Err(problem) => failure(problem),
    }
}

/// Only `first_name` and `last_name` are editable. Any other field, including
/// identity, contact, status, KYC or security fields, makes the request invalid.
async fn update_me(State(api): Api, customer: AuthenticatedCustomer, req: Request) -> Response {
    let Some(fields) = json_object(req).await else {
        return invalid_request();
    };
    let (mut first, mut last) = (None, None);
    for (key, value) in &fields {
        let slot = match key.as_str() {
            "first_name" => &mut first,
            "last_name" => &mut last,
            _ => return invalid_request(),
        };
        let Value::String(text) = value else {
            return invalid_request();
        };
        *slot = Some(text.as_str());
    }
    match rename(&api.store, customer.id(), first, last).await {
        Ok(profile) => json_response(StatusCode::OK, profile_json(customer.id(), &profile)),
        Err(RenameError::Invalid {
            first_name,
            last_name,
        }) => {
            let invalid: Vec<_> = [("first_name", first_name), ("last_name", last_name)]
                .into_iter()
                .filter_map(|(name, bad)| bad.then_some(name))
                .collect();
            error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "VALIDATION_ERROR",
                "Invalid profile update",
                json!({"fields": invalid}),
            )
        }
        Err(RenameError::Store(Outcome::NamesLocked)) => error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "VALIDATION_ERROR",
            "Names can no longer be changed",
            json!({"reason": "names_locked"}),
        ),
        Err(RenameError::Store(Outcome::Ineligible)) => unauthenticated(),
        Err(RenameError::Store(_)) => failure(ProfileError::Corrupt),
        Err(RenameError::Failed(problem)) => failure(problem),
    }
}

async fn preferences(State(api): Api, customer: AuthenticatedCustomer) -> Response {
    match api.store.preferences(customer.id()).await {
        Ok(Some((preferences, updated_at))) => {
            json_response(StatusCode::OK, preferences_json(preferences, updated_at))
        }
        Ok(None) => failure(ProfileError::Corrupt),
        Err(problem) => failure(problem),
    }
}

async fn update_preferences(
    State(api): Api,
    customer: AuthenticatedCustomer,
    req: Request,
) -> Response {
    let Some(fields) = json_object(req).await else {
        return invalid_request();
    };
    let mut change = PreferenceChange::default();
    for (key, value) in &fields {
        let slot = match key.as_str() {
            "phone_discoverable" => &mut change.phone_discoverable,
            "email_notifications" => &mut change.email_notifications,
            "push_notifications" => &mut change.push_notifications,
            "marketing_consent" => &mut change.marketing_consent,
            _ => return invalid_request(),
        };
        let Value::Bool(flag) = value else {
            return invalid_request();
        };
        *slot = Some(*flag);
    }
    match api.store.update_preferences(customer.id(), change).await {
        Ok(Outcome::Updated) => preferences(State(api), customer).await,
        Ok(Outcome::PhoneUnverified) => error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "VALIDATION_ERROR",
            "Phone discovery requires a verified phone",
            json!({"reason": "phone_unverified"}),
        ),
        Ok(Outcome::Ineligible) => unauthenticated(),
        Ok(Outcome::NamesLocked) => failure(ProfileError::Corrupt),
        Err(problem) => failure(problem),
    }
}

/// The status admitted with the session, read from `identity.users` by the
/// session guard in this request. It is not a KYC decision or financial authorization.
async fn status(customer: AuthenticatedCustomer) -> Response {
    match AccountStatus::parse(customer.account_status()) {
        Some(status) => json_response(
            StatusCode::OK,
            json!({"data":{"account_status": status.public_name()}}),
        ),
        None => failure(ProfileError::Corrupt),
    }
}

fn image_rejected(status: StatusCode, message: &str, reason: &str) -> Response {
    error(
        status,
        "VALIDATION_ERROR",
        message,
        json!({"reason": reason}),
    )
}

fn unsupported_type() -> Response {
    image_rejected(
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "Unsupported image type",
        "unsupported_type",
    )
}

fn too_large() -> Response {
    image_rejected(
        StatusCode::PAYLOAD_TOO_LARGE,
        "Image too large",
        "too_large",
    )
}

/// Uploads, normalizes and stores the image, replacing any current one.
async fn upload(State(api): Api, customer: AuthenticatedCustomer, req: Request) -> Response {
    let Some(format) = single_header(req.headers(), header::CONTENT_TYPE)
        .and_then(UploadFormat::from_content_type)
    else {
        return unsupported_type();
    };
    let declared =
        single_header(req.headers(), header::CONTENT_LENGTH).and_then(|v| v.parse::<u64>().ok());
    if declared.is_some_and(|length| length > MAX_UPLOAD_BYTES as u64) {
        return too_large();
    }
    // Take a slot before buffering, so at most MAX_CONCURRENT_IMAGE_JOBS
    // uploads hold body or decoder memory at once.
    let Ok(Ok(slot)) =
        tokio::time::timeout(IMAGE_SLOT_WAIT, api.image_slots.clone().acquire_owned()).await
    else {
        return error(
            StatusCode::SERVICE_UNAVAILABLE,
            "SERVICE_UNAVAILABLE",
            "Image processing busy",
            json!({"reason": "busy"}),
        );
    };
    let bytes = match tokio::time::timeout(
        UPLOAD_READ_TIMEOUT,
        to_bytes(req.into_body(), MAX_UPLOAD_BYTES),
    )
    .await
    {
        Ok(Ok(bytes)) => bytes,
        // The body exceeded the limit or the client failed to send it.
        Ok(Err(_)) => return too_large(),
        Err(_) => {
            return image_rejected(
                StatusCode::BAD_REQUEST,
                "Upload incomplete",
                "incomplete_upload",
            );
        }
    };
    let normalizer = api.normalizer.clone();
    let normalized =
        tokio::task::spawn_blocking(move || normalizer.normalize(format, &bytes)).await;
    drop(slot);
    let image = match normalized {
        Ok(Ok(image)) => image,
        Ok(Err(ImageRejection::TypeMismatch)) => return unsupported_type(),
        Ok(Err(ImageRejection::TooManyPixels)) => {
            return image_rejected(
                StatusCode::UNPROCESSABLE_ENTITY,
                "Image dimensions too large",
                "too_many_pixels",
            );
        }
        Ok(Err(ImageRejection::TooComplex)) => {
            return image_rejected(
                StatusCode::UNPROCESSABLE_ENTITY,
                "Image cannot be stored within the size limit",
                "too_complex",
            );
        }
        Ok(Err(ImageRejection::Undecodable)) => {
            return image_rejected(
                StatusCode::UNPROCESSABLE_ENTITY,
                "Image cannot be decoded",
                "undecodable",
            );
        }
        Err(_) => {
            tracing::warn!(
                event = "profile_image_decoder_failed",
                "Image decoder failed"
            );
            return image_rejected(
                StatusCode::UNPROCESSABLE_ENTITY,
                "Image cannot be decoded",
                "undecodable",
            );
        }
    };
    match api.store.store_image(customer.id(), &image).await {
        Ok(Some(stored)) => json_response(
            StatusCode::OK,
            json!({"data":{"profile_image": image_json(&stored)}}),
        ),
        Ok(None) => unauthenticated(),
        Err(problem) => failure(problem),
    }
}

/// The first entity tag in `If-None-Match` that names an image version.
fn known_version(headers: &HeaderMap) -> Option<Uuid> {
    single_header(headers, header::IF_NONE_MATCH)?
        .split(',')
        .find_map(|tag| {
            let tag = tag.trim();
            let tag = tag.strip_prefix("W/").unwrap_or(tag);
            tag.strip_prefix('"')?.strip_suffix('"')?.parse().ok()
        })
}

/// Serves only the customer's own re-encoded JPEG. Browsers revalidate on
/// each use, and shared caches never store it.
async fn image(State(api): Api, customer: AuthenticatedCustomer, headers: HeaderMap) -> Response {
    let (status, version, body) = match api
        .store
        .image(customer.id(), known_version(&headers))
        .await
    {
        Ok(Some(StoredImage {
            version,
            jpeg: Some(jpeg),
        })) => (StatusCode::OK, version, Some(jpeg)),
        Ok(Some(StoredImage {
            version,
            jpeg: None,
        })) => (StatusCode::NOT_MODIFIED, version, None),
        Ok(None) => {
            return error(
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "No profile image",
                json!({}),
            );
        }
        Err(problem) => return failure(problem),
    };
    let etag = HeaderValue::from_str(&format!("\"{version}\"")).expect("UUID entity tag");
    let mut response = match body {
        Some(jpeg) => (
            status,
            [
                (header::CONTENT_TYPE, "image/jpeg"),
                (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
                (
                    header::CONTENT_SECURITY_POLICY,
                    "default-src 'none'; sandbox",
                ),
                (header::CONTENT_DISPOSITION, "inline"),
            ],
            Body::from(jpeg),
        )
            .into_response(),
        None => status.into_response(),
    };
    let headers = response.headers_mut();
    headers.insert(header::ETAG, etag);
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-cache"),
    );
    headers.insert(header::VARY, HeaderValue::from_static("Cookie"));
    headers.insert(
        "cross-origin-resource-policy",
        HeaderValue::from_static("same-site"),
    );
    response
}

/// Idempotent: removing an absent image also succeeds.
async fn remove(State(api): Api, customer: AuthenticatedCustomer) -> Response {
    match api.store.remove_image(customer.id()).await {
        Ok(ImageRemoval::Removed | ImageRemoval::Absent) => (
            StatusCode::NO_CONTENT,
            [(header::CACHE_CONTROL, "no-store")],
        )
            .into_response(),
        Ok(ImageRemoval::Ineligible) => unauthenticated(),
        Err(problem) => failure(problem),
    }
}
