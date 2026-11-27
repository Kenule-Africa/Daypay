//! Internal transfer APIs (API §§16.2, 20, 28; IMP-P7-001–009).
//!
//! Mount only through `auth.protect(routes(..))`. Every handler takes the
//! customer from `AuthenticatedCustomer`; authorization and transfer calls also
//! pass the digest of the validated session cookie, so a confirmation and its
//! single-use token stay bound to the session that created them. Amounts are
//! exact decimal strings at the asset's accounting precision, never rounded.
//! Transfers stay refused while the shipped `NoApprovedPolicy` gate is in
//! force and until the ledger's `CUSTOMER_TRANSFER` activation exists.
use crate::login::AuthenticatedCustomer;
use crate::profile::{error, json_object, json_response};
use crate::sessions::request_token;
use axum::{
    Router,
    extract::{Path, RawQuery, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, header::HeaderName},
    response::Response,
    routing::{any, get, post},
};
use deypay_application::ledger::NoApprovedPolicy;
use deypay_application::transfers::{
    AuthorizationChallenge, Executed, TransferError, TransferInput, TransferRecord,
    TransferService, public_status,
};
use deypay_infrastructure::{
    database::DatabasePool,
    transfers::{PgTransfers, generate_proof_token},
};
use percent_encoding::percent_decode_str;
use serde_json::{Map, Value, json};
use std::sync::Arc;
use uuid::Uuid;

const RESOLVE: &str = "/api/v1/recipients/resolve";
const AUTHORIZATIONS: &str = "/api/v1/transaction-authorizations";
const CONFIRM: &str = "/api/v1/transaction-authorizations/{challenge_id}/confirm";
const INTERNAL: &str = "/api/v1/transfers/internal";
const TRANSACTIONS: &str = "/api/v1/transactions";
const TRANSACTION: &str = "/api/v1/transactions/{transaction_id}";
const IDEMPOTENCY_KEY: &str = "idempotency-key";
const DEFAULT_PAGE: u32 = 20;
/// Ample for `username=` and a percent-encoded 31-byte handle.
const MAX_QUERY_BYTES: usize = 256;

pub struct TransferApi<G = NoApprovedPolicy> {
    service: TransferService<PgTransfers, G>,
}

impl TransferApi {
    /// The pool's login is the restricted API role (`deypay_app`).
    pub fn new(pool: DatabasePool) -> Arc<Self> {
        Self::with_gate(pool, NoApprovedPolicy)
    }
}

impl<G: deypay_application::ledger::FinancialGate> TransferApi<G> {
    /// Use [`TransferApi::new`] in deployed code: only an approved financial
    /// policy may replace the shipped gate.
    pub fn with_gate(pool: DatabasePool, gate: G) -> Arc<Self> {
        Arc::new(Self {
            service: TransferService::new(PgTransfers::new(pool), gate),
        })
    }
}

type Api<G> = State<Arc<TransferApi<G>>>;

pub fn routes<G>(api: Arc<TransferApi<G>>) -> Router
where
    G: deypay_application::ledger::FinancialGate + Send + Sync + 'static,
{
    Router::new()
        .route(RESOLVE, get(resolve::<G>))
        .route(AUTHORIZATIONS, post(authorize::<G>))
        .route(CONFIRM, post(confirm::<G>))
        .route(INTERNAL, post(transfer::<G>))
        .route(TRANSACTIONS, get(list::<G>))
        .route(TRANSACTION, get(detail::<G>))
        .with_state(api)
}

/// Mounted instead of [`routes`] when customer authentication is disabled.
pub fn unavailable() -> Router {
    let closed = any(|| async { unavailable_response() });
    [
        RESOLVE,
        AUTHORIZATIONS,
        CONFIRM,
        INTERNAL,
        TRANSACTIONS,
        TRANSACTION,
    ]
    .into_iter()
    .fold(Router::new(), |router, path| {
        router.route(path, closed.clone())
    })
}

fn unavailable_response() -> Response {
    error(
        StatusCode::SERVICE_UNAVAILABLE,
        "SERVICE_UNAVAILABLE",
        "Transfers unavailable",
        json!({}),
    )
}

fn invalid() -> Response {
    error(
        StatusCode::BAD_REQUEST,
        "VALIDATION_ERROR",
        "Invalid request",
        json!({}),
    )
}

