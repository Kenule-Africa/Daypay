//! Wallet and asset management tests (IMP-P6-010), run against real
//! PostgreSQL through the production grants. Customer calls run as
//! `deypay_app`, provider work as `deypay_wallet_provisioner`, status changes
//! as an operator-bound `deypay_ledger_controller` login and postings as
//! `deypay_ledger_poster`. Networks, token contracts, providers, capabilities
//! and activations are fixtures written as the database administrator; the
//! deposit-address provider is a deterministic test double behind the real
//! provisioning state machine.
use deypay_application::ledger::*;
use deypay_application::wallets::*;
use deypay_domain::ledger::{Direction, EventIdentity, JournalDraft, PostingLine};
use deypay_domain::units::{MinorUnits, PostingAmount};
use deypay_infrastructure::MIGRATOR;
use deypay_infrastructure::ledger::{LedgerRole, PgLedger};
use deypay_infrastructure::wallets::{PgProvisioning, PgWalletAdmin, PgWallets};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use std::collections::VecDeque;
use std::sync::Mutex;
use uuid::Uuid;

const LEDGER_ROLES: &str = include_str!("../../../../infrastructure/postgres/ledger-roles.psql");
const WALLET_ROLES: &str = include_str!("../../../../infrastructure/postgres/wallet-roles.psql");
const LEDGER_GRANTS: &str = include_str!("../../../../infrastructure/postgres/ledger-grants.psql");
const WALLET_GRANTS: &str = include_str!("../../../../infrastructure/postgres/wallet-grants.psql");
const UNIT: u128 = 1_000_000;
const CEILING: &str = "1000000000000000000000000000000";
const OPERATOR_LOGIN: &str = "deypay_test_wallet_operator";
const OUTSIDER_LOGIN: &str = "deypay_test_wallet_outsider";
const NETWORK: &str = "evm-fixture";
const USDC_CONTRACT: &str = "0xABCDEF0123456789abcdef0123456789ABCDEF01";
const USDT_CONTRACT: &str = "0x2222222222222222222222222222222222222222";

type Service = WalletService<PgWallets, NoApprovedPolicy>;

struct World {
    pool: PgPool,
    app_pool: PgPool,
    service: Service,
    poster: PgLedger,
    usdc: Uuid,
    usdt: Uuid,
}

/// A role standing in for a login, created once per cluster.
async fn test_role(pool: &PgPool, name: &str, member_of: &str) {
    sqlx::raw_sql(&format!(
        "DO $$ BEGIN
             CREATE ROLE {name} NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS
                 IN ROLE {member_of};
         EXCEPTION WHEN duplicate_object OR unique_violation THEN NULL;
         END $$"
    ))
    .execute(pool)
    .await
    .unwrap();
}

/// Connections whose session user is `login`, as if it had authenticated.
fn session_as(pool: &PgPool, login: &'static str) -> PgPool {
    PgPoolOptions::new()
        .max_connections(2)
        .after_connect(move |conn, _| {
            Box::pin(async move {
                sqlx::query("SELECT set_config('session_authorization', $1, false)")
                    .bind(login)
                    .execute(conn)
                    .await?;
                Ok(())
            })
        })
        .connect_lazy_with((*pool.connect_options()).clone())
}

/// The restricted API runtime role, as the profile HTTP tests use it.
fn app_pool(pool: &PgPool) -> PgPool {
    PgPoolOptions::new()
        .max_connections(8)
        .after_connect(|conn, _| {
            Box::pin(async move {
                sqlx::query("SET ROLE deypay_app").execute(conn).await?;
                Ok(())
            })
        })
        .connect_lazy_with((*pool.connect_options()).clone())
}

async fn setup(pool: PgPool) -> World {
    for script in [LEDGER_ROLES, WALLET_ROLES, LEDGER_GRANTS, WALLET_GRANTS] {
        sqlx::raw_sql(script).execute(&pool).await.unwrap();
    }
    let asset = |symbol: &'static str| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, Uuid>("SELECT id FROM wallets.assets WHERE symbol = $1")
                .bind(symbol)
                .fetch_one(&pool)
                .await
                .unwrap()
        }
    };
    let usdc = asset("USDC").await;
    let usdt = asset("USDT").await;
    let app_pool = app_pool(&pool);
    World {
        service: WalletService::new(PgWallets::new(app_pool.clone()), NoApprovedPolicy),
        poster: PgLedger::acting_as(pool.clone(), LedgerRole::Poster),
        app_pool,
        pool,
        usdc,
        usdt,
    }
}

async fn customer(pool: &PgPool, name: &str, status: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO identity.users(email, status, email_verified_at)
         VALUES ($1, $2, CASE WHEN $2 IN ('REGISTERED', 'CONTACT_UNVERIFIED') THEN NULL ELSE now() END)
         RETURNING id",
    )
    .bind(format!("{name}@customer.test"))
    .bind(status)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// Runs SQL as `role` (or the session's own user) and returns its SQLSTATE.
async fn sqlstate(pool: &PgPool, role: Option<&str>, sql: &str) -> Option<String> {
    let mut tx = pool.begin().await.unwrap();
    if let Some(role) = role {
        sqlx::query(&format!("SET LOCAL ROLE {role}"))
            .execute(&mut *tx)
            .await
            .unwrap();
    }
    let error = match sqlx::raw_sql(sql).execute(&mut *tx).await {
        Ok(_) => tx.commit().await.err(),
        Err(error) => Some(error),
    };
    error.and_then(|error| {
        error
            .as_database_error()
            .and_then(|db| db.code())
            .map(|code| code.into_owned())
    })
}

async fn count(pool: &PgPool, sql: &str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}

impl World {
    async fn activate(&self, capability: &str, asset: Uuid) {
        sqlx::query(
            "INSERT INTO ledger.capability_activations(capability, asset_id, approval_reference, max_operation_minor)
             VALUES ($1, $2, 'TEST-FIXTURE', $3::numeric)",
        )
        .bind(capability)
        .bind(asset)
        .bind(CEILING)
        .execute(&self.pool)
        .await
        .unwrap();
    }

    async fn wallets(&self, customer: Uuid) -> Vec<Wallet> {
        self.service.wallets(customer).await.unwrap()
    }

    async fn wallet_of(&self, customer: Uuid, asset: &str) -> Wallet {
        self.wallets(customer)
            .await
            .into_iter()
            .find(|wallet| wallet.asset == asset)
            .unwrap()
    }

    async fn ledger_account(&self, customer: Uuid, asset: Uuid, role: &str) -> Uuid {
        sqlx::query_scalar(
            "SELECT id FROM ledger.accounts WHERE owner_id = $1 AND asset_id = $2 AND customer_role = $3",
        )
        .bind(customer)
        .bind(asset)
        .bind(role)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    /// A verified deposit credited to the customer through the ledger.
    async fn deposit(&self, key: &str, customer: Uuid, asset: Uuid, value: u128) {
        let custody: Uuid = sqlx::query_scalar(
            "INSERT INTO ledger.accounts(chart_code, account_type, normal_balance, negative_allowed, asset_id,
                 owner_type, instance_key)
             VALUES (CASE WHEN $1 = (SELECT id FROM wallets.assets WHERE symbol = 'USDC') THEN '1100' ELSE '1110' END,
                 'ASSET', 'DEBIT', false, $1, 'PLATFORM', 'primary')
             ON CONFLICT (chart_code, asset_id, instance_key) WHERE owner_type = 'PLATFORM' DO UPDATE SET status = 'OPEN'
             RETURNING id",
        )
        .bind(asset)
        .fetch_one(&self.pool)
        .await
        .unwrap();
        let available = self.ledger_account(customer, asset, "AVAILABLE").await;
        let amount = PostingAmount::from_value(value).unwrap();
        self.poster
            .post(&PostingCommand {
                identity: EventIdentity::new("DEPOSIT_CONFIRMED", key, "RECOGNITION").unwrap(),
                source_transaction: None,
                journal: JournalDraft::new(vec![
                    PostingLine {
                        account_id: custody,
                        asset_id: asset,
                        direction: Direction::Debit,
                        amount,
                    },
                    PostingLine {
                        account_id: available,
                        asset_id: asset,
                        direction: Direction::Credit,
                        amount,
                    },
                ])
                .unwrap(),
                description: None,
                actor: Actor::System,
                correlation_id: None,
            })
            .await
            .unwrap();
    }

