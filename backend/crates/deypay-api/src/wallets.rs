//! Customer wallet APIs (API §§18–19, IMP-P6-006–009).
//!
//! Mount only through `auth.protect(routes(..))`. Every handler takes the
//! customer from `AuthenticatedCustomer`; a wallet is looked up only together
//! with its owner, and an unknown, foreign or malformed wallet identifier gets
//! the same 404. Balances come from the ledger, formatted as exact decimal
//! strings; nothing is substituted when the ledger is unavailable. Wallet
//! visibility never grants permission to move funds.
use crate::login::AuthenticatedCustomer;
use crate::profile::{error, json_object, json_response};
use axum::{
    Router,
    extract::{Path, RawQuery, Request, State},
    http::StatusCode,
    response::Response,
    routing::{any, get},
};
use deypay_application::ledger::NoApprovedPolicy;
use deypay_application::wallets::{
    AddressState, ChainFamily, DepositAddress, Restriction, SupportedNetwork, Wallet, WalletError,
    WalletNetwork, WalletService, WalletStatus,
};

use deypay_infrastructure::{database::DatabasePool, wallets::PgWallets};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

const WALLETS: &str = "/api/v1/wallets";
const WALLET: &str = "/api/v1/wallets/{wallet_id}";
const BALANCE: &str = "/api/v1/wallets/{wallet_id}/balance";
const DEPOSIT_ADDRESSES: &str = "/api/v1/wallets/{wallet_id}/deposit-addresses";
const NETWORKS: &str = "/api/v1/networks";
const ASSET_NETWORKS: &str = "/api/v1/assets/{asset}/networks";

pub struct WalletApi {
    service: WalletService<PgWallets, NoApprovedPolicy>,
}

impl WalletApi {
    /// The pool's login is the restricted API role (`deypay_app`).
    pub fn new(pool: DatabasePool) -> Arc<Self> {
        Arc::new(Self {
            service: WalletService::new(PgWallets::new(pool), NoApprovedPolicy),
        })
    }
}

type Api = State<Arc<WalletApi>>;

pub fn routes(api: Arc<WalletApi>) -> Router {
    Router::new()
        .route(WALLETS, get(list))
        .route(WALLET, get(detail))
        .route(BALANCE, get(balance))
        .route(
            DEPOSIT_ADDRESSES,
            get(deposit_address).post(request_address),
        )
        .route(NETWORKS, get(networks))
        .route(ASSET_NETWORKS, get(asset_networks))
        .with_state(api)
}

/// Mounted instead of [`routes`] when customer authentication is disabled.
pub fn unavailable() -> Router {
    let closed = any(|| async { unavailable_response() });
    [
        WALLETS,
        WALLET,
        BALANCE,
        DEPOSIT_ADDRESSES,
        NETWORKS,
        ASSET_NETWORKS,
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
        "Wallets unavailable",
        json!({}),
    )
}

fn not_found() -> Response {
    error(
        StatusCode::NOT_FOUND,
        "NOT_FOUND",
        "Wallet not found",
        json!({}),
    )
}

fn failure(problem: WalletError) -> Response {
    match problem {
        WalletError::NotFound => not_found(),
        WalletError::InvalidRequest => error(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "Invalid request",
            json!({}),
        ),
        WalletError::UnsupportedNetwork => error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "UNSUPPORTED_NETWORK",
            "Network not supported for this wallet",
            json!({}),
        ),
        WalletError::Restricted | WalletError::NotEligible => error(
            StatusCode::FORBIDDEN,
            "ACCOUNT_RESTRICTED",
            "This wallet cannot receive deposits",
            json!({}),
        ),
        WalletError::KycRequired => error(
            StatusCode::FORBIDDEN,
            "KYC_REQUIRED",
            "Identity verification is required for deposit addresses",
            json!({}),
        ),
        WalletError::DepositsNotEnabled => error(
            StatusCode::SERVICE_UNAVAILABLE,
            "SERVICE_UNAVAILABLE",
            "Deposits are not enabled for this asset and network",
            json!({"reason": "deposits_not_enabled"}),
        ),
        WalletError::ProviderUnavailable => error(
            StatusCode::SERVICE_UNAVAILABLE,
            "PROVIDER_UNAVAILABLE",
            "Deposit addresses are not available yet",
            json!({}),
        ),
        WalletError::Conflict => error(
            StatusCode::CONFLICT,
            "DUPLICATE_REQUEST",
            "The request conflicts with a request in progress",
            json!({}),
        ),
        WalletError::Unavailable => {
            tracing::warn!(event = "wallet_store_unavailable", "Wallets unavailable");
            unavailable_response()
        }
        WalletError::Corrupt
        | WalletError::InvalidTransition
        | WalletError::NotEmpty
        | WalletError::ApprovalRequired
        | WalletError::OperatorNotAuthenticated
        | WalletError::NotPermitted => {
            tracing::error!(event = "wallet_integrity_failure", problem = ?problem, "Wallet request failed");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "Internal error",
                json!({}),
            )
        }
    }
}