fn failure(problem: TransferError) -> Response {
    let (status, code, message, details) = match problem {
        TransferError::InvalidRequest => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "VALIDATION_ERROR",
            "Invalid request",
            json!({}),
        ),
        TransferError::InvalidAmount => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "INVALID_AMOUNT",
            "Enter a positive amount with no more decimal places than the asset allows",
            json!({}),
        ),
        TransferError::UnsupportedAsset => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "UNSUPPORTED_ASSET",
            "Asset not supported for transfers",
            json!({}),
        ),
        TransferError::RecipientNotFound => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "RECIPIENT_NOT_FOUND",
            "Recipient not found",
            json!({}),
        ),
        TransferError::RecipientUnavailable => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "RECIPIENT_UNAVAILABLE",
            "This recipient cannot receive the transfer",
            json!({}),
        ),
        TransferError::SelfTransfer => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "SELF_TRANSFER_NOT_PERMITTED",
            "You cannot send to yourself",
            json!({}),
        ),
        TransferError::KycRequired => (
            StatusCode::FORBIDDEN,
            "KYC_REQUIRED",
            "Identity verification is required to send funds",
            json!({}),
        ),
        TransferError::Restricted => (
            StatusCode::FORBIDDEN,
            "ACCOUNT_RESTRICTED",
            "This account cannot send funds",
            json!({}),
        ),
        TransferError::AuthorizationInvalid => (
            StatusCode::FORBIDDEN,
            "AUTHORIZATION_INVALID",
            "This authorization is no longer valid; review and confirm the transfer again",
            json!({}),
        ),
        TransferError::IdempotencyConflict => (
            StatusCode::CONFLICT,
            "IDEMPOTENCY_CONFLICT",
            "This idempotency key was used for a different request",
            json!({}),
        ),
        TransferError::NotEnabled => (
            StatusCode::SERVICE_UNAVAILABLE,
            "SERVICE_UNAVAILABLE",
            "Transfers are not available yet",
            json!({"reason": "transfers_not_enabled"}),
        ),
        TransferError::AssetUnavailable => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "ASSET_UNAVAILABLE",
            "This asset is temporarily unavailable",
            json!({}),
        ),
        TransferError::InsufficientFunds => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "INSUFFICIENT_FUNDS",
            "Insufficient available balance",
            json!({}),
        ),
        TransferError::LimitExceeded => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "TRANSACTION_LIMIT_EXCEEDED",
            "The amount is above the allowed limit",
            json!({}),
        ),
        TransferError::LookupLimited => (
            StatusCode::TOO_MANY_REQUESTS,
            "RATE_LIMITED",
            "Too many recipient lookups; try again later",
            json!({}),
        ),
        TransferError::LookupUnavailable => (
            StatusCode::SERVICE_UNAVAILABLE,
            "SERVICE_UNAVAILABLE",
            "Recipient lookup is not available yet",
            json!({"reason": "recipient_lookup_not_enabled"}),
        ),
        TransferError::SessionInvalid => (
            StatusCode::UNAUTHORIZED,
            "AUTHENTICATION_REQUIRED",
            "Authentication required",
            json!({}),
        ),
        TransferError::NotFound => (
            StatusCode::NOT_FOUND,
            "TRANSACTION_NOT_FOUND",
            "Transaction not found",
            json!({}),
        ),
        TransferError::Busy => (
            StatusCode::SERVICE_UNAVAILABLE,
            "SERVICE_UNAVAILABLE",
            "Busy; retry the same request",
            json!({"retryable": true}),
        ),
        TransferError::Unavailable => {
            tracing::warn!(
                event = "transfer_store_unavailable",
                "Transfers unavailable"
            );
            (
                StatusCode::SERVICE_UNAVAILABLE,
                "SERVICE_UNAVAILABLE",
                "Transfers unavailable; retry with the same Idempotency-Key",
                json!({"retryable": true}),
            )
        }
        TransferError::Corrupt => {
            tracing::error!(
                event = "transfer_integrity_failure",
                "Transfer request failed"
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "Internal error",
                json!({}),
            )
        }
    };
    error(status, code, message, details)
}

/// The validated session cookie's digest. The guard has already accepted it.
fn session_digest(headers: &HeaderMap) -> Option<String> {
    request_token(headers)
        .ok()
        .flatten()
        .map(|token| token.digest())
}