    fn reserve(&self, key: &str, customer: Uuid, asset: Uuid, value: u128) -> ReserveCommand {
        ReserveCommand {
            operation_key: key.into(),
            customer,
            asset_id: asset,
            amount: PostingAmount::from_value(value).unwrap(),
            transaction: None,
            event_type: "WITHDRAWAL_RESERVED".into(),
            actor: Actor::Customer(customer),
            correlation_id: None,
        }
    }

    async fn violations(&self) -> Vec<String> {
        sqlx::query_scalar("SELECT kind FROM ledger.integrity_violations()")
            .fetch_all(&self.pool)
            .await
            .unwrap()
    }

    /// An enabled EVM fixture network with deposits, and verified, enabled
    /// USDC and USDT contracts on it.
    async fn deposit_network(&self) -> (Uuid, Uuid, Uuid) {
        let network: Uuid = sqlx::query_scalar(
            "INSERT INTO wallets.networks(code, name, chain_family, chain_identifier, enabled, deposits_enabled,
                 required_confirmations, evidence_reference)
             VALUES ($1, 'EVM fixture', 'EVM', '31337', true, true, 12, 'TEST-FIXTURE') RETURNING id",
        )
        .bind(NETWORK)
        .fetch_one(&self.pool)
        .await
        .unwrap();
        let mut mappings = Vec::new();
        for (asset, contract) in [(self.usdc, USDC_CONTRACT), (self.usdt, USDT_CONTRACT)] {
            mappings.push(
                sqlx::query_scalar::<_, Uuid>(
                    "INSERT INTO wallets.asset_networks(asset_id, network_id, token_contract, token_decimals,
                         verification_status, evidence_reference, verified_at, status, deposit_enabled)
                     VALUES ($1, $2, $3, 6, 'VERIFIED', 'TEST-FIXTURE', now(), 'ENABLED', true) RETURNING id",
                )
                .bind(asset)
                .bind(network)
                .bind(contract)
                .fetch_one(&self.pool)
                .await
                .unwrap(),
            );
        }
        (network, mappings[0], mappings[1])
    }

    /// An approved, enabled provider with a production-enabled deposit-address
    /// capability for every asset/network.
    async fn provider(&self, code: &str, priority: i32) -> Uuid {
        let provider: Uuid = sqlx::query_scalar(
            "INSERT INTO integrations.providers(provider_code, provider_name, provider_type, enabled, lifecycle_status)
             VALUES ($1, 'Provider fixture', 'TEST', true, 'APPROVED') RETURNING id",
        )
        .bind(code)
        .fetch_one(&self.pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO integrations.provider_capabilities(provider_id, service, environment, available,
                 production_approved, verification_level, evidence_reference, routing_priority)
             VALUES ($1, 'DEPOSIT_ADDRESS_PROVISIONING', 'production', true, true, 'PRODUCTION_ENABLED',
                 'TEST-FIXTURE', $2)",
        )
        .bind(provider)
        .bind(priority)
        .execute(&self.pool)
        .await
        .unwrap();
        provider
    }

    async fn address_state(&self, customer: Uuid, wallet: Uuid) -> AddressState {
        self.service
            .deposit_address(customer, wallet, NETWORK)
            .await
            .unwrap()
    }
}

/// Deterministic provider double: answers from a script and records calls.
struct ScriptedProvider {
    code: &'static str,
    answers: Mutex<VecDeque<ProviderResult>>,
    confirmations: Mutex<VecDeque<AddressConfirmation>>,
    calls: Mutex<Vec<(&'static str, String)>>,
}

impl ScriptedProvider {
    fn new(code: &'static str) -> Self {
        Self {
            code,
            answers: Mutex::default(),
            confirmations: Mutex::default(),
            calls: Mutex::default(),
        }
    }

    fn answer(&self, result: ProviderResult) -> &Self {
        self.answers.lock().unwrap().push_back(result);
        self
    }

    fn confirm(&self, confirmation: AddressConfirmation) -> &Self {
        self.confirmations.lock().unwrap().push_back(confirmation);
        self
    }

    fn requests(&self) -> usize {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .filter(|(kind, _)| *kind == "request")
            .count()
    }