/// Malformed identifiers are indistinguishable from unknown ones.
fn wallet_id(text: &str) -> Option<Uuid> {
    Uuid::try_parse(text).ok()
}

fn family(value: ChainFamily) -> &'static str {
    match value {
        ChainFamily::Evm => "evm",
        ChainFamily::Solana => "solana",
    }
}

fn restrictions(wallet: &Wallet, policy_approved: bool) -> Vec<&'static str> {
    let mut names: Vec<&'static str> = wallet
        .restrictions
        .iter()
        .map(|r| r.public_name())
        .collect();
    let policy = Restriction::FinancialActivityNotEnabled.public_name();
    if !policy_approved && !names.contains(&policy) {
        names.push(policy);
    }
    names
}

fn balance_json(wallet: &Wallet) -> Value {
    wallet.balance.map_or(Value::Null, |balance| {
        json!({
            "available": wallet.format(balance.available),
            "reserved": wallet.format(balance.reserved),
            "total": wallet.format(balance.total),
        })
    })
}

fn wallet_json(wallet: &Wallet, policy_approved: bool) -> Value {
    json!({
        "id": wallet.id.to_string(),
        "asset": wallet.asset,
        "asset_name": wallet.asset_name,
        "status": wallet.effective_status().public_name(),
        "balance": balance_json(wallet),
        "restrictions": restrictions(wallet, policy_approved),
        "capabilities": {
            "deposit": wallet.deposit_networks > 0,
            "send": wallet.can_send(policy_approved),
        },
        "created_at": wallet.created_at,
        "updated_at": wallet.updated_at,
    })
}

fn network_json(network: &WalletNetwork) -> Value {
    let address = match &network.address {
        AddressState::Active { address, memo } => json!({"address": address, "memo": memo}),
        _ => Value::Null,
    };
    json!({
        "network": network.code,
        "name": network.name,
        "chain_family": family(network.chain_family),
        "token_contract": network.token_contract,
        "token_decimals": network.token_decimals,
        "memo_required": network.memo_required,
        "required_confirmations": network.required_confirmations,
        "deposit": {
            "available": network.deposit_available,
            "address_status": network.address.public_name(),
            "address": address,
        },
        "withdrawal": {"available": network.withdrawal_available},
    })
}

async fn list(State(api): Api, customer: AuthenticatedCustomer) -> Response {
    match api.service.wallets(customer.id()).await {
        Ok(wallets) => {
            let approved = api.service.policy_approved();
            let data: Vec<Value> = wallets.iter().map(|w| wallet_json(w, approved)).collect();
            json_response(StatusCode::OK, json!({"data": data}))
        }
        Err(problem) => failure(problem),
    }
}

async fn detail(
    State(api): Api,
    customer: AuthenticatedCustomer,
    Path(id): Path<String>,
) -> Response {
    let Some(id) = wallet_id(&id) else {
        return not_found();
    };
    let wallet = match api.service.wallet(customer.id(), id).await {
        Ok(wallet) => wallet,
        Err(problem) => return failure(problem),
    };
    let networks = match api.service.networks(customer.id(), id).await {
        Ok(networks) => networks,
        Err(problem) => return failure(problem),
    };
    let mut body = wallet_json(&wallet, api.service.policy_approved());
    body["networks"] = networks.iter().map(network_json).collect();
    json_response(StatusCode::OK, json!({"data": body}))
}

async fn balance(
    State(api): Api,
    customer: AuthenticatedCustomer,
    Path(id): Path<String>,
) -> Response {
    let Some(id) = wallet_id(&id) else {
        return not_found();
    };
    match api.service.wallet(customer.id(), id).await {
        Ok(wallet) if wallet.status == WalletStatus::Provisioning => error(
            StatusCode::SERVICE_UNAVAILABLE,
            "SERVICE_UNAVAILABLE",
            "Wallet is still being set up",
            json!({"reason": "wallet_provisioning"}),
        ),
        Ok(wallet) => {
            let mut data = balance_json(&wallet);
            data["wallet_id"] = json!(wallet.id.to_string());
            data["asset"] = json!(wallet.asset);
            data["status"] = json!(wallet.effective_status().public_name());
            json_response(StatusCode::OK, json!({"data": data}))
        }
        Err(problem) => failure(problem),
    }
}

/// The single `network=<code>` query parameter; anything else is invalid.
fn network_query(query: Option<&str>) -> Option<&str> {
    let mut parts = query?.split('&');
    let (name, value) = parts.next()?.split_once('=')?;
    (parts.next().is_none() && name == "network").then_some(value)
}

fn address_json(asset: &str, network: &str, state: &AddressState) -> Value {
    let (address, memo) = match state {
        AddressState::Active { address, memo } => (Some(address.as_str()), memo.as_deref()),
        _ => (None, None),
    };
    json!({"data": {
        "asset": asset,
        "network": network,
        "address": address,
        "memo": memo,
        "status": state.public_name(),
    }})
}