/// Exactly one `username` parameter, percent-decoded.
fn username_param(query: Option<&str>) -> Option<String> {
    let query = query.filter(|q| q.len() <= MAX_QUERY_BYTES)?;
    let (key, value) = query.split_once('=')?;
    if key != "username" || value.contains(['&', '=', ';']) {
        return None;
    }
    percent_decode_str(&value.replace('+', " "))
        .decode_utf8()
        .ok()
        .map(|v| v.into_owned())
}

async fn resolve<G>(
    State(api): Api<G>,
    customer: AuthenticatedCustomer,
    RawQuery(query): RawQuery,
) -> Response
where
    G: deypay_application::ledger::FinancialGate,
{
    let Some(username) = username_param(query.as_deref()) else {
        return invalid();
    };
    match api.service.resolve(customer.id(), &username).await {
        Ok(Some(recipient)) => json_response(
            StatusCode::OK,
            json!({"data": {
                "recipient_id": recipient.id.to_string(),
                "username": recipient.username,
                "display_name": recipient.display_name,
                "can_receive": recipient.can_receive,
            }}),
        ),
        Ok(None) => error(
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
            "Recipient not found",
            json!({}),
        ),
        Err(TransferError::InvalidRequest) => error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "VALIDATION_ERROR",
            "Invalid username",
            json!({"fields": ["username"]}),
        ),
        Err(problem) => failure(problem),
    }
}

/// The transfer terms of a request body; `extra` names the other allowed fields.
struct Terms {
    recipient: Uuid,
    asset: String,
    amount: String,
    description: Option<String>,
}

fn terms(fields: &Map<String, Value>, extra: &[&str]) -> Option<Terms> {
    let allowed = ["recipient_id", "asset", "amount", "description"];
    if fields
        .keys()
        .any(|key| !allowed.contains(&key.as_str()) && !extra.contains(&key.as_str()))
    {
        return None;
    }
    let text = |name: &str| fields.get(name).and_then(Value::as_str);
    let description = match fields.get("description") {
        None | Some(Value::Null) => None,
        Some(Value::String(text)) => Some(text.clone()),
        Some(_) => return None,
    };
    Some(Terms {
        recipient: Uuid::try_parse(text("recipient_id")?).ok()?,
        asset: text("asset")?.to_owned(),
        amount: text("amount")?.to_owned(),
        description,
    })
}

impl Terms {
    fn input(&self) -> TransferInput<'_> {
        TransferInput {
            recipient: self.recipient,
            asset: &self.asset,
            amount: &self.amount,
            description: self.description.as_deref(),
        }
    }
}

fn challenge_json(challenge: &AuthorizationChallenge) -> Value {
    json!({
        "challenge_id": challenge.id.to_string(),
        "operation_type": "internal_transfer",
        "expires_at": challenge.expires_at,
        "verification_method": challenge.verification_method.to_ascii_lowercase(),
        "terms": {
            "recipient": {
                "id": challenge.recipient.to_string(),
                "username": challenge.recipient_username,
                "display_name": challenge.recipient_display_name,
            },
            "asset": challenge.asset,
            "amount": challenge.format(challenge.amount),
            "fee": challenge.format(challenge.fee),
            "total": challenge.amount.checked_add(challenge.fee).map(|total| challenge.format(total)),
            "description": challenge.description,
        },
    })
}

/// Body: `operation_type` "internal_transfer", `recipient_id`, `asset`,
/// `amount` and an optional `description`. Returns 201 with the challenge and
/// the normalized terms to display.
async fn authorize<G>(State(api): Api<G>, customer: AuthenticatedCustomer, req: Request) -> Response
where
    G: deypay_application::ledger::FinancialGate,
{
    let Some(session) = session_digest(req.headers()) else {
        return failure(TransferError::SessionInvalid);
    };
    let Some(fields) = json_object(req).await else {
        return invalid();
    };
    if fields.get("operation_type").and_then(Value::as_str) != Some("internal_transfer") {
        return invalid();
    }
    let Some(terms) = terms(&fields, &["operation_type"]) else {
        return invalid();
    };
    match api
        .service
        .authorize(customer.id(), &session, terms.input())
        .await
    {
        Ok(challenge) => json_response(
            StatusCode::CREATED,
            json!({"data": challenge_json(&challenge)}),
        ),
        Err(problem) => failure(problem),
    }
}