    fn next(&self, kind: &'static str, request: &AddressRequest) -> ProviderResult {
        self.calls
            .lock()
            .unwrap()
            .push((kind, request.reference.clone()));
        self.answers
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(ProviderResult::Unknown)
    }
}

impl DepositAddressProvider for &ScriptedProvider {
    fn provider_code(&self) -> &str {
        self.code
    }
    async fn request_address(&self, request: &AddressRequest) -> ProviderResult {
        self.next("request", request)
    }
    async fn lookup_address(&self, request: &AddressRequest) -> ProviderResult {
        self.next("lookup", request)
    }
    async fn confirm_address(
        &self,
        _: &AddressRequest,
        _: &str,
        _: Option<&str>,
    ) -> AddressConfirmation {
        self.confirmations
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(AddressConfirmation::Unknown)
    }
}

async fn run(
    store: &PgProvisioning,
    providers: Vec<&ScriptedProvider>,
) -> Result<ProvisioningReport, WalletError> {
    AddressProvisioner::new(store.clone(), providers)
        .run_once(10)
        .await
}

fn provisioned(address: &str, reference: &str) -> ProviderResult {
    ProviderResult::Provisioned {
        external_reference: reference.into(),
        address: address.into(),
        memo: None,
    }
}

fn confirmed() -> AddressConfirmation {
    AddressConfirmation::Confirmed {
        evidence_reference: "provider:lookup:fixture".into(),
        sha256: [5; 32],
    }
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn asset_registry_is_controlled_and_audited(pool: PgPool) {
    let world = setup(pool).await;
    let seeded: Vec<(String, String, i16, bool)> = sqlx::query_as(
        "SELECT symbol, asset_type, accounting_precision, enabled FROM wallets.assets ORDER BY symbol",
    )
    .fetch_all(&world.pool)
    .await
    .unwrap();
    assert_eq!(
        seeded,
        [
            ("NGN", "FIAT", 2, true),
            ("USDC", "STABLECOIN", 6, true),
            ("USDT", "STABLECOIN", 6, true)
        ]
        .map(|(s, t, p, e)| (s.to_owned(), t.to_owned(), p, e))
        .to_vec()
    );
    let insert = |values: &str| {
        format!(
            "INSERT INTO wallets.assets(symbol, name, asset_type, accounting_precision, enabled, approval_reference)
             VALUES {values}"
        )
    };
    for (sql, expected) in [
        (
            insert("('USDC', 'Duplicate', 'STABLECOIN', 6, false, NULL)"),
            "23505",
        ),
        (
            insert("('EURC', 'Euro Coin', 'TOKEN', 6, false, NULL)"),
            "23514",
        ),
        (
            insert("('EURC', 'Euro Coin', 'STABLECOIN', 19, false, NULL)"),
            "23514",
        ),
        (
            insert("('eurc', 'Euro Coin', 'STABLECOIN', 6, false, NULL)"),
            "23514",
        ),
        (
            insert("('EURC', 'Euro Coin', 'STABLECOIN', 6, true, NULL)"),
            "23514",
        ),
        // Approved but without ledger chart accounts: cannot become operational.
        (
            insert("('EURC', 'Euro Coin', 'STABLECOIN', 6, true, 'fixture approval')"),
            "WA005",
        ),
        (
            "UPDATE wallets.assets SET accounting_precision = 2 WHERE symbol = 'USDC'".into(),
            "23514",
        ),
        (
            "UPDATE wallets.assets SET asset_type = 'FIAT' WHERE symbol = 'USDC'".into(),
            "55000",
        ),
        (
            "UPDATE wallets.assets SET suspended_at = now() WHERE symbol = 'USDC'".into(),
            "23514",
        ),
        (
            "DELETE FROM wallets.assets WHERE symbol = 'NGN'".into(),
            "55000",
        ),
    ] {
        assert_eq!(
            sqlstate(&world.pool, None, &sql).await.as_deref(),
            Some(expected),
            "{sql}"
        );
    }
    // A known but unapproved asset may be recorded; it is not operational.
    assert_eq!(
        sqlstate(
            &world.pool,
            None,
            &insert("('EURC', 'Euro Coin', 'STABLECOIN', 6, false, NULL)")
        )
        .await,
        None
    );
    // Only the database administrator changes the registry.
    for role in [
        "deypay_app",
        "deypay_wallet_provisioner",
        "deypay_wallet_owner",
        "deypay_ledger_controller",
        "deypay_ledger_poster",
    ] {
        for sql in [
            "UPDATE wallets.assets SET enabled = false WHERE symbol = 'USDT'",
            "INSERT INTO wallets.networks(code, name, chain_family, chain_identifier) VALUES ('rogue', 'x', 'EVM', '7')",
            "UPDATE integrations.providers SET enabled = true",
        ] {
            assert_eq!(
                sqlstate(&world.pool, Some(role), sql).await.as_deref(),
                Some("42501"),
                "{role}: {sql}"
            );
        }
    }
    // Every material change is kept, and the history itself cannot change.
    sqlx::query(
        "UPDATE wallets.assets SET suspended_at = now(), suspension_reason = 'Incident drill' WHERE symbol = 'USDT'",
    )
    .execute(&world.pool)
    .await
    .unwrap();
    let history: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT operation, current->>'suspension_reason' FROM wallets.registry_changes
         WHERE registry = 'wallets.assets' AND record_id = $1 ORDER BY changed_at",
    )
    .bind(world.usdt)
    .fetch_all(&world.pool)
    .await
    .unwrap();
    assert_eq!(
        history,
        vec![
            ("INSERT".to_owned(), None),
            ("UPDATE".to_owned(), Some("Incident drill".to_owned()))
        ]
    );
    for sql in [
        "UPDATE wallets.registry_changes SET changed_by = 'someone'",
        "DELETE FROM wallets.registry_changes",
    ] {
        assert_eq!(
            sqlstate(&world.pool, None, sql).await.as_deref(),
            Some("55000"),
            "{sql}"
        );
    }
    // The candidate providers are recorded but neither approved nor enabled.
    let providers: Vec<(String, String, bool)> = sqlx::query_as(
        "SELECT provider_code, lifecycle_status, enabled FROM integrations.providers ORDER BY provider_code",
    )
    .fetch_all(&world.pool)
    .await
    .unwrap();
    assert_eq!(
        providers,
        vec![
            ("IMPORTAPAY".to_owned(), "CANDIDATE".to_owned(), false),
            ("NUVION".to_owned(), "CANDIDATE".to_owned(), false)
        ]
    );
    assert_eq!(
        count(
            &world.pool,
            "SELECT count(*) FROM integrations.provider_capabilities"
        )
        .await,
        0
    );
    assert_eq!(
        sqlstate(
            &world.pool,
            None,
            "UPDATE integrations.providers SET enabled = true WHERE provider_code = 'NUVION'"
        )
        .await
        .as_deref(),
        Some("23514"),
        "a candidate provider cannot be enabled"
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn networks_and_token_contracts_are_explicit_and_verified(pool: PgPool) {
    let world = setup(pool).await;
    let network: Uuid = sqlx::query_scalar(
        "INSERT INTO wallets.networks(code, name, chain_family, chain_identifier)
         VALUES ('evm-fixture', 'EVM fixture', 'EVM', '31337') RETURNING id",
    )
    .fetch_one(&world.pool)
    .await
    .unwrap();
    let net = |values: &str| {
        format!(
            "INSERT INTO wallets.networks(code, name, chain_family, chain_identifier) VALUES {values}"
        )
    };
    let map = |asset: Uuid, contract: &str, decimals: i32| {
        format!(
            "INSERT INTO wallets.asset_networks(asset_id, network_id, token_contract, token_decimals)
             VALUES ('{asset}', '{network}', '{contract}', {decimals})"
        )
    };
    let ngn: Uuid = sqlx::query_scalar("SELECT id FROM wallets.assets WHERE symbol = 'NGN'")
        .fetch_one(&world.pool)
        .await
        .unwrap();
    for (sql, expected) in [
        (net("('evm-fixture', 'Again', 'EVM', '1')"), "23505"),
        (net("('evm-other', 'Same chain', 'EVM', '31337')"), "23505"),
        (net("('Base', 'Upper case', 'EVM', '8453')"), "23514"),
        (
            net("('tron-fixture', 'Unsupported family', 'TRON', '728126428')"),
            "23514",
        ),
        (net("('evm-hex', 'Hex chain id', 'EVM', '0x7a69')"), "23514"),
        (
            net("('sol-bad', 'Not base58', 'SOLANA', 'not-a-genesis-hash!')"),
            "23514",
        ),
        (
            format!("UPDATE wallets.networks SET enabled = true WHERE id = '{network}'"),
            "23514",
        ),
        (
            format!(
                "UPDATE wallets.networks SET enabled = true, evidence_reference = 'TEST-FIXTURE',
                 deposits_enabled = true WHERE id = '{network}'"
            ),
            "23514",
        ),
        (
            format!(
                "UPDATE wallets.networks SET deposits_enabled = true, required_confirmations = 12 WHERE id = '{network}'"
            ),
            "23514",
        ),
        (
            format!("UPDATE wallets.networks SET chain_identifier = '1' WHERE id = '{network}'"),
            "55000",
        ),
        ("DELETE FROM wallets.networks".into(), "55000"),
        (map(world.usdc, "0x123", 6), "WA002"),
        (
            map(world.usdc, "0x0000000000000000000000000000000000000000", 6),
            "WA002",
        ),
        (
            map(ngn, "0x1111111111111111111111111111111111111111", 2),
            "WA005",
        ),
        (
            map(world.usdt, "0x3333333333333333333333333333333333333333", 37),
            "23514",
        ),
    ] {
        assert_eq!(
            sqlstate(&world.pool, None, &sql).await.as_deref(),
            Some(expected),
            "{sql}"
        );
    }
    // Contracts are stored canonically; one contract maps to one asset.
    sqlx::raw_sql(&map(world.usdc, USDC_CONTRACT, 6))
        .execute(&world.pool)
        .await
        .unwrap();
    let (mapping, stored): (Uuid, String) =
        sqlx::query_as("SELECT id, token_contract FROM wallets.asset_networks WHERE asset_id = $1")
            .bind(world.usdc)
            .fetch_one(&world.pool)
            .await
            .unwrap();
    assert_eq!(stored, USDC_CONTRACT.to_ascii_lowercase());
    let on = |sql: &str| sql.replace("{id}", &mapping.to_string());
    for (sql, expected) in [
        (
            map(world.usdt, &USDC_CONTRACT.to_ascii_lowercase(), 6),
            "23505",
        ),
        (
            on("UPDATE wallets.asset_networks SET status = 'ENABLED' WHERE id = '{id}'"),
            "23514",
        ),
        (
            on("UPDATE wallets.asset_networks SET token_decimals = 18 WHERE id = '{id}'"),
            "55000",
        ),
        (
            on(
                "UPDATE wallets.asset_networks SET verification_status = 'VERIFIED' WHERE id = '{id}'",
            ),
            "23514",
        ),
    ] {
        assert_eq!(
            sqlstate(&world.pool, None, &sql).await.as_deref(),
            Some(expected),
            "{sql}"
        );
    }
    sqlx::raw_sql(&on(
        "UPDATE wallets.asset_networks SET verification_status = 'VERIFIED', verified_at = now(),
         evidence_reference = 'TEST-FIXTURE' WHERE id = '{id}'",
    ))
    .execute(&world.pool)
    .await
    .unwrap();
    assert_eq!(
        sqlstate(
            &world.pool,
            None,
            &on("UPDATE wallets.asset_networks SET status = 'ENABLED' WHERE id = '{id}'")
        )
        .await
        .as_deref(),
        Some("WA005"),
        "the network must be enabled first"
    );
    assert!(
        world
            .service
            .supported_networks(None)
            .await
            .unwrap()
            .is_empty(),
        "nothing is presented as supported before activation"
    );
    sqlx::query(
        "UPDATE wallets.networks SET enabled = true, evidence_reference = 'TEST-FIXTURE' WHERE id = $1",
    )
    .bind(network)
    .execute(&world.pool)
    .await
    .unwrap();
    sqlx::raw_sql(&on(
        "UPDATE wallets.asset_networks SET status = 'ENABLED' WHERE id = '{id}'",
    ))
    .execute(&world.pool)
    .await
    .unwrap();
    // A second enabled USDC contract on the same network would be ambiguous.
    assert_eq!(
        sqlstate(
            &world.pool,
            None,
            &format!(
                "INSERT INTO wallets.asset_networks(asset_id, network_id, token_contract, token_decimals,
                     verification_status, verified_at, evidence_reference, status)
                 VALUES ('{}', '{network}', '0x4444444444444444444444444444444444444444', 6, 'VERIFIED', now(),
                     'TEST-FIXTURE', 'ENABLED')",
                world.usdc
            )
        )
        .await
        .as_deref(),
        Some("23505")
    );
    let supported = world
        .service
        .supported_networks(Some("USDC"))
        .await
        .unwrap();
    assert_eq!(supported.len(), 1);
    assert_eq!(
        (
            supported[0].code.as_str(),
            supported[0].chain_family,
            supported[0].deposits
        ),
        ("evm-fixture", ChainFamily::Evm, false)
    );
    assert_eq!(
        world.service.supported_networks(Some("NGN")).await,
        Err(WalletError::UnsupportedNetwork),
        "NGN has no customer wallet networks"
    );
    // A token migration retires the old mapping; a retired one never returns.
    sqlx::raw_sql(&on(
        "UPDATE wallets.asset_networks SET status = 'RETIRED' WHERE id = '{id}'",
    ))
    .execute(&world.pool)
    .await
    .unwrap();
    assert_eq!(
        sqlstate(
            &world.pool,
            None,
            &on("UPDATE wallets.asset_networks SET status = 'ENABLED' WHERE id = '{id}'")
        )
        .await
        .as_deref(),
        Some("55000")
    );
    assert!(
        world
            .service
            .supported_networks(Some("USDC"))
            .await
            .unwrap()
            .is_empty()
    );
    // Solana contracts must be 32-byte base58 keys.
    let solana: Uuid = sqlx::query_scalar(
        "INSERT INTO wallets.networks(code, name, chain_family, chain_identifier)
         VALUES ('sol-fixture', 'Solana fixture', 'SOLANA', '11111111111111111111111111111111') RETURNING id",
    )
    .fetch_one(&world.pool)
    .await
    .unwrap();
    for (contract, expected) in [
        ("0x1111111111111111111111111111111111111111", Some("WA002")),
        ("11111111111111111111111111111112", None),
    ] {
        assert_eq!(
            sqlstate(
                &world.pool,
                None,
                &format!(
                    "INSERT INTO wallets.asset_networks(asset_id, network_id, token_contract, token_decimals)
                     VALUES ('{}', '{solana}', '{contract}', 6)",
                    world.usdc
                )
            )
            .await
            .as_deref(),
            expected,
            "{contract}"
        );
    }
    assert!(
        count(
            &world.pool,
            "SELECT count(*) FROM wallets.registry_changes WHERE registry = 'wallets.asset_networks'"
        )
        .await
            >= 5
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn customer_wallets_are_created_once_at_zero(pool: PgPool) {
    let world = setup(pool).await;
    let alice = customer(&world.pool, "alice", "CONTACT_VERIFIED").await;
    let wallets = world.wallets(alice).await;
    assert_eq!(
        wallets.iter().map(|w| w.asset.as_str()).collect::<Vec<_>>(),
        ["USDC", "USDT"]
    );
    for wallet in &wallets {
        assert_eq!(
            wallet.status,
            deypay_application::wallets::WalletStatus::Active
        );
        let balance = wallet.balance.unwrap();
        assert_eq!(
            (balance.available, balance.reserved, balance.total),
            (MinorUnits::ZERO, MinorUnits::ZERO, MinorUnits::ZERO)
        );
        assert_eq!(wallet.format(balance.total), "0.000000");
        assert_eq!(
            wallet.restrictions,
            vec![
                Restriction::KycRequired,
                Restriction::FinancialActivityNotEnabled
            ],
            "a wallet does not imply KYC approval or financial permission"
        );
        assert!(!wallet.can_send(world.service.policy_approved()));
    }
    let ledger: (i64, i64) = sqlx::query_as(
        "SELECT count(*), count(*) FILTER (WHERE b.balance_minor = 0) FROM ledger.accounts a
         JOIN ledger.balances b ON b.account_id = a.id WHERE a.owner_id = $1",
    )
    .bind(alice)
    .fetch_one(&world.pool)
    .await
    .unwrap();
    assert_eq!(
        ledger,
        (4, 4),
        "available and reserved accounts per wallet, all at zero"
    );
    assert_eq!(
        count(&world.pool, "SELECT count(*) FROM ledger.journals").await,
        0,
        "initialization creates no value"
    );
    // Repeated initialization changes nothing.
    assert_eq!(world.service.store().initialize(alice).await, Ok(0));
    assert_eq!(
        world
            .wallets(alice)
            .await
            .iter()
            .map(|w| w.id)
            .collect::<Vec<_>>(),
        wallets.iter().map(|w| w.id).collect::<Vec<_>>()
    );
    let provisioned = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM audit.audit_events WHERE action = 'WALLET_PROVISIONED' AND metadata->>'customer_id' = $1",
    )
    .bind(alice.to_string())
    .fetch_one(&world.pool)
    .await
    .unwrap();
    assert_eq!(provisioned, 2);
    // There is no customer NGN wallet.
    let ngn: Uuid = sqlx::query_scalar("SELECT id FROM wallets.assets WHERE symbol = 'NGN'")
        .fetch_one(&world.pool)
        .await
        .unwrap();
    assert!(
        sqlx::query("INSERT INTO wallets.wallet_profiles(user_id, asset_id, asset_symbol, status, provisioning_status)
                     VALUES ($1, $2, 'NGN', 'PROVISIONING', 'PENDING')")
            .bind(alice)
            .bind(ngn)
            .execute(&world.pool)
            .await
            .is_err()
    );

    // Concurrent first requests create each wallet exactly once.
    let bob = customer(&world.pool, "bob", "KYC_PENDING").await;
    let service = std::sync::Arc::new(world.service);
    let mut tasks = Vec::new();
    for _ in 0..16 {
        let service = service.clone();
        tasks.push(tokio::spawn(async move { service.wallets(bob).await }));
    }
    let mut seen = std::collections::BTreeSet::new();
    for task in tasks {
        for wallet in task.await.unwrap().unwrap() {
            seen.insert(wallet.id);
        }
    }
    assert_eq!(seen.len(), 2);
    let bob_rows: (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM wallets.wallet_profiles WHERE user_id = $1),
                (SELECT count(*) FROM ledger.accounts WHERE owner_id = $1)",
    )
    .bind(bob)
    .fetch_one(&world.pool)
    .await
    .unwrap();
    assert_eq!(bob_rows, (2, 4));
    assert!(
        seen.iter().all(|id| !wallets.iter().any(|w| w.id == *id)),
        "two customers never share a wallet"
    );

    // Accounts that cannot hold a session get no wallet, and nothing is created.
    for (name, status) in [
        ("unverified", "CONTACT_UNVERIFIED"),
        ("suspended", "SUSPENDED"),
        ("rejected", "KYC_REJECTED"),
        ("closed", "CLOSED"),
    ] {
        let id = customer(&world.pool, name, status).await;
        assert_eq!(
            service.store().initialize(id).await,
            Err(WalletError::NotEligible),
            "{status}"
        );
        assert_eq!(service.wallets(id).await, Ok(vec![]), "{status}");
    }

    // A caller transaction that rolls back takes the whole initialization with it.
    let carol = customer(&world.pool, "carol", "CONTACT_VERIFIED").await;
    let mut tx = world.app_pool.begin().await.unwrap();
    sqlx::query("SELECT wallets.initialize_customer_wallets($1)")
        .bind(carol)
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.rollback().await.unwrap();
    let carol_rows: (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM wallets.wallet_profiles WHERE user_id = $1),
                (SELECT count(*) FROM ledger.accounts WHERE owner_id = $1)",
    )
    .bind(carol)
    .fetch_one(&world.pool)
    .await
    .unwrap();
    assert_eq!(carol_rows, (0, 0));
    assert_eq!(service.wallets(carol).await.unwrap().len(), 2);

    // An interrupted earlier initialization is completed by the next request.
    let dave = customer(&world.pool, "dave", "CONTACT_VERIFIED").await;
    sqlx::query(
        "INSERT INTO wallets.wallet_profiles(user_id, asset_id, asset_symbol, status, provisioning_status)
         VALUES ($1, $2, 'USDC', 'PROVISIONING', 'PENDING')",
    )
    .bind(dave)
    .bind(world.usdc)
    .execute(&world.pool)
    .await
    .unwrap();
    let pending = service.store().wallets(dave, None).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(
        pending[0].balance, None,
        "no balance is guessed before the ledger accounts exist"
    );
    let completed = service.wallets(dave).await.unwrap();
    assert_eq!(completed.len(), 2);
    assert!(
        completed
            .iter()
            .all(|w| w.status == deypay_application::wallets::WalletStatus::Active)
    );
    assert_eq!(
        completed[0].id, pending[0].id,
        "the wallet identity is kept"
    );
    let violations: Vec<String> =
        sqlx::query_scalar("SELECT kind FROM ledger.integrity_violations()")
            .fetch_all(&world.pool)
            .await
            .unwrap();
    assert!(violations.is_empty());
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn balances_come_only_from_the_ledger(pool: PgPool) {
    let world = setup(pool).await;
    let alice = customer(&world.pool, "alice", "KYC_APPROVED").await;
    let bob = customer(&world.pool, "bob", "KYC_APPROVED").await;
    let usdc_wallet = world.wallet_of(alice, "USDC").await;
    world.wallets(bob).await;
    world.activate("CUSTOMER_CREDIT", world.usdc).await;
    world.activate("CUSTOMER_RESERVATION", world.usdc).await;
    world.activate("CUSTOMER_CREDIT", world.usdt).await;
    world
        .deposit("chain:fixture:1", alice, world.usdc, 12_500_000)
        .await;
    world
        .poster
        .reserve(&world.reserve("withdrawal:1", alice, world.usdc, 2 * UNIT))
        .await
        .unwrap();
    let wallet = world.service.wallet(alice, usdc_wallet.id).await.unwrap();
    let balance = wallet.balance.unwrap();
    assert_eq!(
        (
            wallet.format(balance.available),
            wallet.format(balance.reserved),
            wallet.format(balance.total)
        ),
        (
            "10.500000".to_owned(),
            "2.000000".to_owned(),
            "12.500000".to_owned()
        )
    );
    let ledger: (String, String, String) = sqlx::query_as(
        "SELECT available_minor, reserved_minor, total_minor FROM ledger.customer_balance($1, $2)",
    )
    .bind(alice)
    .bind(world.usdc)
    .fetch_one(&world.pool)
    .await
    .unwrap();
    assert_eq!(
        ledger,
        (
            balance.available.to_string(),
            balance.reserved.to_string(),
            balance.total.to_string()
        )
    );
    // Large exact values, no rounding, and assets kept apart.
    let large = 123_456_789_012_345_678_901_234_567_890_123_456_u128;
    world
        .deposit("chain:fixture:2", alice, world.usdt, large)
        .await;
    let usdt = world.wallet_of(alice, "USDT").await;
    assert_eq!(
        usdt.format(usdt.balance.unwrap().available),
        "123456789012345678901234567890.123456"
    );
    assert_eq!(
        world.wallet_of(alice, "USDC").await.balance.unwrap().total,
        balance.total
    );
    // Another customer's wallet is indistinguishable from an unknown one.
    assert_eq!(
        world.service.wallet(bob, usdc_wallet.id).await,
        Err(WalletError::NotFound)
    );
    assert_eq!(
        world.service.wallet(alice, Uuid::from_u128(0x5eed)).await,
        Err(WalletError::NotFound)
    );
    assert_eq!(
        world.service.networks(bob, usdc_wallet.id).await,
        Err(WalletError::NotFound)
    );
    // The API role reaches balances only through the wallet functions.
    for sql in [
        "SELECT * FROM ledger.balances".to_owned(),
        format!(
            "SELECT * FROM ledger.customer_balance('{alice}', '{}')",
            world.usdc
        ),
        "SELECT * FROM wallets.wallet_addresses".to_owned(),
        "SELECT * FROM wallets.wallet_provisioning_requests".to_owned(),
        format!(
            "UPDATE wallets.wallet_profiles SET status = 'SUSPENDED' WHERE user_id = '{alice}'"
        ),
    ] {
        assert_eq!(
            sqlstate(&world.pool, Some("deypay_app"), &sql)
                .await
                .as_deref(),
            Some("42501"),
            "{sql}"
        );
    }
    // Without the database there is an error, never substitute data.
    let unreachable = world.app_pool.clone();
    unreachable.close().await;
    let offline = WalletService::new(PgWallets::new(unreachable), NoApprovedPolicy);
    assert_eq!(offline.wallets(alice).await, Err(WalletError::Unavailable));
    assert_eq!(
        offline.wallet(alice, usdc_wallet.id).await,
        Err(WalletError::Unavailable)
    );
    assert!(world.violations().await.is_empty());
}

/// An administrator's identity is the operator bound to the login (Phase 5).
async fn operator(world: &World) -> (PgWalletAdmin, Uuid, PgWalletAdmin, Uuid) {
    for login in [OPERATOR_LOGIN, OUTSIDER_LOGIN] {
        test_role(&world.pool, login, "deypay_ledger_controller").await;
    }
    let role: Uuid = sqlx::query_scalar(
        "INSERT INTO audit.admin_roles(role_code, description) VALUES ('WALLET_OPERATIONS', 'fixture') RETURNING id",
    )
    .fetch_one(&world.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO audit.role_permissions(role_id, permission_id)
         SELECT $1, id FROM audit.admin_permissions WHERE permission_code = 'WALLET_STATUS_CONTROL'",
    )
    .bind(role)
    .execute(&world.pool)
    .await
    .unwrap();
    let staff = customer(&world.pool, "operator", "CONTACT_VERIFIED").await;
    let outsider = customer(&world.pool, "outsider", "CONTACT_VERIFIED").await;
    sqlx::query("INSERT INTO audit.admin_assignments(user_id, role_id) VALUES ($1, $2)")
        .bind(staff)
        .bind(role)
        .execute(&world.pool)
        .await
        .unwrap();
    for (login, user) in [(OPERATOR_LOGIN, staff), (OUTSIDER_LOGIN, outsider)] {
        sqlx::query(
            "INSERT INTO ledger.operator_logins(login_name, user_id, approval_reference) VALUES ($1, $2, 'TEST-FIXTURE')",
        )
        .bind(login)
        .bind(user)
        .execute(&world.pool)
        .await
        .unwrap();
    }
    (
        PgWalletAdmin::new(session_as(&world.pool, OPERATOR_LOGIN)),
        staff,
        PgWalletAdmin::new(session_as(&world.pool, OUTSIDER_LOGIN)),
        outsider,
    )
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn wallet_status_changes_require_an_authenticated_operator(pool: PgPool) {
    use deypay_application::wallets::WalletStatus as Status;
    let world = setup(pool).await;
    let (admin, staff, outsider_admin, outsider) = operator(&world).await;
    let alice = customer(&world.pool, "alice", "KYC_APPROVED").await;
    let usdc = world.wallet_of(alice, "USDC").await;
    let usdt = world.wallet_of(alice, "USDT").await;
    world.activate("CUSTOMER_CREDIT", world.usdc).await;
    world.activate("CUSTOMER_RESERVATION", world.usdc).await;
    world
        .deposit("chain:fixture:1", alice, world.usdc, 5 * UNIT)
        .await;

    // Suspension blocks spending even with a positive ledger balance.
    assert_eq!(
        admin
            .set_status(usdc.id, Status::Suspended, "Fraud review", staff)
            .await,
        Ok(Status::Suspended)
    );
    let suspended = world.service.wallet(alice, usdc.id).await.unwrap();
    assert_eq!(suspended.effective_status(), Status::Suspended);
    assert!(
        suspended
            .restrictions
            .contains(&Restriction::WalletSuspended)
    );
    assert_eq!(suspended.balance.unwrap().available.value(), 5 * UNIT);
    assert_eq!(
        world
            .poster
            .reserve(&world.reserve("withdrawal:1", alice, world.usdc, UNIT))
            .await,
        Err(LedgerError::NotEligible)
    );
    // Repeating a change is harmless and records nothing new.
    let history = |wallet: Uuid| {
        let pool = world.pool.clone();
        async move {
            sqlx::query_as::<_, (Option<String>, String, String, Option<Uuid>, String)>(
                "SELECT previous_status, new_status, actor_type, actor_id, database_login
                 FROM wallets.wallet_status_history WHERE wallet_profile_id = $1 ORDER BY created_at",
            )
            .bind(wallet)
            .fetch_all(&pool)
            .await
            .unwrap()
        }
    };
    let before = history(usdc.id).await.len();
    assert_eq!(
        admin
            .set_status(usdc.id, Status::Suspended, "Fraud review", staff)
            .await,
        Ok(Status::Suspended)
    );
    assert_eq!(history(usdc.id).await.len(), before);
    for next in [Status::Restricted, Status::Active] {
        assert_eq!(
            admin
                .set_status(usdc.id, next, "Review closed", staff)
                .await,
            Ok(next)
        );
    }
    // Closing needs an empty wallet; the funded one stays open.
    assert_eq!(
        admin
            .set_status(usdc.id, Status::Closed, "Customer request", staff)
            .await,
        Err(WalletError::NotEmpty)
    );
    assert_eq!(
        admin
            .set_status(usdt.id, Status::Closed, "Customer request", staff)
            .await,
        Ok(Status::Closed)
    );
    assert_eq!(
        admin
            .set_status(usdt.id, Status::Active, "Reopen", staff)
            .await,
        Err(WalletError::InvalidTransition),
        "closed is final"
    );
    assert_eq!(
        admin
            .set_status(usdc.id, Status::Provisioning, "Invalid", staff)
            .await,
        Err(WalletError::InvalidRequest)
    );
    // A closed wallet stays in the customer's history with its ledger accounts.
    let listed = world.wallets(alice).await;
    assert_eq!(listed.len(), 2);
    assert_eq!(
        listed.iter().find(|w| w.id == usdt.id).unwrap().status,
        Status::Closed
    );
    assert_eq!(
        count(
            &world.pool,
            &format!(
                "SELECT count(*) FROM ledger.accounts WHERE wallet_profile_id = '{}'",
                usdt.id
            )
        )
        .await,
        2
    );
    // Only the authenticated, permitted operator may act.
    let unbound = PgWalletAdmin::new(world.pool.clone());
    assert_eq!(
        unbound
            .set_status(usdc.id, Status::Suspended, "Impersonation", staff)
            .await,
        Err(WalletError::OperatorNotAuthenticated)
    );
    assert_eq!(
        outsider_admin
            .set_status(usdc.id, Status::Suspended, "No permission", outsider)
            .await,
        Err(WalletError::ApprovalRequired)
    );
    assert_eq!(
        admin
            .set_status(usdc.id, Status::Suspended, "Forged", outsider)
            .await,
        Err(WalletError::OperatorNotAuthenticated)
    );
    // A wallet still being provisioned cannot be changed by an operator.
    let bob = customer(&world.pool, "bob", "CONTACT_VERIFIED").await;
    let pending: Uuid = sqlx::query_scalar(
        "INSERT INTO wallets.wallet_profiles(user_id, asset_id, asset_symbol, status, provisioning_status)
         VALUES ($1, $2, 'USDC', 'PROVISIONING', 'PENDING') RETURNING id",
    )
    .bind(bob)
    .bind(world.usdc)
    .fetch_one(&world.pool)
    .await
    .unwrap();
    assert_eq!(
        admin
            .set_status(pending, Status::Suspended, "Early", staff)
            .await,
        Err(WalletError::InvalidTransition)
    );
    // Every change is attributed to the operator and their login.
    let trail = history(usdc.id).await;
    assert_eq!(
        trail
            .iter()
            .map(|(from, to, kind, actor, login)| (
                from.clone(),
                to.clone(),
                kind.clone(),
                *actor,
                login.clone()
            ))
            .skip(2)
            .collect::<Vec<_>>(),
        vec![
            (
                Some("ACTIVE".into()),
                "SUSPENDED".into(),
                "ADMIN".into(),
                Some(staff),
                OPERATOR_LOGIN.into()
            ),
            (
                Some("SUSPENDED".into()),
                "RESTRICTED".into(),
                "ADMIN".into(),
                Some(staff),
                OPERATOR_LOGIN.into()
            ),
            (
                Some("RESTRICTED".into()),
                "ACTIVE".into(),
                "ADMIN".into(),
                Some(staff),
                OPERATOR_LOGIN.into()
            ),
        ]
    );
    assert_eq!(
        count(
            &world.pool,
            "SELECT count(*) FROM audit.audit_events WHERE action = 'WALLET_STATUS_CHANGED'"
        )
        .await,
        4
    );
    // Neither customers nor direct edits bypass the transition rules.
    for (role, sql, expected) in [
        (
            None,
            format!(
                "UPDATE wallets.wallet_profiles SET status = 'ACTIVE' WHERE id = '{}'",
                usdt.id
            ),
            "WA003",
        ),
        (
            None,
            format!(
                "DELETE FROM wallets.wallet_profiles WHERE id = '{}'",
                usdt.id
            ),
            "55000",
        ),
        (
            None,
            format!(
                "UPDATE wallets.wallet_profiles SET user_id = '{bob}' WHERE id = '{}'",
                usdc.id
            ),
            "55000",
        ),
        (
            Some("deypay_app"),
            format!(
                "SELECT wallets.set_wallet_status('{}', 'ACTIVE', 'self-service', '{alice}')",
                usdc.id
            ),
            "42501",
        ),
    ] {
        assert_eq!(
            sqlstate(&world.pool, role, &sql).await.as_deref(),
            Some(expected),
            "{sql}"
        );
    }
    assert!(world.violations().await.is_empty());
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn restrictions_fail_closed_and_never_bypass_the_ledger(pool: PgPool) {
    use deypay_application::wallets::WalletStatus as Status;
    let world = setup(pool).await;
    let alice = customer(&world.pool, "alice", "KYC_PENDING").await;
    let wallet = world.wallet_of(alice, "USDC").await;
    assert_eq!(
        wallet.restrictions,
        vec![
            Restriction::KycRequired,
            Restriction::FinancialActivityNotEnabled
        ]
    );
    world.activate("CUSTOMER_TRANSFER", world.usdc).await;
    world.activate("CUSTOMER_RESERVATION", world.usdc).await;
    world.activate("CUSTOMER_CREDIT", world.usdc).await;
    assert_eq!(
        world.wallet_of(alice, "USDC").await.restrictions,
        vec![Restriction::KycRequired]
    );
    // The wallet being active does not let an unapproved customer spend.
    world
        .deposit("chain:fixture:1", alice, world.usdc, 3 * UNIT)
        .await;
    assert_eq!(
        world
            .poster
            .reserve(&world.reserve("withdrawal:1", alice, world.usdc, UNIT))
            .await,
        Err(LedgerError::NotEligible)
    );
    sqlx::query("UPDATE identity.users SET status = 'KYC_APPROVED' WHERE id = $1")
        .bind(alice)
        .execute(&world.pool)
        .await
        .unwrap();
    let clear = world.wallet_of(alice, "USDC").await;
    assert!(clear.restrictions.is_empty());
    assert!(
        !clear.can_send(world.service.policy_approved()),
        "the shipped application gate still refuses"
    );
    assert_eq!(
        world.wallet_of(alice, "USDT").await.restrictions,
        vec![Restriction::FinancialActivityNotEnabled],
        "activation is per asset"
    );
    // Compliance restrictions, holds and asset suspension each fail closed.
    let restriction: Uuid = sqlx::query_scalar(
        "INSERT INTO compliance.account_restrictions(user_id, restriction_type, reason_code, active)
         VALUES ($1, 'FINANCIAL', 'TEST', true) RETURNING id",
    )
    .bind(alice)
    .fetch_one(&world.pool)
    .await
    .unwrap();
    let restricted = world.wallet_of(alice, "USDC").await;
    assert_eq!(
        restricted.restrictions,
        vec![Restriction::AccountRestricted]
    );
    assert_eq!(restricted.effective_status(), Status::Restricted);
    sqlx::query("UPDATE compliance.account_restrictions SET active = false, lifted_at = now() WHERE id = $1")
        .bind(restriction)
        .execute(&world.pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO ledger.asset_holds(asset_id, reason) VALUES ($1, 'Reconciliation shortfall')",
    )
    .bind(world.usdc)
    .execute(&world.pool)
    .await
    .unwrap();
    let held = world.wallet_of(alice, "USDC").await;
    assert_eq!(held.restrictions, vec![Restriction::AssetOnHold]);
    assert_eq!(held.effective_status(), Status::Restricted);
    assert_eq!(
        world
            .poster
            .reserve(&world.reserve("withdrawal:2", alice, world.usdc, UNIT))
            .await,
        Err(LedgerError::AssetOnHold)
    );
    sqlx::query(
        "UPDATE wallets.assets SET suspended_at = now(), suspension_reason = 'Incident' WHERE id = $1",
    )
    .bind(world.usdt)
    .execute(&world.pool)
    .await
    .unwrap();
    assert!(
        world
            .wallet_of(alice, "USDT")
            .await
            .restrictions
            .contains(&Restriction::AssetUnavailable)
    );
    assert!(world.violations().await.is_empty());
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn deposit_addresses_follow_the_provider_boundary(pool: PgPool) {
    let world = setup(pool).await;
    world.deposit_network().await;
    let alice = customer(&world.pool, "alice", "KYC_APPROVED").await;
    let pending_kyc = customer(&world.pool, "pending", "KYC_PENDING").await;
    let alice_usdc = world.wallet_of(alice, "USDC").await.id;
    let alice_usdt = world.wallet_of(alice, "USDT").await.id;
    let pending_usdc = world.wallet_of(pending_kyc, "USDC").await.id;
    let request = |customer: Uuid, wallet: Uuid| {
        world
            .service
            .request_deposit_address(customer, wallet, NETWORK)
    };

    // Fail closed: no credit activation, no provider, no KYC, unsupported network.
    assert_eq!(
        request(alice, alice_usdc).await,
        Err(WalletError::DepositsNotEnabled)
    );
    world.activate("CUSTOMER_CREDIT", world.usdc).await;
    world.activate("CUSTOMER_CREDIT", world.usdt).await;
    assert_eq!(
        request(alice, alice_usdc).await,
        Err(WalletError::ProviderUnavailable)
    );
    assert_eq!(
        world.address_state(alice, alice_usdc).await,
        AddressState::Unavailable
    );
    assert_eq!(
        request(pending_kyc, pending_usdc).await,
        Err(WalletError::KycRequired)
    );
    assert_eq!(
        world
            .service
            .request_deposit_address(alice, alice_usdc, "sol-fixture")
            .await,
        Err(WalletError::UnsupportedNetwork)
    );
    assert_eq!(
        world
            .service
            .request_deposit_address(alice, alice_usdc, "Bad Network")
            .await,
        Err(WalletError::UnsupportedNetwork)
    );
    assert_eq!(
        count(
            &world.pool,
            "SELECT count(*) FROM wallets.wallet_provisioning_requests"
        )
        .await,
        0
    );

    // With an approved provider, a durable request is opened exactly once.
    let provider_a = world.provider("FIXTURE_A", 50).await;
    assert_eq!(
        world.address_state(alice, alice_usdc).await,
        AddressState::NotRequested
    );
    assert_eq!(
        request(alice, alice_usdc).await,
        Ok(DepositAddress::Pending { created: true })
    );
    assert_eq!(
        request(alice, alice_usdc).await,
        Ok(DepositAddress::Pending { created: false })
    );
    assert_eq!(
        world.address_state(alice, alice_usdc).await,
        AddressState::Pending
    );
    let bob = customer(&world.pool, "bob", "KYC_APPROVED").await;
    let bob_usdc = world.wallet_of(bob, "USDC").await.id;
    assert_eq!(
        request(bob, alice_usdc).await,
        Err(WalletError::NotFound),
        "nobody provisions another customer's address"
    );
    let reference: String =
        sqlx::query_scalar("SELECT request_reference FROM wallets.wallet_provisioning_requests")
            .fetch_one(&world.pool)
            .await
            .unwrap();
    assert!(
        reference.starts_with("deypay-addr-"),
        "DEYPAY owns the request identity"
    );

    // Provisioned and confirmed: only then is the address shown.
    let a = ScriptedProvider::new("FIXTURE_A");
    let b = ScriptedProvider::new("FIXTURE_B");
    let store = PgProvisioning::new(world.pool.clone());
    let address_one = "0x00000000000000000000000000000000000000A1";
    a.answer(provisioned(address_one, "a-wallet-1"))
        .confirm(confirmed());
    let report = run(&store, vec![&a]).await.unwrap();
    assert_eq!((report.submitted, report.activated), (1, 1));
    let shown = AddressState::Active {
        address: address_one.to_ascii_lowercase(),
        memo: None,
    };
    assert_eq!(world.address_state(alice, alice_usdc).await, shown);
    assert_eq!(
        request(alice, alice_usdc).await,
        Ok(DepositAddress::Active {
            address: address_one.to_ascii_lowercase(),
            memo: None
        })
    );
    // One address may serve the same customer's other token with the same provider.
    request(alice, alice_usdt).await.unwrap();
    a.answer(provisioned(address_one, "a-wallet-1"))
        .confirm(confirmed());
    assert_eq!(run(&store, vec![&a]).await.unwrap().activated, 1);
    assert!(matches!(
        world.address_state(alice, alice_usdt).await,
        AddressState::Active { .. }
    ));
    // But never another customer's: the assignment is refused and nothing shows.
    request(bob, bob_usdc).await.unwrap();
    a.answer(provisioned(address_one, "a-wallet-1"));
    assert_eq!(run(&store, vec![&a]).await, Err(WalletError::Conflict));
    assert_eq!(
        world.address_state(bob, bob_usdc).await,
        AddressState::Pending
    );
    a.answer(provisioned(
        "0x00000000000000000000000000000000000000b2",
        "a-wallet-2",
    ))
    .confirm(confirmed());
    let report = run(&store, vec![&a]).await.unwrap();
    assert_eq!(
        (report.submitted, report.looked_up, report.activated),
        (0, 1, 1)
    );
    assert_eq!(
        a.requests(),
        3,
        "the conflicting attempt was looked up, not resubmitted"
    );

    // Unknown outcome, then routing moves to another provider: the request
    // stays with its original provider and is only ever looked up.
    let carol = customer(&world.pool, "carol", "KYC_APPROVED").await;
    let carol_usdc = world.wallet_of(carol, "USDC").await.id;
    request(carol, carol_usdc).await.unwrap();
    a.answer(ProviderResult::Unknown);
    assert_eq!(run(&store, vec![&a]).await.unwrap().submitted, 1);
    let provider_b = world.provider("FIXTURE_B", 1).await;
    sqlx::query(
        "UPDATE integrations.provider_capabilities SET operational = false WHERE provider_id = $1",
    )
    .bind(provider_a)
    .execute(&world.pool)
    .await
    .unwrap();
    assert_eq!(
        request(carol, carol_usdc).await,
        Ok(DepositAddress::Pending { created: false }),
        "an uncertain request is never repeated elsewhere"
    );
    assert_eq!(run(&store, vec![&b]).await.unwrap().skipped, 1);
    a.answer(ProviderResult::Unknown);
    a.answer(provisioned(
        "0x00000000000000000000000000000000000000c3",
        "a-wallet-3",
    ))
    .confirm(confirmed());
    assert_eq!(run(&store, vec![&a]).await.unwrap().looked_up, 1);
    assert_eq!(
        world.address_state(carol, carol_usdc).await,
        AddressState::Pending
    );
    assert_eq!(run(&store, vec![&a]).await.unwrap().activated, 1);
    assert_eq!(a.requests(), 4, "carol's request was submitted once");
    let (carol_provider, carol_status): (Uuid, String) = sqlx::query_as(
        "SELECT provider_id, status FROM wallets.wallet_addresses WHERE wallet_profile_id = $1",
    )
    .bind(carol_usdc)
    .fetch_one(&world.pool)
    .await
    .unwrap();
    assert_eq!(
        (carol_provider, carol_status.as_str()),
        (provider_a, "ACTIVE")
    );

    // New requests go to the newly selected provider. A request that was
    // certainly not received may be submitted again; a mismatched address is
    // rejected and an unconfirmed one is never shown.
    let dave = customer(&world.pool, "dave", "KYC_APPROVED").await;
    let dave_usdc = world.wallet_of(dave, "USDC").await.id;
    request(dave, dave_usdc).await.unwrap();
    let dave_provider: Uuid = sqlx::query_scalar(
        "SELECT provider_id FROM wallets.wallet_provisioning_requests WHERE wallet_profile_id = $1",
    )
    .bind(dave_usdc)
    .fetch_one(&world.pool)
    .await
    .unwrap();
    assert_eq!(dave_provider, provider_b);
    b.answer(ProviderResult::NotReceived);
    assert_eq!(run(&store, vec![&b]).await.unwrap().submitted, 1);
    b.answer(provisioned(
        "0x00000000000000000000000000000000000000d4",
        "b-wallet-1",
    ))
    .confirm(AddressConfirmation::Mismatch {
        reason: "Provider reports another address".into(),
    });
    assert_eq!(run(&store, vec![&b]).await.unwrap().rejected, 1);
    assert_eq!(
        world.address_state(dave, dave_usdc).await,
        AddressState::NotRequested
    );
    assert_eq!(
        request(dave, dave_usdc).await,
        Ok(DepositAddress::Pending { created: true })
    );
    b.answer(provisioned(
        "0x00000000000000000000000000000000000000e5",
        "b-wallet-2",
    ));
    run(&store, vec![&b]).await.unwrap();
    assert_eq!(
        world.address_state(dave, dave_usdc).await,
        AddressState::Pending,
        "an unverified address is not deposit-ready"
    );
    assert_eq!(
        request(dave, dave_usdc).await,
        Ok(DepositAddress::Pending { created: false })
    );
    assert_eq!(b.requests(), 3);

    // Retiring an address keeps its record and provider; the next request
    // goes to the provider routing selects now.
    let alice_address: Uuid = sqlx::query_scalar(
        "SELECT id FROM wallets.wallet_addresses WHERE wallet_profile_id = $1 AND status = 'ACTIVE'",
    )
    .bind(alice_usdc)
    .fetch_one(&world.pool)
    .await
    .unwrap();
    store
        .retire(alice_address, "Provider change")
        .await
        .unwrap();
    assert_eq!(
        request(alice, alice_usdc).await,
        Ok(DepositAddress::Pending { created: true })
    );
    let history: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT provider_id, status FROM wallets.wallet_addresses WHERE wallet_profile_id = $1 ORDER BY created_at",
    )
    .bind(alice_usdc)
    .fetch_all(&world.pool)
    .await
    .unwrap();
    assert_eq!(history, vec![(provider_a, "RETIRED".to_owned())]);
    let alice_provider: Uuid = sqlx::query_scalar(
        "SELECT provider_id FROM wallets.wallet_provisioning_requests
         WHERE wallet_profile_id = $1 AND status = 'PENDING'",
    )
    .bind(alice_usdc)
    .fetch_one(&world.pool)
    .await
    .unwrap();
    assert_eq!(alice_provider, provider_b);

    // Addresses are never edited in place, and only the provisioner records them.
    for (role, sql, expected) in [
        (
            None,
            format!(
                "UPDATE wallets.wallet_addresses SET address = '0x00000000000000000000000000000000000000ff'
                 WHERE id = '{alice_address}'"
            ),
            "55000",
        ),
        (None, "DELETE FROM wallets.wallet_addresses".to_owned(), "55000"),
        (
            Some("deypay_app"),
            "SELECT * FROM wallets.provisioning_work(10)".to_owned(),
            "42501",
        ),
        (
            Some("deypay_app"),
            format!("SELECT wallets.verify_deposit_address('{alice_address}', 'forged', sha256('x'))"),
            "42501",
        ),
        (
            Some("deypay_wallet_provisioner"),
            format!("SELECT wallets.initialize_customer_wallets('{alice}')"),
            "42501",
        ),
        (
            Some("deypay_wallet_provisioner"),
            format!("SELECT wallets.set_wallet_status('{alice_usdc}', 'ACTIVE', 'x', '{alice}')"),
            "42501",
        ),
    ] {
        assert_eq!(
            sqlstate(&world.pool, role, &sql).await.as_deref(),
            Some(expected),
            "{sql}"
        );
    }
    // Provider work never touched balances or the ledger's integrity.
    assert_eq!(
        count(&world.pool, "SELECT count(*) FROM ledger.journals").await,
        0
    );
    assert!(world.violations().await.is_empty());
}