async fn deposit_address(
    State(api): Api,
    customer: AuthenticatedCustomer,
    Path(id): Path<String>,
    RawQuery(query): RawQuery,
) -> Response {
    let Some(id) = wallet_id(&id) else {
        return not_found();
    };
    let Some(network) = network_query(query.as_deref()) else {
        return failure(WalletError::InvalidRequest);
    };
    let wallet = match api.service.wallet(customer.id(), id).await {
        Ok(wallet) => wallet,
        Err(problem) => return failure(problem),
    };
    match api
        .service
        .deposit_address(customer.id(), id, network)
        .await
    {
        Ok(state) => json_response(StatusCode::OK, address_json(&wallet.asset, network, &state)),
        Err(problem) => failure(problem),
    }
}

/// Body: exactly `{"network": "<code>"}`. Returns the verified address, or
/// 202 with a pending request. Never contacts a provider during the request.
async fn request_address(
    State(api): Api,
    customer: AuthenticatedCustomer,
    Path(id): Path<String>,
    req: Request,
) -> Response {
    let Some(id) = wallet_id(&id) else {
        return not_found();
    };
    let Some(fields) = json_object(req).await else {
        return failure(WalletError::InvalidRequest);
    };
    let (1, Some(Value::String(network))) = (fields.len(), fields.get("network")) else {
        return failure(WalletError::InvalidRequest);
    };
    let wallet = match api.service.wallet(customer.id(), id).await {
        Ok(wallet) => wallet,
        Err(problem) => return failure(problem),
    };
    match api
        .service
        .request_deposit_address(customer.id(), id, network)
        .await
    {
        Ok(DepositAddress::Active { address, memo }) => json_response(
            StatusCode::OK,
            address_json(
                &wallet.asset,
                network,
                &AddressState::Active { address, memo },
            ),
        ),
        Ok(DepositAddress::Pending { .. }) => json_response(
            StatusCode::ACCEPTED,
            address_json(&wallet.asset, network, &AddressState::Pending),
        ),
        Err(problem) => failure(problem),
    }
}

fn supported_json(network: &SupportedNetwork) -> Value {
    json!({
        "asset": network.asset,
        "token_contract": network.token_contract,
        "token_decimals": network.token_decimals,
        "deposits": network.deposits,
        "withdrawals": network.withdrawals,
    })
}

/// Enabled networks, each with its enabled asset contracts.
async fn networks(State(api): Api, _: AuthenticatedCustomer) -> Response {
    let rows = match api.service.supported_networks(None).await {
        Ok(rows) => rows,
        Err(problem) => return failure(problem),
    };
    let mut data: Vec<Value> = Vec::new();
    for row in &rows {
        if data
            .last()
            .is_none_or(|last| last["network"] != json!(row.code))
        {
            data.push(json!({
                "network": row.code,
                "name": row.name,
                "chain_family": family(row.chain_family),
                "memo_required": row.memo_required,
                "required_confirmations": row.required_confirmations,
                "assets": [],
            }));
        }
        if let Some(Value::Array(assets)) = data.last_mut().map(|last| &mut last["assets"]) {
            assets.push(supported_json(row));
        }
    }
    json_response(StatusCode::OK, json!({"data": data}))
}

async fn asset_networks(
    State(api): Api,
    _: AuthenticatedCustomer,
    Path(asset): Path<String>,
) -> Response {
    let symbol = asset.to_ascii_uppercase();
    match api.service.supported_networks(Some(&symbol)).await {
        Ok(rows) => {
            let data: Vec<Value> = rows
                .iter()
                .map(|row| {
                    let mut item = supported_json(row);
                    item["network"] = json!(row.code);
                    item["name"] = json!(row.name);
                    item["chain_family"] = json!(family(row.chain_family));
                    item["memo_required"] = json!(row.memo_required);
                    item["required_confirmations"] = json!(row.required_confirmations);
                    item
                })
                .collect();
            json_response(StatusCode::OK, json!({"data": data}))
        }
        Err(WalletError::UnsupportedNetwork) => error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "UNSUPPORTED_ASSET",
            "Asset not supported",
            json!({}),
        ),
        Err(problem) => failure(problem),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_single_network_parameter_is_accepted() {
        assert_eq!(network_query(Some("network=base")), Some("base"));
        for bad in [
            None,
            Some(""),
            Some("network"),
            Some("net=base"),
            Some("network=base&network=polygon"),
            Some("network=base&x=1"),
        ] {
            assert_eq!(network_query(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn malformed_wallet_identifiers_look_unknown() {
        assert!(wallet_id("not-a-uuid").is_none());
        assert!(wallet_id("42e0fca0-5942-4ef0-974e-ef84594a35ef").is_some());
    }
}
