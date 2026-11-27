//! PostgreSQL tests for internal transfers (IMP-P7-001–010, test strategy
//! §§19, 26, 29, 32). Every scenario runs the production migrations and grant
//! scripts; customer calls run as the restricted `deypay_app` role. The only
//! fixture standing in for unfinished work is `ApprovedForTests`, an approved
//! financial policy that defers to the database's own checks.
use deypay_application::ledger::*;
use deypay_application::transfers::*;
use deypay_domain::ledger::{Direction as Side, EventIdentity, JournalDraft, PostingLine};
use deypay_domain::units::PostingAmount;
use deypay_infrastructure::MIGRATOR;
use deypay_infrastructure::ledger::{LedgerRole, PgLedger};
use deypay_infrastructure::transfers::{PgTransfers, generate_proof_token};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use std::sync::{Arc, Once};
use uuid::Uuid;

const SCRIPTS: [&str; 6] = [
    include_str!("../../../../infrastructure/postgres/ledger-roles.psql"),
    include_str!("../../../../infrastructure/postgres/wallet-roles.psql"),
    include_str!("../../../../infrastructure/postgres/payments-roles.psql"),
    include_str!("../../../../infrastructure/postgres/ledger-grants.psql"),
    include_str!("../../../../infrastructure/postgres/wallet-grants.psql"),
    include_str!("../../../../infrastructure/postgres/payments-grants.psql"),
];
const UNIT: u128 = 1_000_000;
const CEILING: &str = "10000000000000000000000000000000000000";

/// Test-only stand-in for a future approved financial policy: it allows the
/// request and leaves every decision to the database.
struct ApprovedForTests;

impl FinancialGate for ApprovedForTests {
    async fn authorize(&self, _: Uuid, _: GatedOperation<'_>) -> Result<(), LedgerError> {
        Ok(())
    }
    fn policy_approved(&self) -> bool {
        true
    }
}

type Service = TransferService<PgTransfers, ApprovedForTests>;

struct World {
    pool: PgPool,
    app_pool: PgPool,
    service: Arc<Service>,
    poster: PgLedger,
    usdc: Uuid,
    usdt: Uuid,
}

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
    static LOGGING: Once = Once::new();
    LOGGING.call_once(|| deypay_common::runtime::init_logging("warn").unwrap());
    for script in SCRIPTS {
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
        service: Arc::new(TransferService::new(
            PgTransfers::new(app_pool.clone()),
            ApprovedForTests,
        )),
        poster: PgLedger::acting_as(pool.clone(), LedgerRole::Poster),
        app_pool,
        pool,
        usdc,
        usdt,
    }
}

fn to(recipient: Uuid, asset: &'static str, amount: &'static str) -> TransferInput<'static> {
    TransferInput {
        recipient,
        asset,
        amount,
        description: None,
    }
}

async fn count(pool: &PgPool, sql: &str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}

/// Runs SQL as `role` and returns its SQLSTATE.
async fn sqlstate(pool: &PgPool, role: &str, sql: &str) -> Option<String> {
    let mut tx = pool.begin().await.unwrap();
    sqlx::query(&format!("SET LOCAL ROLE {role}"))
        .execute(&mut *tx)
        .await
        .unwrap();
    let error = sqlx::raw_sql(sql).execute(&mut *tx).await.err();
    error.and_then(|e| {
        e.as_database_error()
            .and_then(|db| db.code())
            .map(|c| c.into_owned())
    })
}