/// Body: exactly `{"confirm": true}`. Returns the single-use token once.
async fn confirm<G>(
    State(api): Api<G>,
    customer: AuthenticatedCustomer,
    Path(id): Path<String>,
    req: Request,
) -> Response
where
    G: deypay_application::ledger::FinancialGate,
{
    let Some(session) = session_digest(req.headers()) else {
        return failure(TransferError::SessionInvalid);
    };
    let Some(fields) = json_object(req).await else {
        return invalid();
    };
    if fields.len() != 1 || fields.get("confirm") != Some(&Value::Bool(true)) {
        return invalid();
    }
    // Unknown and malformed challenges are indistinguishable from expired ones.
    let Ok(challenge) = Uuid::try_parse(&id) else {
        return failure(TransferError::AuthorizationInvalid);
    };
    let token = match generate_proof_token() {
        Ok(token) => token,
        Err(problem) => return failure(problem),
    };
    match api
        .service
        .confirm(customer.id(), &session, challenge, token)
        .await
    {
        Ok(proof) => json_response(
            StatusCode::OK,
            json!({"data": {
                "authorization_token": proof.token,
                "expires_at": proof.expires_at,
            }}),
        ),
        Err(problem) => failure(problem),
    }
}

fn transfer_json(executed: &Executed) -> Value {
    let record = &executed.transfer;
    json!({
        "transaction_id": record.id.to_string(),
        "reference": record.reference,
        "type": "internal_transfer",
        "status": public_status(&record.status),
        "asset": record.asset,
        "amount": record.format(record.amount),
        "fee": record.format(record.fee),
        "description": record.description,
        "recipient": counterparty_json(record),
        "created_at": record.created_at,
        "completed_at": record.completed_at,
    })
}

/// Header `Idempotency-Key` (8–128 of `A-Z a-z 0-9 _ . : -`) and body
/// `recipient_id`, `authorization_token`, `asset`, `amount`, optional
/// `description`. A repeated identical request returns the original transfer
/// with `Idempotency-Replayed: true`.
async fn transfer<G>(State(api): Api<G>, customer: AuthenticatedCustomer, req: Request) -> Response
where
    G: deypay_application::ledger::FinancialGate,
{
    let Some(session) = session_digest(req.headers()) else {
        return failure(TransferError::SessionInvalid);
    };
    let mut keys = req.headers().get_all(IDEMPOTENCY_KEY).iter();
    let key = match (keys.next().and_then(|v| v.to_str().ok()), keys.next()) {
        (Some(key), None) => key.to_owned(),
        _ => return invalid(),
    };
    let Some(fields) = json_object(req).await else {
        return invalid();
    };
    let Some(terms) = terms(&fields, &["authorization_token"]) else {
        return invalid();
    };
    let Some(token) = fields.get("authorization_token").and_then(Value::as_str) else {
        return invalid();
    };
    match api
        .service
        .execute(customer.id(), &session, &key, token, terms.input())
        .await
    {
        Ok(executed) => {
            let mut response = json_response(
                StatusCode::CREATED,
                json!({"data": transfer_json(&executed)}),
            );
            if executed.replayed {
                response.headers_mut().insert(
                    HeaderName::from_static("idempotency-replayed"),
                    HeaderValue::from_static("true"),
                );
            }
            response
        }
        Err(problem) => failure(problem),
    }
}

fn counterparty_json(record: &TransferRecord) -> Value {
    json!({
        "id": record.counterparty.id.to_string(),
        "username": record.counterparty.username,
        "display_name": record.counterparty.display_name,
    })
}

/// API §28.4 with the viewer's direction; the recipient sees no fee.
fn transaction_json(record: &TransferRecord) -> Value {
    let amount = record.format(record.amount);
    json!({
        "id": record.id.to_string(),
        "reference": record.reference,
        "type": "internal_transfer",
        "status": public_status(&record.status),
        "direction": record.direction.public_name(),
        "source_asset": record.asset,
        "source_amount": amount,
        "destination_asset": record.asset,
        "destination_amount": amount,
        "fee": record.format(record.fee),
        "description": record.description,
        "counterparty": counterparty_json(record),
        "created_at": record.created_at,
        "completed_at": record.completed_at,
    })
}

/// `limit` (1–100, default 20) and `cursor` (the last `id` of the previous
/// page), each at most once; nothing else.
fn page_query(query: Option<&str>) -> Option<(Option<Uuid>, u32)> {
    let (mut cursor, mut limit) = (None, None);
    for part in query.unwrap_or("").split('&').filter(|p| !p.is_empty()) {
        match part.split_once('=')? {
            ("cursor", value) if cursor.is_none() => cursor = Some(Uuid::try_parse(value).ok()?),
            ("limit", value) if limit.is_none() => {
                let parsed: u32 = value.parse().ok()?;
                if value.starts_with('0') || !(1..=100).contains(&parsed) {
                    return None;
                }
                limit = Some(parsed);
            }
            _ => return None,
        }
    }
    Some((cursor, limit.unwrap_or(DEFAULT_PAGE)))
}

async fn list<G>(
    State(api): Api<G>,
    customer: AuthenticatedCustomer,
    RawQuery(query): RawQuery,
) -> Response
where
    G: deypay_application::ledger::FinancialGate,
{
    let Some((cursor, limit)) = page_query(query.as_deref()) else {
        return invalid();
    };
    match api.service.history(customer.id(), cursor, limit).await {
        Ok(records) => {
            let mut next = None;
            if let Some(last) = records.last().filter(|_| records.len() == limit as usize) {
                match api.service.history(customer.id(), Some(last.id), 1).await {
                    Ok(older) if !older.is_empty() => next = Some(last.id.to_string()),
                    Ok(_) => {}
                    Err(problem) => return failure(problem),
                }
            }
            let data: Vec<Value> = records.iter().map(transaction_json).collect();
            json_response(
                StatusCode::OK,
                json!({"data": data, "meta": {"next_cursor": next}}),
            )
        }
        Err(TransferError::InvalidRequest) => invalid(),
        Err(problem) => failure(problem),
    }
}

async fn detail<G>(
    State(api): Api<G>,
    customer: AuthenticatedCustomer,
    Path(id): Path<String>,
) -> Response
where
    G: deypay_application::ledger::FinancialGate,
{
    // Unknown, foreign and malformed identifiers get the same 404.
    let Ok(id) = Uuid::try_parse(&id) else {
        return failure(TransferError::NotFound);
    };
    match api.service.transaction(customer.id(), id).await {
        Ok(record) => json_response(StatusCode::OK, json!({"data": transaction_json(&record)})),
        Err(problem) => failure(problem),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_queries_are_strict() {
        let id = Uuid::from_u128(5);
        assert_eq!(page_query(None), Some((None, DEFAULT_PAGE)));
        assert_eq!(page_query(Some("limit=5")), Some((None, 5)));
        assert_eq!(
            page_query(Some(&format!("cursor={id}&limit=100"))),
            Some((Some(id), 100))
        );
        for bad in [
            "limit=0",
            "limit=101",
            "limit=05",
            "limit=-1",
            "limit=5&limit=6",
            "cursor=nope",
            "page=2",
            "limit",
        ] {
            assert_eq!(page_query(Some(bad)), None, "{bad}");
        }
    }

    #[test]
    fn username_queries_are_single_and_decoded() {
        assert_eq!(
            username_param(Some("username=%40Ada")).as_deref(),
            Some("@Ada")
        );
        assert_eq!(username_param(Some("username=a&x=1")), None);
        assert_eq!(username_param(Some("name=ada")), None);
        assert_eq!(username_param(None), None);
    }

    #[test]
    fn transfer_bodies_allow_only_their_fields() {
        let body = |value: Value| value.as_object().unwrap().clone();
        let id = Uuid::from_u128(9).to_string();
        let ok = body(json!({"recipient_id": id, "asset": "USDT", "amount": "1.5"}));
        let parsed = terms(&ok, &[]).unwrap();
        assert_eq!(
            (parsed.asset.as_str(), parsed.amount.as_str()),
            ("USDT", "1.5")
        );
        assert!(parsed.description.is_none());
        assert!(
            terms(
                &body(json!({"recipient_id": id, "asset": "USDT", "amount": 1})),
                &[]
            )
            .is_none()
        );
        assert!(
            terms(
                &body(json!({"recipient_id": id, "asset": "USDT", "amount": "1", "fee": "0"})),
                &[]
            )
            .is_none()
        );
        assert!(
            terms(
                &body(json!({"recipient_id": "bob", "asset": "USDT", "amount": "1"})),
                &[]
            )
            .is_none()
        );
        assert!(
            terms(
                &body(
                    json!({"recipient_id": id, "asset": "USDT", "amount": "1", "description": 5})
                ),
                &[]
            )
            .is_none()
        );
    }
}