impl World {
    /// A customer with a verified email, an active username and a profile name.
    async fn customer(&self, name: &str, status: &str) -> Uuid {
        let id: Uuid = sqlx::query_scalar(
            "INSERT INTO identity.users(email, status, email_verified_at) VALUES ($1, $2, now()) RETURNING id",
        )
        .bind(format!("{}@customer.test", name.to_lowercase()))
        .bind(status)
        .fetch_one(&self.pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO customers.usernames(user_id, username) VALUES ($1, $2)")
            .bind(id)
            .bind(name)
            .execute(&self.pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO customers.profiles(user_id, first_name, last_name, country_code)
             VALUES ($1, initcap(split_part($2, '_', 1)), 'Tester', 'NG')",
        )
        .bind(id)
        .bind(name)
        .execute(&self.pool)
        .await
        .unwrap();
        id
    }

    /// An active session; returns the digest the API derives from its cookie.
    async fn session(&self, customer: Uuid) -> String {
        let digest = hex::encode(Sha256::digest(generate_proof_token().unwrap()));
        sqlx::query(
            "INSERT INTO identity.sessions(user_id, token_hash, expires_at, absolute_expires_at)
             VALUES ($1, $2, now() + interval '1 hour', now() + interval '12 hours')",
        )
        .bind(customer)
        .bind(&digest)
        .execute(&self.pool)
        .await
        .unwrap();
        digest
    }

    async fn admin(&self, sql: &str) {
        sqlx::raw_sql(sql).execute(&self.pool).await.unwrap();
    }

    async fn activate(&self, capability: &str, asset: Uuid, ceiling: &str) {
        sqlx::query(
            "INSERT INTO ledger.capability_activations(capability, asset_id, approval_reference, max_operation_minor)
             VALUES ($1, $2, 'TEST-FIXTURE', $3::numeric)",
        )
        .bind(capability)
        .bind(asset)
        .bind(ceiling)
        .execute(&self.pool)
        .await
        .unwrap();
    }

    async fn lookup_budget(&self, per_window: i32) {
        sqlx::query(
            "INSERT INTO payments.recipient_lookup_policy(window_seconds, lookups_per_window, approval_reference)
             VALUES (600, $1, 'TEST-FIXTURE')",
        )
        .bind(per_window)
        .execute(&self.pool)
        .await
        .unwrap();
    }

    /// A verified deposit credited through the ledger's poster role.
    async fn fund(&self, customer: Uuid, asset: Uuid, value: u128) {
        sqlx::query("SELECT wallets.initialize_customer_wallets($1)")
            .bind(customer)
            .execute(&self.pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO ledger.capability_activations(capability, asset_id, approval_reference, max_operation_minor)
             VALUES ('CUSTOMER_CREDIT', $1, 'TEST-FIXTURE', $2::numeric) ON CONFLICT DO NOTHING",
        )
        .bind(asset)
        .bind(CEILING)
        .execute(&self.pool)
        .await
        .unwrap();
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
        let available: Uuid = sqlx::query_scalar(
            "SELECT id FROM ledger.accounts WHERE owner_id = $1 AND asset_id = $2 AND customer_role = 'AVAILABLE'",
        )
        .bind(customer)
        .bind(asset)
        .fetch_one(&self.pool)
        .await
        .unwrap();
        let amount = PostingAmount::from_value(value).unwrap();
        self.poster
            .post(&PostingCommand {
                identity: EventIdentity::new(
                    "DEPOSIT_CONFIRMED",
                    &format!("chain:fixture:{}", generate_proof_token().unwrap()),
                    "RECOGNITION",
                )
                .unwrap(),
                source_transaction: None,
                journal: JournalDraft::new(vec![
                    PostingLine {
                        account_id: custody,
                        asset_id: asset,
                        direction: Side::Debit,
                        amount,
                    },
                    PostingLine {
                        account_id: available,
                        asset_id: asset,
                        direction: Side::Credit,
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

    async fn available(&self, customer: Uuid, asset: Uuid) -> u128 {
        let text: Option<String> =
            sqlx::query_scalar("SELECT available_minor FROM ledger.customer_balance($1, $2)")
                .bind(customer)
                .bind(asset)
                .fetch_optional(&self.pool)
                .await
                .unwrap();
        text.map_or(0, |t| t.parse().unwrap())
    }

    /// Authorizes and confirms the exact terms; returns the single-use token.
    async fn authorized(&self, customer: Uuid, session: &str, input: TransferInput<'_>) -> String {
        let challenge = self
            .service
            .authorize(customer, session, input)
            .await
            .unwrap();
        self.service
            .confirm(
                customer,
                session,
                challenge.id,
                generate_proof_token().unwrap(),
            )
            .await
            .unwrap()
            .token
    }

    async fn send(
        &self,
        customer: Uuid,
        session: &str,
        key: &str,
        input: TransferInput<'_>,
    ) -> Result<Executed, TransferError> {
        let token = self.authorized(customer, session, input).await;
        self.service
            .execute(customer, session, key, &token, input)
            .await
    }

    async fn violations(&self) -> Vec<String> {
        sqlx::query_scalar("SELECT kind FROM ledger.integrity_violations()")
            .fetch_all(&self.pool)
            .await
            .unwrap()
    }
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn transfers_move_exact_funds_once_and_both_parties_see_them(pool: PgPool) {
    let world = setup(pool).await;
    world.lookup_budget(50).await;
    let alice = world.customer("alice_k", "KYC_APPROVED").await;
    let bob = world.customer("bob_k", "KYC_APPROVED").await;
    let carol = world.customer("carol_k", "KYC_APPROVED").await;
    let session = world.session(alice).await;
    world.fund(alice, world.usdt, 100 * UNIT).await;

    // Resolution returns only the permitted identity details.
    assert_eq!(
        world.service.resolve(alice, "@Bob_K").await,
        Ok(Some(Recipient {
            id: bob,
            username: "bob_k".into(),
            display_name: Some("Bob Tester".into()),
            can_receive: true,
        }))
    );

    // Not yet activated: refused before any challenge exists.
    assert_eq!(
        world
            .service
            .authorize(alice, &session, to(bob, "USDT", "25.5"))
            .await,
        Err(TransferError::NotEnabled)
    );
    world
        .activate("CUSTOMER_TRANSFER", world.usdt, CEILING)
        .await;

    let input = TransferInput {
        description: Some("  Lunch  "),
        ..to(bob, "USDT", "25.5")
    };
    let challenge = world
        .service
        .authorize(alice, &session, input)
        .await
        .unwrap();
    assert_eq!(
        (
            challenge.recipient,
            challenge.recipient_username.as_deref(),
            challenge.asset.as_str(),
            challenge.format(challenge.amount),
            challenge.format(challenge.fee),
            challenge.description.as_deref(),
            challenge.verification_method.as_str(),
        ),
        (
            bob,
            Some("bob_k"),
            "USDT",
            "25.500000".to_owned(),
            "0.000000".to_owned(),
            Some("Lunch"),
            "CONFIRMATION"
        )
    );
    let token = world
        .service
        .confirm(
            alice,
            &session,
            challenge.id,
            generate_proof_token().unwrap(),
        )
        .await
        .unwrap()
        .token;
    let executed = world
        .service
        .execute(alice, &session, "transfer-key-1", &token, input)
        .await
        .unwrap();
    assert!(!executed.replayed);
    let sent = executed.transfer.clone();
    assert_eq!(
        (
            sent.status.as_str(),
            sent.direction,
            sent.amount.value(),
            sent.fee.value(),
            sent.counterparty.id,
            sent.description.as_deref(),
        ),
        (
            "COMPLETED",
            Direction::Debit,
            25_500_000,
            0,
            bob,
            Some("Lunch")
        )
    );
    assert!(sent.reference.starts_with("DP-") && sent.reference.len() == 19);

    // Exact balances, one balanced journal, unchanged aggregate liability.
    assert_eq!(world.available(alice, world.usdt).await, 74_500_000);
    assert_eq!(world.available(bob, world.usdt).await, 25_500_000);
    assert_eq!(
        count(
            &world.pool,
            "SELECT count(*) FROM ledger.journals WHERE event_type = 'INTERNAL_TRANSFER_EXECUTED'"
        )
        .await,
        1
    );
    let (lines, debit, credit): (i64, String, String) = sqlx::query_as(
        "SELECT count(*), sum(e.amount_minor) FILTER (WHERE e.direction = 'DEBIT')::text,
                sum(e.amount_minor) FILTER (WHERE e.direction = 'CREDIT')::text
         FROM ledger.entries e JOIN payments.internal_transfers t ON t.ledger_journal_id = e.journal_id",
    )
    .fetch_one(&world.pool)
    .await
    .unwrap();
    assert_eq!(
        (lines, debit.as_str(), credit.as_str()),
        (2, "25500000", "25500000")
    );
    assert_eq!(
        world.available(alice, world.usdt).await + world.available(bob, world.usdt).await,
        100 * UNIT
    );

    // An identical retry returns the same transfer; nothing moves twice.
    let replay = world
        .service
        .execute(alice, &session, "transfer-key-1", &token, input)
        .await
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.transfer, sent);
    assert_eq!(
        world
            .service
            .execute(
                alice,
                &session,
                "transfer-key-1",
                &token,
                to(bob, "USDT", "1")
            )
            .await,
        Err(TransferError::IdempotencyConflict)
    );
    assert_eq!(
        world
            .service
            .execute(alice, &session, "transfer-key-2", &token, input)
            .await,
        Err(TransferError::AuthorizationInvalid),
        "a consumed proof is never accepted again"
    );
    assert_eq!(world.available(alice, world.usdt).await, 74_500_000);

    // Both participants see the same transaction and reference; others see nothing.
    let received = world.service.transaction(bob, sent.id).await.unwrap();
    assert_eq!(
        (
            received.reference.as_str(),
            received.direction,
            received.counterparty.id,
            received.counterparty.username.as_deref(),
            received.fee.value(),
        ),
        (
            sent.reference.as_str(),
            Direction::Credit,
            alice,
            Some("alice_k"),
            0
        )
    );
    assert_eq!(
        world.service.history(bob, None, 20).await.unwrap(),
        vec![received]
    );
    assert_eq!(
        world.service.transaction(carol, sent.id).await,
        Err(TransferError::NotFound)
    );
    assert!(
        world
            .service
            .history(carol, None, 20)
            .await
            .unwrap()
            .is_empty()
    );

    // The workflow, authorization, notification and audit evidence.
    let states: Vec<(Option<String>, String)> = sqlx::query_as(
        "SELECT previous_status::text, new_status::text FROM payments.transaction_status_history
         WHERE transaction_id = $1 ORDER BY created_at, new_status = 'COMPLETED', new_status <> 'CREATED',
             new_status = 'AUTHORIZED'",
    )
    .bind(sent.id)
    .fetch_all(&world.pool)
    .await
    .unwrap();
    assert_eq!(
        states,
        vec![
            (None, "CREATED".into()),
            (Some("CREATED".into()), "VALIDATED".into()),
            (Some("VALIDATED".into()), "AUTHORIZED".into()),
            (Some("AUTHORIZED".into()), "COMPLETED".into()),
        ]
    );
    assert_eq!(
        count(
            &world.pool,
            &format!(
                "SELECT count(*) FROM identity.transaction_proofs WHERE accepted_transaction_id = '{}'
                 AND consumed_at IS NOT NULL",
                sent.id
            )
        )
        .await,
        1
    );
    assert_eq!(
        count(
            &world.pool,
            &format!(
                "SELECT count(*) FROM integrations.outbox_events WHERE event_type = 'INTERNAL_TRANSFER_COMPLETED'
                 AND payload->>'recipient_user_id' = '{bob}' AND payload->>'sender_user_id' = '{alice}'"
            )
        )
        .await,
        1
    );
    assert_eq!(
        count(
            &world.pool,
            "SELECT count(*) FROM audit.audit_events WHERE action IN
                 ('TRANSACTION_AUTHORIZATION_REQUESTED', 'TRANSACTION_AUTHORIZED', 'INTERNAL_TRANSFER_COMPLETED')"
        )
        .await,
        3
    );

    // A completed transfer is immutable.
    for sql in [
        format!(
            "UPDATE payments.internal_transfers SET fee_minor = 0 WHERE transaction_id = '{}'",
            sent.id
        ),
        format!(
            "UPDATE payments.transactions SET description = 'x' WHERE id = '{}'",
            sent.id
        ),
        format!("DELETE FROM payments.transactions WHERE id = '{}'", sent.id),
    ] {
        let mut tx = world.pool.begin().await.unwrap();
        let error = sqlx::raw_sql(&sql).execute(&mut *tx).await.unwrap_err();
        assert_eq!(
            error
                .as_database_error()
                .and_then(|db| db.code())
                .as_deref(),
            Some("55000"),
            "{sql}"
        );
    }
    // Upgrade an acceptance in the original Phase 7 format. Further profile edits
    // cannot change its replay, and the migration must never post another journal.
    world
        .admin(&format!(
            "UPDATE payments.idempotency_records SET response_body = jsonb_build_object(
                 'transaction_id', transaction_id, 'status', 'COMPLETED', 'asset', 'USDT', 'amount_minor', '25500000')
             WHERE transaction_id = '{}'",
            sent.id
        ))
        .await;
    world
        .admin(include_str!(
            "../../../migrations/202611271602_internal_transfer_acceptance.sql"
        ))
        .await;
    world
        .admin(&format!(
            "UPDATE customers.profiles SET first_name = 'Updated' WHERE user_id = '{bob}'"
        ))
        .await;
    let upgraded = world
        .service
        .execute(alice, &session, "transfer-key-1", &token, input)
        .await
        .unwrap();
    assert!(upgraded.replayed);
    assert_eq!(upgraded.transfer, sent);
    assert_eq!(world.available(alice, world.usdt).await, 74_500_000);
    assert!(world.violations().await.is_empty());
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn refused_transfers_leave_no_trace(pool: PgPool) {
    let world = setup(pool).await;
    let alice = world.customer("alice_r", "KYC_APPROVED").await;
    let bob = world.customer("bob_r", "KYC_APPROVED").await;
    let pending = world.customer("pending_r", "KYC_PENDING").await;
    let session = world.session(alice).await;
    world
        .activate("CUSTOMER_TRANSFER", world.usdt, CEILING)
        .await;
    world
        .activate("CUSTOMER_TRANSFER", world.usdc, &(5 * UNIT).to_string())
        .await;
    world.fund(alice, world.usdt, 10 * UNIT).await;
    let transactions = || count(&world.pool, "SELECT count(*) FROM payments.transactions");

    // Insufficient funds: nothing is recorded and the proof stays unused, so
    // the same confirmation succeeds once the funds arrive.
    let token = world
        .authorized(alice, &session, to(bob, "USDT", "12"))
        .await;
    assert_eq!(
        world
            .service
            .execute(
                alice,
                &session,
                "short-key-01",
                &token,
                to(bob, "USDT", "12")
            )
            .await,
        Err(TransferError::InsufficientFunds)
    );
    assert_eq!(transactions().await, 0);
    world.fund(alice, world.usdt, 2 * UNIT).await;
    assert!(
        world
            .service
            .execute(
                alice,
                &session,
                "short-key-01",
                &token,
                to(bob, "USDT", "12")
            )
            .await
            .is_ok()
    );
    assert_eq!(world.available(alice, world.usdt).await, 0);
    let before = transactions().await;

    // Each refusal: no transaction, no journal change.
    world.fund(alice, world.usdt, 5 * UNIT).await;
    world.fund(alice, world.usdc, 10 * UNIT).await;
    assert_eq!(
        world
            .service
            .authorize(alice, &session, to(alice, "USDT", "1"))
            .await,
        Err(TransferError::SelfTransfer)
    );
    assert_eq!(
        world
            .service
            .authorize(alice, &session, to(Uuid::from_u128(42), "USDT", "1"))
            .await,
        Err(TransferError::RecipientNotFound)
    );
    assert_eq!(
        world
            .service
            .authorize(alice, &session, to(pending, "USDT", "1"))
            .await,
        Err(TransferError::RecipientUnavailable),
        "recipients need KYC approval (engineering default)"
    );
    assert_eq!(
        world
            .send(alice, &session, "usdc-key-01", to(bob, "USDC", "6"))
            .await,
        Err(TransferError::LimitExceeded),
        "above the activated per-operation ceiling"
    );
    assert!(
        world
            .send(alice, &session, "usdc-key-02", to(bob, "USDC", "5"))
            .await
            .is_ok()
    );
    let before_restrictions = transactions().await;
    assert_eq!(before_restrictions, before + 1);

    let cases: [(&str, String, String, TransferError); 6] = [
        (
            "recipient-restricted",
            format!(
                "INSERT INTO compliance.account_restrictions(user_id, restriction_type, reason_code, active)
                 VALUES ('{bob}', 'FINANCIAL', 'TEST', true)"
            ),
            format!("UPDATE compliance.account_restrictions SET active = false, lifted_at = now() WHERE user_id = '{bob}'"),
            TransferError::RecipientUnavailable,
        ),
        (
            "recipient-suspended",
            format!(
                "UPDATE wallets.wallet_profiles SET status = 'SUSPENDED' WHERE user_id = '{bob}' AND asset_id = '{}'",
                world.usdt
            ),
            format!(
                "UPDATE wallets.wallet_profiles SET status = 'ACTIVE' WHERE user_id = '{bob}' AND asset_id = '{}'",
                world.usdt
            ),
            TransferError::RecipientUnavailable,
        ),
        (
            "sender-kyc",
            format!("UPDATE identity.users SET status = 'KYC_PENDING' WHERE id = '{alice}'"),
            format!("UPDATE identity.users SET status = 'KYC_APPROVED' WHERE id = '{alice}'"),
            TransferError::KycRequired,
        ),
        (
            "sender-restricted",
            format!(
                "INSERT INTO compliance.account_restrictions(user_id, restriction_type, reason_code, active)
                 VALUES ('{alice}', 'FINANCIAL', 'TEST', true)"
            ),
            format!("UPDATE compliance.account_restrictions SET active = false, lifted_at = now() WHERE user_id = '{alice}'"),
            TransferError::Restricted,
        ),
        (
            "sender-wallet-restricted",
            format!(
                "UPDATE wallets.wallet_profiles SET status = 'RESTRICTED' WHERE user_id = '{alice}' AND asset_id = '{}'",
                world.usdt
            ),
            format!(
                "UPDATE wallets.wallet_profiles SET status = 'ACTIVE' WHERE user_id = '{alice}' AND asset_id = '{}'",
                world.usdt
            ),
            TransferError::Restricted,
        ),
        (
            "asset-hold",
            format!(
                "INSERT INTO ledger.asset_holds(asset_id, reason) VALUES ('{}', 'Reconciliation shortfall')",
                world.usdt
            ),
            format!(
                "UPDATE ledger.asset_holds SET active = false, lifted_at = now(), lifted_by = '{alice}',
                 lift_reason = 'Resolved' WHERE active"
            ),
            TransferError::AssetUnavailable,
        ),
    ];
    for (name, revoke, restore, expected) in cases {
        // Authorized while eligible, refused at execution after the change.
        let fresh = world.session(alice).await;
        let token = world.authorized(alice, &fresh, to(bob, "USDT", "1")).await;
        world.admin(&revoke).await;
        // An identity change asks sessions to refresh; a refreshed cookie
        // keeps the same session, which the test reproduces directly.
        world
            .admin("UPDATE identity.sessions SET refresh_required = false")
            .await;
        assert_eq!(
            world
                .service
                .execute(
                    alice,
                    &fresh,
                    &format!("{name}-key"),
                    &token,
                    to(bob, "USDT", "1")
                )
                .await,
            Err(expected),
            "{name}"
        );
        assert_eq!(
            world
                .service
                .authorize(alice, &fresh, to(bob, "USDT", "1"))
                .await,
            Err(expected),
            "{name}: refused before confirmation too"
        );
        world.admin(&restore).await;
        world
            .admin("UPDATE identity.sessions SET refresh_required = false")
            .await;
    }
    assert_eq!(transactions().await, before_restrictions);

    // A RESTRICTED wallet blocks sending but still receives.
    world
        .admin(&format!(
            "UPDATE wallets.wallet_profiles SET status = 'RESTRICTED' WHERE user_id = '{bob}' AND asset_id = '{}'",
            world.usdt
        ))
        .await;
    assert!(
        world
            .send(
                alice,
                &session,
                "restricted-recipient",
                to(bob, "USDT", "1")
            )
            .await
            .is_ok()
    );
    assert!(world.violations().await.is_empty());
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn authorization_is_single_use_and_bound_to_terms_session_and_time(pool: PgPool) {
    let world = setup(pool).await;
    let alice = world.customer("alice_a", "KYC_APPROVED").await;
    let bob = world.customer("bob_a", "KYC_APPROVED").await;
    let carol = world.customer("carol_a", "KYC_APPROVED").await;
    let session = world.session(alice).await;
    let other_session = world.session(alice).await;
    let carol_session = world.session(carol).await;
    world
        .activate("CUSTOMER_TRANSFER", world.usdt, CEILING)
        .await;
    world.fund(alice, world.usdt, 50 * UNIT).await;
    let terms = to(bob, "USDT", "2");
    // Even simultaneous reviews in one session leave just one confirmable challenge.
    let (first, second) = tokio::join!(
        world.service.authorize(alice, &session, terms),
        world.service.authorize(alice, &session, terms)
    );
    let challenges = [first.unwrap().id, second.unwrap().id];
    let open: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM identity.verification_challenges WHERE id = ANY($1) AND revoked_at IS NULL",
    )
    .bind(challenges.as_slice())
    .fetch_one(&world.pool)
    .await
    .unwrap();
    assert_eq!(open, 1);

    let short_session = world.session(alice).await;
    sqlx::query("UPDATE identity.sessions SET expires_at = now() + interval '1 minute' WHERE token_hash = $1")
        .bind(&short_session)
        .execute(&world.pool)
        .await
        .unwrap();
    let short_token = world.authorized(alice, &short_session, terms).await;
    let within_session: bool = sqlx::query_scalar(
        "SELECT p.expires_at <= s.expires_at FROM identity.transaction_proofs p
         JOIN identity.verification_challenges c ON c.id = p.challenge_id
         JOIN identity.sessions s ON s.id = c.session_id WHERE p.proof_hash = $1",
    )
    .bind(token_digest(&short_token).unwrap())
    .fetch_one(&world.pool)
    .await
    .unwrap();
    assert!(within_session);
    let token = world.authorized(alice, &session, terms).await;

    // Any material change invalidates the confirmation (SEC-TXN-005).
    for (name, changed) in [
        ("amount", to(bob, "USDT", "2.000001")),
        ("recipient", to(carol, "USDT", "2")),
        ("asset", to(bob, "USDC", "2")),
        (
            "description",
            TransferInput {
                description: Some("Rent"),
                ..terms
            },
        ),
    ] {
        assert_eq!(
            world
                .service
                .execute(alice, &session, &format!("changed-{name}"), &token, changed)
                .await,
            Err(TransferError::AuthorizationInvalid),
            "{name}"
        );
    }
    // Bound to the session that confirmed it and to the customer.
    assert_eq!(
        world
            .service
            .execute(alice, &other_session, "other-session", &token, terms)
            .await,
        Err(TransferError::AuthorizationInvalid)
    );
    assert_eq!(
        world
            .service
            .execute(carol, &carol_session, "other-customer", &token, terms)
            .await,
        Err(TransferError::AuthorizationInvalid)
    );
    assert!(
        world
            .service
            .execute(alice, &session, "bound-ok", &token, terms)
            .await
            .is_ok()
    );

    // A challenge completes once, only in its own session and for its customer;
    // a newer challenge in the session replaces an older one.
    let first = world
        .service
        .authorize(alice, &session, terms)
        .await
        .unwrap();
    let second = world
        .service
        .authorize(alice, &session, terms)
        .await
        .unwrap();
    for (customer, digest, challenge) in [
        (alice, &session, first.id),
        (alice, &other_session, second.id),
        (carol, &carol_session, second.id),
        (alice, &session, Uuid::from_u128(5)),
    ] {
        assert_eq!(
            world
                .service
                .confirm(customer, digest, challenge, generate_proof_token().unwrap())
                .await,
            Err(TransferError::AuthorizationInvalid)
        );
    }
    world
        .service
        .confirm(alice, &session, second.id, generate_proof_token().unwrap())
        .await
        .unwrap();
    assert_eq!(
        world
            .service
            .confirm(alice, &session, second.id, generate_proof_token().unwrap())
            .await,
        Err(TransferError::AuthorizationInvalid),
        "completed once"
    );

    // Expired challenges and proofs are refused.
    let expired_token = generate_proof_token().unwrap();
    let expired_challenge: Uuid = sqlx::query_scalar(
        "WITH c AS (
             INSERT INTO identity.verification_challenges(user_id, challenge_type, code_hash, session_id,
                 operation_type, binding_version, binding_digest, created_at, expires_at, consumed_at)
             SELECT $1, 'TRANSACTION_CONFIRMATION', 'none:explicit-confirmation', s.id, 'INTERNAL_TRANSFER', 1,
                 payments.transfer_binding($1, $2, $3, 2000000, 0, NULL),
                 now() - interval '20 minutes', now() - interval '15 minutes', now() - interval '18 minutes'
             FROM identity.sessions s WHERE s.token_hash = $4
             RETURNING id, consumed_at)
         INSERT INTO identity.transaction_proofs(user_id, challenge_id, proof_hash, created_at, expires_at,
             challenge_completed_at)
         SELECT $1, c.id, $5, c.consumed_at, c.consumed_at + interval '2 minutes', c.consumed_at FROM c
         RETURNING challenge_id",
    )
    .bind(alice)
    .bind(bob)
    .bind(world.usdt)
    .bind(&session)
    .bind(token_digest(&expired_token).unwrap())
    .fetch_one(&world.pool)
    .await
    .unwrap();
    assert_eq!(
        world
            .service
            .execute(alice, &session, "expired-proof", &expired_token, terms)
            .await,
        Err(TransferError::AuthorizationInvalid)
    );
    let stale: Uuid = sqlx::query_scalar(
        "INSERT INTO identity.verification_challenges(user_id, challenge_type, code_hash, session_id,
             operation_type, binding_version, binding_digest, created_at, expires_at)
         SELECT $1, 'TRANSACTION_CONFIRMATION', 'none:explicit-confirmation', s.id, 'INTERNAL_TRANSFER', 1,
             'digest', now() - interval '20 minutes', now() - interval '15 minutes'
         FROM identity.sessions s WHERE s.token_hash = $2 RETURNING id",
    )
    .bind(alice)
    .bind(&session)
    .fetch_one(&world.pool)
    .await
    .unwrap();
    assert_ne!(stale, expired_challenge);
    assert_eq!(
        world
            .service
            .confirm(alice, &session, stale, generate_proof_token().unwrap())
            .await,
        Err(TransferError::AuthorizationInvalid)
    );

    // A revoked session can neither authorize nor spend an earlier proof.
    let token = world.authorized(alice, &other_session, terms).await;
    world
        .admin(&format!(
            "UPDATE identity.sessions SET revoked_at = now() WHERE token_hash = '{other_session}'"
        ))
        .await;
    assert_eq!(
        world
            .service
            .execute(alice, &other_session, "revoked-session", &token, terms)
            .await,
        Err(TransferError::SessionInvalid)
    );
    assert_eq!(
        world.service.authorize(alice, &other_session, terms).await,
        Err(TransferError::SessionInvalid)
    );
    assert_eq!(
        count(
            &world.pool,
            "SELECT count(*) FROM payments.internal_transfers"
        )
        .await,
        1
    );
    assert!(world.violations().await.is_empty());
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn concurrent_transfers_cannot_overspend_or_duplicate(pool: PgPool) {
    let world = setup(pool).await;
    let alice = world.customer("alice_c", "KYC_APPROVED").await;
    let bob = world.customer("bob_c", "KYC_APPROVED").await;
    world
        .activate("CUSTOMER_TRANSFER", world.usdt, CEILING)
        .await;
    world.fund(alice, world.usdt, 10 * UNIT).await;

    // Five confirmed transfers of 4 USDT race for 10 USDT.
    let mut prepared = Vec::new();
    for _ in 0..5 {
        let session = world.session(alice).await;
        let token = world
            .authorized(alice, &session, to(bob, "USDT", "4"))
            .await;
        prepared.push((session, token));
    }
    let tasks: Vec<_> = prepared
        .into_iter()
        .enumerate()
        .map(|(n, (session, token))| {
            let service = world.service.clone();
            tokio::spawn(async move {
                service
                    .execute(
                        alice,
                        &session,
                        &format!("race-key-{n:04}"),
                        &token,
                        to(bob, "USDT", "4"),
                    )
                    .await
            })
        })
        .collect();
    let mut outcomes = Vec::new();
    for task in tasks {
        outcomes.push(task.await.unwrap());
    }
    assert_eq!(outcomes.iter().filter(|o| o.is_ok()).count(), 2);
    assert!(
        outcomes
            .iter()
            .filter(|o| o.is_err())
            .all(|o| *o == Err(TransferError::InsufficientFunds))
    );
    assert_eq!(world.available(alice, world.usdt).await, 2 * UNIT);
    assert_eq!(world.available(bob, world.usdt).await, 8 * UNIT);

    // The same request sent twice at once is executed once.
    world.fund(alice, world.usdt, 8 * UNIT).await;
    let session = world.session(alice).await;
    let token = world
        .authorized(alice, &session, to(bob, "USDT", "1"))
        .await;
    let twice: Vec<_> = (0..2)
        .map(|_| {
            let (service, session, token) = (world.service.clone(), session.clone(), token.clone());
            tokio::spawn(async move {
                service
                    .execute(
                        alice,
                        &session,
                        "same-key-0001",
                        &token,
                        to(bob, "USDT", "1"),
                    )
                    .await
            })
        })
        .collect();
    let mut results = Vec::new();
    for task in twice {
        results.push(task.await.unwrap().unwrap());
    }
    assert_eq!(results[0].transfer.id, results[1].transfer.id);
    assert_eq!(results.iter().filter(|r| r.replayed).count(), 1);
    assert_eq!(world.available(alice, world.usdt).await, 9 * UNIT);
    assert_eq!(
        count(
            &world.pool,
            "SELECT count(*) FROM payments.internal_transfers"
        )
        .await,
        3
    );
    assert!(world.violations().await.is_empty());
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn recipient_lookup_is_exact_budgeted_and_private(pool: PgPool) {
    let world = setup(pool).await;
    let alice = world.customer("alice_l", "KYC_APPROVED").await;
    world.customer("bob_l", "KYC_APPROVED").await;
    world.customer("pending_l", "KYC_PENDING").await;
    let retired = world.customer("retired_l", "KYC_APPROVED").await;
    world
        .admin(&format!(
            "UPDATE customers.usernames SET is_active = false, retired_at = now() WHERE user_id = '{retired}'"
        ))
        .await;
    let suspended = world.customer("gone_l", "KYC_APPROVED").await;
    world
        .admin(&format!(
            "UPDATE identity.users SET status = 'SUSPENDED' WHERE id = '{suspended}'"
        ))
        .await;

    assert_eq!(
        world.service.resolve(alice, "bob_l").await,
        Err(TransferError::LookupUnavailable),
        "no approved budget, no lookups"
    );
    world.lookup_budget(6).await;
    // Exercise the SQL result contract directly so function or grant failures report PostgreSQL's error.
    let retired_hit: Option<(Uuid, String, Option<String>, bool)> =
        sqlx::query_as("SELECT * FROM payments.resolve_recipient($1, $2)")
            .bind(alice)
            .bind("@Retired_L")
            .fetch_optional(&world.app_pool)
            .await
            .unwrap();
    assert_eq!(retired_hit, None);
    assert_eq!(
        world.service.resolve(alice, "a..b").await,
        Err(TransferError::InvalidRequest)
    );
    assert!(
        world.service.resolve(alice, "bob").await.unwrap().is_none(),
        "exact match only"
    );
    assert!(
        world
            .service
            .resolve(alice, "gone_l")
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        !world
            .service
            .resolve(alice, "pending_l")
            .await
            .unwrap()
            .unwrap()
            .can_receive,
        "no reason is given"
    );
    let me = world
        .service
        .resolve(alice, "ALICE_L")
        .await
        .unwrap()
        .unwrap();
    assert_eq!((me.id, me.can_receive), (alice, false));
    assert!(
        world
            .service
            .resolve(alice, "bob_l")
            .await
            .unwrap()
            .unwrap()
            .can_receive
    );
    assert_eq!(
        world.service.resolve(alice, "bob_l").await,
        Err(TransferError::LookupLimited),
        "misses count against the budget too"
    );
    assert_eq!(
        count(
            &world.pool,
            "SELECT sum(lookups) FROM payments.recipient_lookup_usage"
        )
        .await,
        6
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn history_pages_and_privileges(pool: PgPool) {
    let world = setup(pool).await;
    let alice = world.customer("alice_h", "KYC_APPROVED").await;
    let bob = world.customer("bob_h", "KYC_APPROVED").await;
    let carol = world.customer("carol_h", "KYC_APPROVED").await;
    let session = world.session(alice).await;
    world
        .activate("CUSTOMER_TRANSFER", world.usdt, CEILING)
        .await;
    world.fund(alice, world.usdt, 10 * UNIT).await;
    let mut sent = Vec::new();
    for (n, amount) in ["1", "2", "3"].into_iter().enumerate() {
        sent.push(
            world
                .send(
                    alice,
                    &session,
                    &format!("history-{n:04}"),
                    to(bob, "USDT", amount),
                )
                .await
                .unwrap()
                .transfer
                .id,
        );
    }
    let page = world.service.history(alice, None, 2).await.unwrap();
    assert_eq!(
        page.iter().map(|r| r.id).collect::<Vec<_>>(),
        [sent[2], sent[1]]
    );
    let rest = world
        .service
        .history(alice, Some(sent[1]), 2)
        .await
        .unwrap();
    assert_eq!(rest.iter().map(|r| r.id).collect::<Vec<_>>(), [sent[0]]);
    assert_eq!(
        world.service.history(carol, Some(sent[1]), 2).await,
        Err(TransferError::InvalidRequest),
        "another customer's transaction is no cursor"
    );

    // NoApprovedPolicy, the shipped gate, refuses before the database is asked.
    let shipped = TransferService::new(PgTransfers::new(world.app_pool.clone()), NoApprovedPolicy);
    let challenges = count(
        &world.pool,
        "SELECT count(*) FROM identity.verification_challenges",
    )
    .await;
    assert_eq!(
        shipped
            .authorize(alice, &session, to(bob, "USDT", "1"))
            .await,
        Err(TransferError::NotEnabled)
    );
    assert_eq!(
        count(
            &world.pool,
            "SELECT count(*) FROM identity.verification_challenges"
        )
        .await,
        challenges
    );

    // The API role reaches only the customer functions.
    let first = sent[0];
    for sql in [
        format!(
            "INSERT INTO payments.transactions(user_id, public_reference, transaction_type, status)
             VALUES ('{alice}', 'forged', 'INTERNAL_TRANSFER', 'COMPLETED')"
        ),
        format!("UPDATE payments.internal_transfers SET amount_minor = 1 WHERE transaction_id = '{first}'"),
        format!(
            "INSERT INTO identity.transaction_proofs(user_id, challenge_id, proof_hash, expires_at,
                 challenge_completed_at) VALUES ('{alice}', '{first}', 'x', now(), now())"
        ),
        "UPDATE identity.verification_challenges SET consumed_at = now()".to_owned(),
        "INSERT INTO payments.recipient_lookup_policy(window_seconds, lookups_per_window, approval_reference)
             VALUES (60, 10000, 'self-approved')"
            .to_owned(),
        format!(
            "SELECT * FROM ledger.transfer_between_customers('forged', '{alice}', '{bob}', '{}', '1', NULL,
                 'CUSTOMER', '{alice}', NULL)",
            world.usdt
        ),
        format!("SELECT * FROM payments.participant_transfers('{bob}', NULL, NULL, 10)"),
        format!("SELECT * FROM payments.require_transfer_eligibility('{alice}', '{bob}', '{}')", world.usdt),
        format!("SELECT * FROM wallets.transfer_wallets('{alice}', '{bob}', '{}')", world.usdt),
        "SELECT * FROM payments.internal_transfers".to_owned(),
    ] {
        assert_eq!(
            sqlstate(&world.pool, "deypay_app", &sql).await.as_deref(),
            Some("42501"),
            "{sql}"
        );
    }
    assert!(world.violations().await.is_empty());
}
