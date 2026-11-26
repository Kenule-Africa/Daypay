//! Financial invariant and adversarial tests for the Phase 5 ledger
//! (IMP-P5-017), run against real PostgreSQL through the production grants:
//! postings use deypay_ledger_poster, provider outcomes deypay_ledger_evidence,
//! and corrections/reconciliation deypay_ledger_controller from sessions whose
//! login is bound to one operator. Assets, customers, operators, provider
//! operations and capability activations are fixtures written as the database
//! administrator; every rule under test runs in the real ledger functions.
use deypay_application::ledger::*;
use deypay_domain::ledger::{Direction, EventIdentity, JournalDraft, PostingLine};
use deypay_domain::units::{MinorUnits, PostingAmount};
use deypay_infrastructure::MIGRATOR;
use deypay_infrastructure::ledger::{
    CorrectionOutcome, LedgerRole, PgLedger, ProviderEvidence, ProviderOutcome,
};
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};
use uuid::Uuid;

const ROLES: &str = include_str!("../../../../infrastructure/postgres/ledger-roles.psql");
const GRANTS: &str = include_str!("../../../../infrastructure/postgres/ledger-grants.psql");
/// One whole USDC or USDT at the approved six decimal places.
const UNIT: u128 = 1_000_000;
/// Fixture activation ceiling, far above any amount used here.
const CEILING: u128 = 1_000_000_000_000_000_000_000_000_000;
const CAPABILITIES: [&str; 3] = [
    "CUSTOMER_CREDIT",
    "CUSTOMER_TRANSFER",
    "CUSTOMER_RESERVATION",
];
/// Stand-ins for operator logins (roles are cluster-wide and shared by tests).
const MAKER_LOGIN: &str = "deypay_test_operator_maker";
const CHECKER_LOGIN: &str = "deypay_test_operator_checker";
const OUTSIDER_LOGIN: &str = "deypay_test_operator_outsider";
const DUAL_LOGIN: &str = "deypay_test_dual_duty";
const MIGRATION_LOGIN: &str = "deypay_test_migration_admin";
/// Records a denied operation must leave untouched.
const RECORDS: [&str; 10] = [
    "ledger.accounting_events",
    "ledger.journals",
    "ledger.entries",
    "ledger.reservations",
    "ledger.reservation_movements",
    "ledger.provider_outcomes",
    "ledger.correction_requests",
    "ledger.recovery_cases",
    "integrations.outbox_events",
    "audit.audit_events",
];
const RUNTIME_ROLES: [&str; 5] = [
    "deypay_ledger_poster",
    "deypay_ledger_evidence",
    "deypay_ledger_controller",
    "deypay_ledger_reader",
    "deypay_ledger_owner",
];

fn amount(value: u128) -> PostingAmount {
    PostingAmount::from_value(value).unwrap()
}

fn units(value: u128) -> MinorUnits {
    MinorUnits::new(value).unwrap()
}

fn line(account: Uuid, asset: Uuid, direction: Direction, value: u128) -> PostingLine {
    PostingLine {
        account_id: account,
        asset_id: asset,
        direction,
        amount: amount(value),
    }
}

struct Books {
    pool: PgPool,
    /// Workflow postings (deypay_ledger_poster).
    ledger: PgLedger,
    /// Authenticated provider adapter (deypay_ledger_evidence).
    evidence: PgLedger,
    /// The maker's authenticated session (deypay_ledger_controller).
    control: PgLedger,
    /// The checker's authenticated session (deypay_ledger_controller).
    approve: PgLedger,
    maker_session: PgPool,
    usdc: Uuid,
    usdt: Uuid,
    custody_usdc: Uuid,
    custody_usdt: Uuid,
    fee_usdt: Uuid,
    suspense_usdc: Uuid,
    maker: Uuid,
    checker: Uuid,
    outsider: Uuid,
    provider: Uuid,
}

#[derive(Clone, Copy)]
struct Customer {
    id: Uuid,
    usdc: CustomerAccounts,
    usdt: CustomerAccounts,
}

impl Customer {
    fn accounts(&self, books: &Books, asset: Uuid) -> CustomerAccounts {
        if asset == books.usdc {
            self.usdc
        } else {
            self.usdt
        }
    }
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

/// Connections whose session user is `login`, exactly as if that login had
/// authenticated. Only the test administrator can open such sessions.
fn session_as(pool: &PgPool, login: &'static str) -> PgPool {
    PgPoolOptions::new()
        .max_connections(2)
        .after_connect(move |conn, _| {
            Box::pin(async move {
                // The setting behind SET SESSION AUTHORIZATION, with the login bound as a value.
                sqlx::query("SELECT set_config('session_authorization', $1, false)")
                    .bind(login)
                    .execute(conn)
                    .await?;
                Ok(())
            })
        })
        .connect_lazy_with((*pool.connect_options()).clone())
}

async fn user(pool: &PgPool, email: &str, status: &str) -> Uuid {
    // Statuses beyond email confirmation carry the verification time, as real rows do.
    sqlx::query_scalar(
        "INSERT INTO identity.users(email, status, email_verified_at)
         VALUES ($1, $2, CASE WHEN $2 IN ('REGISTERED', 'CONTACT_UNVERIFIED') THEN NULL ELSE now() END)
         RETURNING id",
    )
    .bind(email)
    .bind(status)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// The database administrator records an approved activation.
async fn activate(pool: &PgPool, capability: &str, asset: Uuid, ceiling: u128) {
    sqlx::query(
        "INSERT INTO ledger.capability_activations(capability, asset_id, approval_reference, max_operation_minor)
         VALUES ($1, $2, 'TEST-FIXTURE', $3::numeric)",
    )
    .bind(capability)
    .bind(asset)
    .bind(ceiling.to_string())
    .execute(pool)
    .await
    .unwrap();
}

async fn deactivate(pool: &PgPool, capability: &str, asset: Uuid) {
    let changed = sqlx::query(
        "UPDATE ledger.capability_activations
         SET deactivated_at = now(), deactivated_by = session_user, deactivation_reason = 'Test deactivation'
         WHERE capability = $1 AND asset_id = $2 AND deactivated_at IS NULL",
    )
    .bind(capability)
    .bind(asset)
    .execute(pool)
    .await
    .unwrap();
    assert_eq!(changed.rows_affected(), 1);
}

async fn setup(pool: PgPool) -> Books {
    sqlx::raw_sql(ROLES).execute(&pool).await.unwrap();
    for login in [MAKER_LOGIN, CHECKER_LOGIN, OUTSIDER_LOGIN] {
        test_role(&pool, login, "deypay_ledger_controller").await;
    }
    sqlx::raw_sql(GRANTS).execute(&pool).await.unwrap();
    let asset = |symbol: &'static str, precision: i16| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, Uuid>(
                "INSERT INTO wallets.assets(symbol, name, asset_type, accounting_precision, enabled)
                 VALUES ($1, $1 || ' fixture', 'TEST', $2, true) RETURNING id",
            )
            .bind(symbol)
            .bind(precision)
            .fetch_one(&pool)
            .await
            .unwrap()
        }
    };
    let usdc = asset("USDC", 6).await;
    let usdt = asset("USDT", 6).await;
    asset("NGN", 2).await;
    let role: Uuid = sqlx::query_scalar(
        "INSERT INTO audit.admin_roles(role_code, description) VALUES ('LEDGER_OPERATIONS', 'fixture') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO audit.role_permissions(role_id, permission_id)
         SELECT $1, id FROM audit.admin_permissions
         WHERE permission_code LIKE 'LEDGER_%' OR permission_code = 'RECONCILIATION_OPERATE'",
    )
    .bind(role)
    .execute(&pool)
    .await
    .unwrap();
    let maker = user(&pool, "maker@ops.test", "CONTACT_VERIFIED").await;
    let checker = user(&pool, "checker@ops.test", "CONTACT_VERIFIED").await;
    let outsider = user(&pool, "outsider@ops.test", "CONTACT_VERIFIED").await;
    sqlx::query("INSERT INTO audit.admin_assignments(user_id, role_id) VALUES ($1, $3), ($2, $3)")
        .bind(maker)
        .bind(checker)
        .bind(role)
        .execute(&pool)
        .await
        .unwrap();
    for (login, operator) in [
        (MAKER_LOGIN, maker),
        (CHECKER_LOGIN, checker),
        (OUTSIDER_LOGIN, outsider),
    ] {
        sqlx::query(
            "INSERT INTO ledger.operator_logins(login_name, user_id, approval_reference)
             VALUES ($1, $2, 'TEST-FIXTURE')",
        )
        .bind(login)
        .bind(operator)
        .execute(&pool)
        .await
        .unwrap();
    }
    let maker_session = session_as(&pool, MAKER_LOGIN);
    let control = PgLedger::acting_as(maker_session.clone(), LedgerRole::Controller);
    let open = |code: &'static str, asset: Uuid| {
        let control = control.clone();
        async move {
            control
                .open_platform_account(code, asset, "primary", maker)
                .await
                .unwrap()
        }
    };
    let custody_usdc = open("1100", usdc).await;
    let custody_usdt = open("1110", usdt).await;
    let fee_usdt = open("4110", usdt).await;
    let suspense_usdc = open("6150", usdc).await;
    for asset in [usdc, usdt] {
        for capability in CAPABILITIES {
            activate(&pool, capability, asset, CEILING).await;
        }
    }
    let provider: Uuid = sqlx::query_scalar(
        "INSERT INTO integrations.providers(provider_code, provider_name, provider_type)
         VALUES ('TEST_PAYOUT', 'Payout provider fixture', 'TEST') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    Books {
        ledger: PgLedger::acting_as(pool.clone(), LedgerRole::Poster),
        evidence: PgLedger::acting_as(pool.clone(), LedgerRole::Evidence),
        approve: PgLedger::acting_as(session_as(&pool, CHECKER_LOGIN), LedgerRole::Controller),
        control,
        maker_session,
        pool,
        usdc,
        usdt,
        custody_usdc,
        custody_usdt,
        fee_usdt,
        suspense_usdc,
        maker,
        checker,
        outsider,
        provider,
    }
}

impl Books {
    async fn customer(&self, name: &str) -> Customer {
        self.customer_with(name, "KYC_APPROVED").await
    }

    async fn customer_with(&self, name: &str, status: &str) -> Customer {
        let id = user(&self.pool, &format!("{name}@customer.test"), status).await;
        let mut accounts = Vec::new();
        for (asset, symbol) in [(self.usdc, "USDC"), (self.usdt, "USDT")] {
            let wallet: Uuid = sqlx::query_scalar(
                "INSERT INTO wallets.wallet_profiles(user_id, asset_id, asset_symbol, status, provisioning_status)
                 VALUES ($1, $2, $3, 'ACTIVE', 'COMPLETE') RETURNING id",
            )
            .bind(id)
            .bind(asset)
            .bind(symbol)
            .fetch_one(&self.pool)
            .await
            .unwrap();
            accounts.push(self.ledger.open_customer_accounts(wallet).await.unwrap());
        }
        Customer {
            id,
            usdc: accounts[0],
            usdt: accounts[1],
        }
    }

    fn custody(&self, asset: Uuid) -> Uuid {
        if asset == self.usdc {
            self.custody_usdc
        } else {
            self.custody_usdt
        }
    }

    fn operator(&self, login: &'static str) -> PgLedger {
        PgLedger::acting_as(session_as(&self.pool, login), LedgerRole::Controller)
    }

    async fn deposit(
        &self,
        key: &str,
        customer: &Customer,
        asset: Uuid,
        value: u128,
    ) -> Result<PostingReceipt, LedgerError> {
        self.ledger
            .post(&PostingCommand {
                identity: EventIdentity::new("DEPOSIT_CONFIRMED", key, "RECOGNITION").unwrap(),
                source_transaction: None,
                journal: JournalDraft::new(vec![
                    line(self.custody(asset), asset, Direction::Debit, value),
                    line(
                        customer.accounts(self, asset).available,
                        asset,
                        Direction::Credit,
                        value,
                    ),
                ])
                .unwrap(),
                description: Some("Verified deposit".into()),
                actor: Actor::System,
                correlation_id: None,
            })
            .await
    }

    async fn balance(&self, customer: &Customer, asset: Uuid) -> (u128, u128) {
        let balance = self
            .ledger
            .customer_balance(customer.id, asset)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            balance.total.value(),
            balance.available.value() + balance.reserved.value(),
            "total entitlement is available plus reserved"
        );
        (balance.available.value(), balance.reserved.value())
    }

    async fn account(&self, account: Uuid) -> i128 {
        self.ledger
            .account_balance(account)
            .await
            .unwrap()
            .unwrap()
            .balance_minor
    }

    async fn violations(&self) -> Vec<String> {
        sqlx::query_scalar("SELECT kind FROM ledger.integrity_violations()")
            .fetch_all(&self.pool)
            .await
            .unwrap()
    }

    async fn count(&self, table: &str) -> i64 {
        sqlx::query_scalar(&format!("SELECT count(*) FROM {table}"))
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    async fn records(&self) -> Vec<i64> {
        let mut counts = Vec::new();
        for table in RECORDS {
            counts.push(self.count(table).await);
        }
        counts
    }

    fn transfer(
        &self,
        key: &str,
        from: &Customer,
        to: &Customer,
        asset: Uuid,
        value: u128,
    ) -> TransferCommand {
        TransferCommand {
            operation_key: key.into(),
            sender: from.id,
            recipient: to.id,
            asset_id: asset,
            amount: amount(value),
            source_transaction: None,
            actor: Actor::Customer(from.id),
            correlation_id: None,
        }
    }

    fn reserve(&self, key: &str, customer: &Customer, asset: Uuid, value: u128) -> ReserveCommand {
        ReserveCommand {
            operation_key: key.into(),
            customer: customer.id,
            asset_id: asset,
            amount: amount(value),
            transaction: None,
            event_type: "WITHDRAWAL_RESERVED".into(),
            actor: Actor::Customer(customer.id),
            correlation_id: None,
        }
    }

    /// A withdrawal transaction and the provider operation submitted for it.
    async fn withdrawal(&self, customer: &Customer, asset: Uuid, value: u128) -> (Uuid, Uuid) {
        let transaction: Uuid = sqlx::query_scalar(
            "INSERT INTO payments.transactions(public_reference, user_id, transaction_type, status,
                 source_asset_id, source_amount_minor)
             VALUES ('TX-' || gen_random_uuid(), $1, 'WITHDRAWAL', 'RESERVED', $2, $3::numeric) RETURNING id",
        )
        .bind(customer.id)
        .bind(asset)
        .bind(value.to_string())
        .fetch_one(&self.pool)
        .await
        .unwrap();
        let operation: Uuid = sqlx::query_scalar(
            "INSERT INTO integrations.provider_operations(provider_id, provider_account_scope, transaction_id,
                 is_financial, operation_type, leg, authorized_attempt, idempotency_reference, status)
             VALUES ($1, 'fixture-account', $2, true, 'WITHDRAWAL', 'PAYOUT', 1, 'payout:' || $2::text, 'SUBMITTED')
             RETURNING id",
        )
        .bind(self.provider)
        .bind(transaction)
        .fetch_one(&self.pool)
        .await
        .unwrap();
        (transaction, operation)
    }

    /// Reserves funds for a withdrawal with a registered provider operation.
    /// Returns the reservation, its transaction and the provider operation.
    async fn hold(
        &self,
        key: &str,
        customer: &Customer,
        asset: Uuid,
        value: u128,
    ) -> Result<(ReservationReceipt, Uuid, Uuid), LedgerError> {
        let (transaction, operation) = self.withdrawal(customer, asset, value).await;
        let mut command = self.reserve(key, customer, asset, value);
        command.transaction = Some(transaction);
        let receipt = self.ledger.reserve(&command).await?;
        Ok((receipt, transaction, operation))
    }

    fn provider_evidence(
        &self,
        reservation: Uuid,
        operation: Uuid,
        outcome: ProviderOutcome,
    ) -> ProviderEvidence {
        ProviderEvidence {
            reservation,
            outcome,
            provider_operation: operation,
            provider_reference: format!("provider:{operation}"),
            sha256: [9; 32],
            observed_at: "2026-11-26T12:00:00Z".into(),
        }
    }

    /// The authenticated provider adapter records a definitive outcome.
    async fn outcome(
        &self,
        reservation: Uuid,
        operation: Uuid,
        outcome: ProviderOutcome,
    ) -> Result<(Uuid, bool), LedgerError> {
        self.evidence
            .record_provider_outcome(&self.provider_evidence(reservation, operation, outcome))
            .await
    }

    fn release(
        &self,
        reservation: Uuid,
        key: &str,
        value: u128,
        evidence: ReleaseEvidence,
    ) -> ReleaseCommand {
        ReleaseCommand {
            reservation,
            economic_key: key.into(),
            purpose: "RELEASE".into(),
            amount: amount(value),
            evidence,
            evidence_reference: format!("evidence:{key}"),
            actor: Actor::System,
            correlation_id: None,
        }
    }

    fn consume(
        &self,
        reservation: Uuid,
        key: &str,
        credits: Vec<(Uuid, PostingAmount)>,
    ) -> ConsumeCommand {
        ConsumeCommand {
            reservation,
            identity: EventIdentity::new("WITHDRAWAL_SETTLED", key, "SETTLEMENT").unwrap(),
            credits,
            evidence_reference: format!("chain:{key}"),
            actor: Actor::System,
            correlation_id: None,
        }
    }
}

/// Runs SQL as `role` (or as the session's own user) and returns the SQLSTATE
/// of any error, including one raised by a deferred check at commit.
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

#[sqlx::test(migrator = "MIGRATOR")]
async fn deposits_post_balanced_idempotent_journals(pool: PgPool) {
    let books = setup(pool).await;
    let alice = books.customer("alice").await;
    assert_eq!(
        books.balance(&alice, books.usdc).await,
        (0, 0),
        "new accounts start empty"
    );

    let first = books
        .deposit("chain:solana:tx1:0", &alice, books.usdc, 100 * UNIT)
        .await
        .unwrap();
    assert!(!first.replayed);
    assert!(first.posted_at.ends_with('Z'));
    assert_eq!(books.balance(&alice, books.usdc).await, (100 * UNIT, 0));
    assert_eq!(
        books.account(books.custody_usdc).await,
        (100 * UNIT) as i128
    );

    // Repeated delivery of the same verified fact replays without posting again.
    let replay = books
        .deposit("chain:solana:tx1:0", &alice, books.usdc, 100 * UNIT)
        .await
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.journal_id, first.journal_id);
    assert_eq!(books.balance(&alice, books.usdc).await, (100 * UNIT, 0));
    // The same identity with a different payload is a conflict, not a second credit.
    assert_eq!(
        books
            .deposit("chain:solana:tx1:0", &alice, books.usdc, 99 * UNIT)
            .await,
        Err(LedgerError::DuplicateAccountingEvent)
    );
    assert_eq!(books.count("ledger.journals").await, 1);

    let journal = sqlx::query(
        "SELECT j.journal_reference, j.entry_count, e.economic_event_key, e.posting_purpose
         FROM ledger.journals j JOIN ledger.accounting_events e ON e.id = j.accounting_event_id WHERE j.id = $1",
    )
    .bind(first.journal_id)
    .fetch_one(&books.pool)
    .await
    .unwrap();
    assert!(
        journal
            .get::<String, _>("journal_reference")
            .starts_with("JNL-")
    );
    assert_eq!(journal.get::<i32, _>("entry_count"), 2);
    assert_eq!(journal.get::<String, _>("posting_purpose"), "RECOGNITION");
    let outbox: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM integrations.outbox_events WHERE aggregate_id = $1 AND event_type = 'LEDGER_JOURNAL_POSTED'",
    )
    .bind(first.journal_id)
    .fetch_one(&books.pool)
    .await
    .unwrap();
    assert_eq!(outbox, 1);
    assert!(books.violations().await.is_empty());
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn invalid_journals_never_post(pool: PgPool) {
    let books = setup(pool).await;
    let alice = books.customer("alice").await;
    books
        .deposit("seed", &alice, books.usdc, 10 * UNIT)
        .await
        .unwrap();
    let before = books.count("ledger.journals").await;
    let call = |accounts: String,
                assets: String,
                directions: &str,
                amounts: &str,
                purpose: &str| {
        format!(
            "SELECT * FROM ledger.post_journal('DEPOSIT_CONFIRMED', 'bad', '{purpose}', NULL,
             ARRAY[{accounts}]::uuid[], ARRAY[{assets}]::uuid[], ARRAY[{directions}], ARRAY[{amounts}],
             NULL, 'SYSTEM', NULL, NULL)"
        )
    };
    let custody = books.custody_usdc;
    let available = alice.usdc.available;
    let pair = format!("'{custody}','{available}'");
    let usdc2 = format!("'{}','{}'", books.usdc, books.usdc);
    let poster = Some("deypay_ledger_poster");
    for (sql, expected) in [
        // Debits must equal credits.
        (
            call(
                pair.clone(),
                usdc2.clone(),
                "'DEBIT','CREDIT'",
                "'5','4'",
                "P",
            ),
            "LE001",
        ),
        // A USDC debit cannot be balanced by a USDT credit.
        (
            call(
                format!("'{custody}','{}'", alice.usdt.available),
                format!("'{}','{}'", books.usdc, books.usdt),
                "'DEBIT','CREDIT'",
                "'5','5'",
                "P",
            ),
            "LE001",
        ),
        (
            call(
                format!("'{custody}'"),
                format!("'{}'", books.usdc),
                "'DEBIT'",
                "'5'",
                "P",
            ),
            "LE006",
        ),
        (
            call(
                pair.clone(),
                usdc2.clone(),
                "'DEBIT','CREDIT'",
                "'0','0'",
                "P",
            ),
            "LE006",
        ),
        (
            call(
                pair.clone(),
                usdc2.clone(),
                "'DEBIT','CREDIT'",
                "'1.5','1.5'",
                "P",
            ),
            "LE006",
        ),
        (
            call(
                pair.clone(),
                usdc2.clone(),
                "'DEBIT','CREDIT'",
                "'-1','-1'",
                "P",
            ),
            "LE006",
        ),
        (
            call(
                pair.clone(),
                usdc2.clone(),
                "'DEBIT','CREDIT'",
                "'01','01'",
                "P",
            ),
            "LE006",
        ),
        (
            call(
                pair.clone(),
                usdc2.clone(),
                "'DEBIT','CREDIT'",
                "'1e3','1e3'",
                "P",
            ),
            "LE006",
        ),
        (
            call(
                pair.clone(),
                usdc2.clone(),
                "'DEBIT','CREDIT'",
                &format!("'{0}','{0}'", "9".repeat(39)),
                "P",
            ),
            "LE006",
        ),
        // Line assets must match their accounts.
        (
            call(
                format!("'{}','{}'", books.custody_usdt, alice.usdt.available),
                usdc2.clone(),
                "'DEBIT','CREDIT'",
                "'5','5'",
                "P",
            ),
            "LE011",
        ),
        (
            call(
                format!("'{custody}','{}'", Uuid::from_u128(7)),
                usdc2.clone(),
                "'DEBIT','CREDIT'",
                "'5','5'",
                "P",
            ),
            "LE008",
        ),
        // Workflow postings cannot take money from customers or touch reservations.
        (
            call(
                pair.clone(),
                usdc2.clone(),
                "'CREDIT','DEBIT'",
                "'5','5'",
                "P",
            ),
            "LE012",
        ),
        (
            call(
                format!("'{custody}','{}'", alice.usdc.reserved),
                usdc2.clone(),
                "'DEBIT','CREDIT'",
                "'5','5'",
                "P",
            ),
            "LE012",
        ),
        (
            call(
                pair.clone(),
                usdc2.clone(),
                "'DEBIT','CREDIT'",
                "'5','5'",
                "RESERVE",
            ),
            "LE012",
        ),
        (
            call(pair, usdc2, "'DEBIT','CREDIT'", "'5','5'", "p"),
            "LE006",
        ),
    ] {
        assert_eq!(
            sqlstate(&books.pool, poster, &sql).await.as_deref(),
            Some(expected),
            "{sql}"
        );
    }
    assert_eq!(books.count("ledger.journals").await, before);
    assert_eq!(books.balance(&alice, books.usdc).await, (10 * UNIT, 0));
    assert!(books.violations().await.is_empty());
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn posted_history_is_immutable_and_unbalanced_journals_cannot_commit(pool: PgPool) {
    let books = setup(pool).await;
    let alice = books.customer("alice").await;
    let posted = books
        .deposit("chain:tx", &alice, books.usdc, 5 * UNIT)
        .await
        .unwrap();
    let admin = None;
    for sql in [
        format!(
            "UPDATE ledger.entries SET amount_minor = 1 WHERE journal_id = '{}'",
            posted.journal_id
        ),
        format!(
            "DELETE FROM ledger.entries WHERE journal_id = '{}'",
            posted.journal_id
        ),
        format!(
            "DELETE FROM ledger.journals WHERE id = '{}'",
            posted.journal_id
        ),
        "UPDATE ledger.accounting_events SET posting_purpose = 'OTHER'".to_owned(),
        "TRUNCATE ledger.entries CASCADE".to_owned(),
        format!(
            "UPDATE ledger.accounts SET asset_id = '{}' WHERE id = '{}'",
            books.usdt, alice.usdc.available
        ),
        format!(
            "DELETE FROM ledger.accounts WHERE id = '{}'",
            alice.usdc.available
        ),
        "DELETE FROM ledger.balances".to_owned(),
        "UPDATE ledger.chart_accounts SET normal_balance = 'DEBIT' WHERE code = '2100'".to_owned(),
    ] {
        assert_eq!(
            sqlstate(&books.pool, admin, &sql).await.as_deref(),
            Some("55000"),
            "{sql}"
        );
    }
    // Even the table owner cannot commit a journal whose entries do not balance.
    let unbalanced = format!(
        "WITH e AS (INSERT INTO ledger.accounting_events(event_type, economic_event_key, posting_purpose, request_hash, actor_type)
                    VALUES ('MANUAL_TEST', 'direct', 'P', sha256('x'), 'SYSTEM') RETURNING id),
              j AS (INSERT INTO ledger.journals(posting_sequence, journal_reference, accounting_event_id, event_type, entry_count)
                    SELECT 999999, 'JNL-000000999999', id, 'MANUAL_TEST', 2 FROM e RETURNING id)
         INSERT INTO ledger.entries(journal_id, line_number, account_id, asset_id, direction, amount_minor)
         SELECT j.id, v.n, v.account, '{usdc}', v.dir, v.amount FROM j,
           (VALUES (1, '{custody}'::uuid, 'DEBIT', 5), (2, '{available}'::uuid, 'CREDIT', 4)) AS v(n, account, dir, amount)",
        usdc = books.usdc,
        custody = books.custody_usdc,
        available = alice.usdc.available,
    );
    assert_eq!(
        sqlstate(&books.pool, admin, &unbalanced).await.as_deref(),
        Some("LE001")
    );
    // A journal missing its declared entries cannot commit either.
    let missing = "WITH e AS (INSERT INTO ledger.accounting_events(event_type, economic_event_key, posting_purpose, request_hash, actor_type)
                    VALUES ('MANUAL_TEST', 'direct2', 'P', sha256('y'), 'SYSTEM') RETURNING id)
         INSERT INTO ledger.journals(posting_sequence, journal_reference, accounting_event_id, event_type, entry_count)
         SELECT 999998, 'JNL-000000999998', id, 'MANUAL_TEST', 2 FROM e";
    assert_eq!(
        sqlstate(&books.pool, admin, missing).await.as_deref(),
        Some("LE001")
    );
    // The projection refuses an overdrawn customer liability.
    let overdraw = format!(
        "UPDATE ledger.balances SET debit_total = debit_total + 6000000, balance_minor = balance_minor - 6000000
         WHERE account_id = '{}'",
        alice.usdc.available
    );
    assert_eq!(
        sqlstate(&books.pool, admin, &overdraw).await.as_deref(),
        Some("23514")
    );
    assert_eq!(books.count("ledger.journals").await, 1);
    assert!(books.violations().await.is_empty());
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn runtime_roles_cannot_bypass_the_posting_interface(pool: PgPool) {
    let books = setup(pool).await;
    let alice = books.customer("alice").await;
    books
        .deposit("seed", &alice, books.usdc, UNIT)
        .await
        .unwrap();
    let post = format!(
        "SELECT * FROM ledger.post_journal('DEPOSIT_CONFIRMED', 'k2', 'P', NULL, ARRAY['{}','{}']::uuid[],
         ARRAY['{u}','{u}']::uuid[], ARRAY['DEBIT','CREDIT'], ARRAY['1','1'], NULL, 'SYSTEM', NULL, NULL)",
        books.custody_usdc,
        alice.usdc.available,
        u = books.usdc
    );
    let insert = "INSERT INTO ledger.entries(journal_id, line_number, account_id, asset_id, direction, amount_minor)
         SELECT journal_id, 9, account_id, asset_id, direction, amount_minor FROM ledger.entries LIMIT 1"
        .to_owned();
    let raise = format!(
        "UPDATE ledger.balances SET credit_total = credit_total + 1, balance_minor = balance_minor + 1 WHERE account_id = '{}'",
        alice.usdc.available
    );
    let internal = format!(
        "SELECT * FROM ledger.apply_journal('CORRECTION', 'X_EVENT', 'k3', 'P', sha256('z'), NULL, NULL,
         ARRAY['{}','{}']::uuid[], ARRAY['{u}','{u}']::uuid[], ARRAY['DEBIT','CREDIT'], ARRAY[1,1]::numeric[],
         NULL, 'SYSTEM', NULL, NULL, false)",
        books.custody_usdc,
        alice.usdc.available,
        u = books.usdc
    );
    let correction = format!(
        "SELECT ledger.request_correction('REVERSAL', (SELECT id FROM ledger.journals LIMIT 1), NULL, 'r', 'e', '{}')",
        books.maker
    );
    let outcome = "SELECT * FROM ledger.record_provider_outcome(gen_random_uuid(), 'NOT_EXECUTED',
         gen_random_uuid(), 'forged', sha256('forged'), now())"
        .to_owned();
    let capability = format!(
        "SELECT ledger.require_capability('CUSTOMER_TRANSFER', '{}', 1)",
        books.usdc
    );
    for (role, sql) in [
        (
            "deypay_app",
            "SELECT count(*) FROM ledger.entries".to_owned(),
        ),
        ("deypay_app", post.clone()),
        ("deypay_app", outcome.clone()),
        (
            "deypay_app",
            "SELECT * FROM ledger.customer_balance(gen_random_uuid(), gen_random_uuid())"
                .to_owned(),
        ),
        ("deypay_ledger_poster", insert.clone()),
        ("deypay_ledger_poster", raise.clone()),
        ("deypay_ledger_poster", internal.clone()),
        ("deypay_ledger_poster", correction),
        ("deypay_ledger_poster", outcome.clone()),
        ("deypay_ledger_poster", capability),
        ("deypay_ledger_evidence", post.clone()),
        ("deypay_ledger_evidence", insert.clone()),
        ("deypay_ledger_evidence", internal.clone()),
        (
            "deypay_ledger_evidence",
            "SELECT count(*) FROM ledger.entries".to_owned(),
        ),
        ("deypay_ledger_controller", post.clone()),
        ("deypay_ledger_controller", insert.clone()),
        ("deypay_ledger_controller", internal),
        ("deypay_ledger_controller", outcome.clone()),
        (
            "deypay_ledger_controller",
            "SELECT ledger.session_operator()".to_owned(),
        ),
        ("deypay_ledger_reader", post),
        ("deypay_ledger_reader", raise),
        ("deypay_ledger_reader", outcome),
    ] {
        assert_eq!(
            sqlstate(&books.pool, Some(role), &sql).await.as_deref(),
            Some("42501"),
            "{role}: {sql}"
        );
    }
    // No runtime role, and not even the ledger functions' owner, can write the
    // activations or operator bindings that the ledger trusts.
    for role in RUNTIME_ROLES {
        for sql in [
            format!(
                "INSERT INTO ledger.capability_activations(capability, asset_id, approval_reference, max_operation_minor)
                 VALUES ('CUSTOMER_TRANSFER', '{}', 'self-approved', 1)",
                books.usdc
            ),
            format!(
                "INSERT INTO ledger.operator_logins(login_name, user_id, approval_reference)
                 VALUES ('{MAKER_LOGIN}', '{}', 'self-bound')",
                books.checker
            ),
            "UPDATE ledger.capability_activations SET max_operation_minor = 1".to_owned(),
        ] {
            assert_eq!(
                sqlstate(&books.pool, Some(role), &sql).await.as_deref(),
                Some("42501"),
                "{role}: {sql}"
            );
        }
    }
    assert_eq!(
        sqlstate(
            &books.pool,
            Some("deypay_ledger_reader"),
            "SELECT count(*) FROM ledger.entries"
        )
        .await,
        None
    );
    assert_eq!(books.count("ledger.journals").await, 1);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn customer_accounts_are_empty_stablecoin_liabilities(pool: PgPool) {
    let books = setup(pool).await;
    let alice = books.customer("alice").await;
    let wallet: Uuid =
        sqlx::query_scalar("SELECT wallet_profile_id FROM ledger.accounts WHERE id = $1")
            .bind(alice.usdc.available)
            .fetch_one(&books.pool)
            .await
            .unwrap();
    // Opening again returns the same accounts and creates no value.
    assert_eq!(
        books.ledger.open_customer_accounts(wallet).await.unwrap(),
        alice.usdc
    );
    assert_eq!(books.count("ledger.journals").await, 0);
    let roles: Vec<(String, String, String, bool)> = sqlx::query_as(
        "SELECT chart_code, account_type, normal_balance, negative_allowed FROM ledger.accounts
         WHERE owner_id = $1 ORDER BY chart_code",
    )
    .bind(alice.id)
    .fetch_all(&books.pool)
    .await
    .unwrap();
    assert_eq!(
        roles,
        ["2100", "2110", "2120", "2130"]
            .iter()
            .map(|code| (
                code.to_string(),
                "LIABILITY".to_owned(),
                "CREDIT".to_owned(),
                false
            ))
            .collect::<Vec<_>>()
    );
    // There is no customer NGN wallet, and customer charts cannot open as platform accounts.
    let ngn: Uuid = sqlx::query_scalar("SELECT id FROM wallets.assets WHERE symbol = 'NGN'")
        .fetch_one(&books.pool)
        .await
        .unwrap();
    assert!(
        sqlx::query("INSERT INTO wallets.wallet_profiles(user_id, asset_id, asset_symbol, status, provisioning_status)
                     VALUES ($1, $2, 'NGN', 'ACTIVE', 'COMPLETE')")
            .bind(alice.id)
            .bind(ngn)
            .execute(&books.pool)
            .await
            .is_err()
    );
    assert_eq!(
        books
            .control
            .open_platform_account("2100", books.usdc, "primary", books.maker)
            .await,
        Err(LedgerError::NotFound)
    );
    assert_eq!(
        books
            .control
            .open_platform_account("1100", books.usdt, "primary", books.maker)
            .await,
        Err(LedgerError::InvalidRequest),
        "custody USDC cannot carry USDT"
    );
    assert_eq!(
        books
            .operator(OUTSIDER_LOGIN)
            .open_platform_account("1130", ngn, "primary", books.outsider)
            .await,
        Err(LedgerError::ApprovalRequired),
        "an authenticated operator without the permission"
    );
    assert_eq!(
        books
            .control
            .open_platform_account("1130", ngn, "primary", books.outsider)
            .await,
        Err(LedgerError::OperatorNotAuthenticated),
        "a stated operator is only a claim"
    );
    // The approved precision is enforced by the asset registry.
    assert!(
        sqlx::query("INSERT INTO wallets.assets(symbol, name, asset_type, accounting_precision) VALUES ('USDC2', 'x', 'TEST', 19)")
            .execute(&books.pool)
            .await
            .is_err()
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn spending_requires_financial_eligibility(pool: PgPool) {
    let books = setup(pool).await;
    let bob = books.customer("bob").await;
    for (name, status) in [
        ("pending", "CONTACT_VERIFIED"),
        ("kyc", "KYC_PENDING"),
        ("suspended", "SUSPENDED"),
    ] {
        let customer = books.customer_with(name, status).await;
        books
            .deposit(&format!("dep:{name}"), &customer, books.usdc, 5 * UNIT)
            .await
            .unwrap();
        assert_eq!(
            books
                .ledger
                .reserve(&books.reserve(&format!("res:{name}"), &customer, books.usdc, UNIT))
                .await,
            Err(LedgerError::NotEligible),
            "{status}"
        );
        assert_eq!(
            books
                .ledger
                .transfer(&books.transfer(&format!("tr:{name}"), &customer, &bob, books.usdc, UNIT))
                .await,
            Err(LedgerError::NotEligible)
        );
    }
    let restricted = books.customer("restricted").await;
    books
        .deposit("dep:restricted", &restricted, books.usdc, 5 * UNIT)
        .await
        .unwrap();
    sqlx::query("INSERT INTO compliance.account_restrictions(user_id, restriction_type, reason_code, active) VALUES ($1, 'FINANCIAL', 'TEST', true)")
        .bind(restricted.id)
        .execute(&books.pool)
        .await
        .unwrap();
    assert_eq!(
        books
            .ledger
            .reserve(&books.reserve("res:restricted", &restricted, books.usdc, UNIT))
            .await,
        Err(LedgerError::NotEligible)
    );
    sqlx::query("UPDATE wallets.wallet_profiles SET status = 'SUSPENDED' WHERE user_id = $1")
        .bind(bob.id)
        .execute(&books.pool)
        .await
        .unwrap();
    books
        .deposit("dep:bob", &bob, books.usdc, 5 * UNIT)
        .await
        .unwrap();
    assert_eq!(
        books
            .ledger
            .reserve(&books.reserve("res:bob", &bob, books.usdc, UNIT))
            .await,
        Err(LedgerError::NotEligible)
    );
    // The application gate shipped today refuses everything before the database is reached.
    let service = LedgerService::new(books.ledger.clone(), NoApprovedPolicy);
    let alice = books.customer("alice").await;
    books
        .deposit("dep:alice", &alice, books.usdc, 5 * UNIT)
        .await
        .unwrap();
    assert_eq!(
        service
            .reserve(&books.reserve("res:alice", &alice, books.usdc, UNIT))
            .await,
        Err(LedgerError::NotEligible)
    );
    assert_eq!(books.balance(&alice, books.usdc).await, (5 * UNIT, 0));
}

/// Finding 1: customer spending and customer crediting stay refused inside
/// PostgreSQL until the database administrator records an approved activation,
/// even for a process that calls the functions directly with the poster role.
#[sqlx::test(migrator = "MIGRATOR")]
async fn customer_movements_are_fail_closed_until_activated(pool: PgPool) {
    let books = setup(pool).await;
    let alice = books.customer("alice").await;
    let bob = books.customer("bob").await;
    books
        .deposit("dep:alice", &alice, books.usdc, 50 * UNIT)
        .await
        .unwrap();
    deactivate(&books.pool, "CUSTOMER_TRANSFER", books.usdc).await;
    deactivate(&books.pool, "CUSTOMER_RESERVATION", books.usdc).await;
    deactivate(&books.pool, "CUSTOMER_CREDIT", books.usdt).await;
    let before = books.records().await;

    // There is no application gate to skip: direct calls are refused too.
    let poster = Some("deypay_ledger_poster");
    let direct = [
        format!(
            "SELECT * FROM ledger.transfer_between_customers('direct:1', '{a}', '{b}', '{u}', '1000000',
             NULL, 'SERVICE', NULL, NULL)",
            a = alice.id,
            b = bob.id,
            u = books.usdc
        ),
        format!(
            "SELECT * FROM ledger.reserve_funds('direct:2', '{a}', '{u}', '1000000', NULL,
             'WITHDRAWAL_RESERVED', 'SERVICE', NULL, NULL)",
            a = alice.id,
            u = books.usdc
        ),
        // A system posting that would create customer entitlement out of thin custody.
        format!(
            "SELECT * FROM ledger.post_journal('DEPOSIT_CONFIRMED', 'direct:3', 'RECOGNITION', NULL,
             ARRAY['{c}','{a}']::uuid[], ARRAY['{u}','{u}']::uuid[], ARRAY['DEBIT','CREDIT'], ARRAY['5','5'],
             NULL, 'SYSTEM', NULL, NULL)",
            c = books.custody_usdt,
            a = alice.usdt.available,
            u = books.usdt
        ),
    ];
    for sql in &direct {
        assert_eq!(
            sqlstate(&books.pool, poster, sql).await.as_deref(),
            Some("LE014"),
            "{sql}"
        );
    }
    assert_eq!(
        books
            .ledger
            .transfer(&books.transfer("tr:1", &alice, &bob, books.usdc, UNIT))
            .await,
        Err(LedgerError::NotActivated)
    );
    assert_eq!(
        books
            .ledger
            .reserve(&books.reserve("res:1", &alice, books.usdc, UNIT))
            .await,
        Err(LedgerError::NotActivated)
    );
    assert_eq!(
        books.deposit("dep:usdt", &alice, books.usdt, UNIT).await,
        Err(LedgerError::NotActivated)
    );
    // Administrator attribution is reserved for approved corrections, and a
    // customer actor can move only their own funds.
    let attributed = format!(
        "SELECT * FROM ledger.post_journal('FEE_RECOGNIZED', 'direct:4', 'RECOGNITION', NULL,
         ARRAY['{c}','{f}']::uuid[], ARRAY['{u}','{u}']::uuid[], ARRAY['DEBIT','CREDIT'], ARRAY['5','5'],
         NULL, 'ADMIN', '{m}', NULL)",
        c = books.custody_usdt,
        f = books.fee_usdt,
        u = books.usdt,
        m = books.maker
    );
    assert_eq!(
        sqlstate(&books.pool, poster, &attributed).await.as_deref(),
        Some("LE012")
    );
    let mut impersonated = books.transfer("tr:2", &alice, &bob, books.usdc, UNIT);
    impersonated.actor = Actor::Customer(bob.id);
    assert_eq!(
        books.ledger.transfer(&impersonated).await,
        Err(LedgerError::InvalidRequest)
    );
    assert_eq!(
        books.records().await,
        before,
        "denied operations write nothing"
    );

    // Postings between platform accounts need no customer activation.
    books
        .ledger
        .post(&PostingCommand {
            identity: EventIdentity::new("FEE_RECOGNIZED", "platform:1", "RECOGNITION").unwrap(),
            source_transaction: None,
            journal: JournalDraft::new(vec![
                line(books.custody_usdt, books.usdt, Direction::Debit, UNIT),
                line(books.fee_usdt, books.usdt, Direction::Credit, UNIT),
            ])
            .unwrap(),
            description: None,
            actor: Actor::System,
            correlation_id: None,
        })
        .await
        .unwrap();

    // A new activation applies its approved per-operation ceiling.
    activate(&books.pool, "CUSTOMER_TRANSFER", books.usdc, 5 * UNIT).await;
    assert_eq!(
        books
            .ledger
            .transfer(&books.transfer("tr:3", &alice, &bob, books.usdc, 5 * UNIT + 1))
            .await,
        Err(LedgerError::NotActivated),
        "above the activated ceiling"
    );
    books
        .ledger
        .transfer(&books.transfer("tr:4", &alice, &bob, books.usdc, 5 * UNIT))
        .await
        .unwrap();
    assert_eq!(books.balance(&alice, books.usdc).await, (45 * UNIT, 0));
    assert_eq!(books.balance(&bob, books.usdc).await, (5 * UNIT, 0));
    assert_eq!(
        books
            .ledger
            .reserve(&books.reserve("res:2", &alice, books.usdc, UNIT))
            .await,
        Err(LedgerError::NotActivated),
        "each capability is activated separately"
    );

    // Activation history cannot be rewritten, only extended.
    let admin = None;
    for sql in [
        format!(
            "UPDATE ledger.capability_activations SET deactivated_at = NULL, deactivated_by = NULL,
             deactivation_reason = NULL WHERE capability = 'CUSTOMER_RESERVATION' AND asset_id = '{}'",
            books.usdc
        ),
        format!(
            "UPDATE ledger.capability_activations SET max_operation_minor = 1000000000000
             WHERE capability = 'CUSTOMER_TRANSFER' AND asset_id = '{}' AND deactivated_at IS NULL",
            books.usdc
        ),
        "DELETE FROM ledger.capability_activations".to_owned(),
    ] {
        assert_eq!(
            sqlstate(&books.pool, admin, &sql).await.as_deref(),
            Some("55000"),
            "{sql}"
        );
    }
    // The shipped application gate refuses on its own as well.
    let service = LedgerService::new(books.ledger.clone(), NoApprovedPolicy);
    assert_eq!(
        service
            .transfer(&books.transfer("tr:5", &alice, &bob, books.usdc, UNIT))
            .await,
        Err(LedgerError::NotEligible)
    );
    assert!(books.violations().await.is_empty());
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn transfers_move_entitlements_atomically(pool: PgPool) {
    let books = setup(pool).await;
    let alice = books.customer("alice").await;
    let bob = books.customer("bob").await;
    books
        .deposit("dep:alice", &alice, books.usdc, 100 * UNIT)
        .await
        .unwrap();
    let sent = books
        .ledger
        .transfer(&books.transfer("tr:1", &alice, &bob, books.usdc, 40 * UNIT))
        .await
        .unwrap();
    assert_eq!(books.balance(&alice, books.usdc).await, (60 * UNIT, 0));
    assert_eq!(books.balance(&bob, books.usdc).await, (40 * UNIT, 0));
    assert_eq!(
        books.account(books.custody_usdc).await,
        (100 * UNIT) as i128,
        "custody is unchanged"
    );
    let replay = books
        .ledger
        .transfer(&books.transfer("tr:1", &alice, &bob, books.usdc, 40 * UNIT))
        .await
        .unwrap();
    assert!(replay.replayed && replay.journal_id == sent.journal_id);
    assert_eq!(
        books
            .ledger
            .transfer(&books.transfer("tr:1", &alice, &bob, books.usdc, 41 * UNIT))
            .await,
        Err(LedgerError::DuplicateAccountingEvent)
    );
    assert_eq!(
        books
            .ledger
            .transfer(&books.transfer("tr:2", &alice, &bob, books.usdc, 60 * UNIT + 1))
            .await,
        Err(LedgerError::InsufficientFunds)
    );
    assert_eq!(
        books
            .ledger
            .transfer(&books.transfer("tr:3", &alice, &bob, books.usdt, UNIT))
            .await,
        Err(LedgerError::InsufficientFunds),
        "USDC never pays for USDT"
    );
    assert_eq!(
        books
            .ledger
            .transfer(&books.transfer("tr:4", &alice, &alice, books.usdc, UNIT))
            .await,
        Err(LedgerError::InvalidRequest)
    );
    let stranger = Customer {
        id: Uuid::from_u128(42),
        ..bob
    };
    assert_eq!(
        books
            .ledger
            .transfer(&books.transfer("tr:5", &alice, &stranger, books.usdc, UNIT))
            .await,
        Err(LedgerError::NotFound)
    );
    assert_eq!(books.balance(&alice, books.usdc).await, (60 * UNIT, 0));
    assert_eq!(books.count("ledger.journals").await, 2);
    assert!(books.violations().await.is_empty());
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn parallel_spending_cannot_overdraw(pool: PgPool) {
    let books = setup(pool).await;
    let alice = books.customer("alice").await;
    let bob = books.customer("bob").await;
    books
        .deposit("dep:alice", &alice, books.usdc, 100 * UNIT)
        .await
        .unwrap();
    books
        .deposit("dep:bob", &bob, books.usdc, 100 * UNIT)
        .await
        .unwrap();

    // Twenty simultaneous 30 USDC reservations against 100 USDC: exactly three fit.
    let mut tasks = Vec::new();
    for i in 0..20 {
        let ledger = books.ledger.clone();
        let command = books.reserve(&format!("withdrawal:{i}"), &alice, books.usdc, 30 * UNIT);
        tasks.push(tokio::spawn(async move { ledger.reserve(&command).await }));
    }
    let mut reserved = 0;
    for task in tasks {
        match task.await.unwrap() {
            Ok(_) => reserved += 1,
            Err(error) => assert_eq!(error, LedgerError::InsufficientFunds),
        }
    }
    assert_eq!(reserved, 3);
    assert_eq!(
        books.balance(&alice, books.usdc).await,
        (10 * UNIT, 90 * UNIT)
    );

    // Crossing transfers in both directions never deadlock or lose value.
    let mut tasks = Vec::new();
    for i in 0..40 {
        let ledger = books.ledger.clone();
        let (from, to) = if i % 2 == 0 {
            (alice, bob)
        } else {
            (bob, alice)
        };
        let command = books.transfer(&format!("cross:{i}"), &from, &to, books.usdc, 7 * UNIT);
        tasks.push(tokio::spawn(async move { ledger.transfer(&command).await }));
    }
    for task in tasks {
        match task.await.unwrap() {
            Ok(_) | Err(LedgerError::InsufficientFunds) => {}
            Err(error) => panic!("unexpected {error:?}"),
        }
    }
    let (alice_available, alice_reserved) = books.balance(&alice, books.usdc).await;
    let (bob_available, bob_reserved) = books.balance(&bob, books.usdc).await;
    assert_eq!(
        alice_available + alice_reserved + bob_available + bob_reserved,
        200 * UNIT
    );
    assert_eq!(alice_reserved, 90 * UNIT);
    assert!(books.violations().await.is_empty());
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn concurrent_duplicate_deliveries_post_once(pool: PgPool) {
    let books = setup(pool).await;
    let alice = books.customer("alice").await;
    let books = std::sync::Arc::new(books);
    let mut tasks = Vec::new();
    for _ in 0..25 {
        let books = books.clone();
        tasks.push(tokio::spawn(async move {
            books
                .deposit("chain:solana:dup:0", &alice, books.usdc, 12 * UNIT)
                .await
        }));
    }
    let mut journals = std::collections::BTreeSet::new();
    let mut fresh = 0;
    for task in tasks {
        let receipt = task.await.unwrap().unwrap();
        journals.insert(receipt.journal_id);
        fresh += usize::from(!receipt.replayed);
    }
    assert_eq!((journals.len(), fresh), (1, 1));
    assert_eq!(books.balance(&alice, books.usdc).await, (12 * UNIT, 0));
    assert_eq!(books.count("ledger.journals").await, 1);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn reservations_follow_evidence_not_timeouts(pool: PgPool) {
    let books = setup(pool).await;
    let alice = books.customer("alice").await;
    let usdt = books.usdt;
    books
        .deposit("dep", &alice, usdt, 100 * UNIT)
        .await
        .unwrap();

    // Withdraw 50 USDT with a 2 USDT fee (ledger spec §15.5): reserve 52.
    let (transaction, operation) = books.withdrawal(&alice, usdt, 52 * UNIT).await;
    let mut command = books.reserve("withdrawal:1", &alice, usdt, 52 * UNIT);
    command.transaction = Some(transaction);
    let held = books.ledger.reserve(&command).await.unwrap();
    assert_eq!(books.balance(&alice, usdt).await, (48 * UNIT, 52 * UNIT));
    let again = books.ledger.reserve(&command).await.unwrap();
    assert!(again.replayed && again.reservation_id == held.reservation_id);
    command.amount = amount(53 * UNIT);
    assert_eq!(
        books.ledger.reserve(&command).await,
        Err(LedgerError::DuplicateAccountingEvent),
        "one operation, one reservation"
    );
    // A timeout leaves the outcome unknown: funds stay reserved.
    let id = held.reservation_id;
    books
        .ledger
        .record_execution(id, ExecutionState::Submitted, "provider:op:1:submitted")
        .await
        .unwrap();
    books
        .ledger
        .record_execution(id, ExecutionState::Unknown, "provider:op:1:timeout")
        .await
        .unwrap();
    for (key, evidence, expected) in [
        (
            "cancel:1",
            ReleaseEvidence::PermittedCancellation,
            LedgerError::ReservationState,
        ),
        (
            "remainder:1",
            ReleaseEvidence::SettlementRemainder,
            LedgerError::ReservationState,
        ),
        (
            "rejected:1",
            ReleaseEvidence::DefinitiveNonExecution,
            LedgerError::EvidenceRequired,
        ),
    ] {
        assert_eq!(
            books
                .ledger
                .release(&books.release(id, key, 52 * UNIT, evidence))
                .await,
            Err(expected),
            "{key}"
        );
    }
    // The workflow cannot declare the outcome itself.
    assert_eq!(
        books
            .ledger
            .record_execution(id, ExecutionState::Executed, "provider:op:1:paid")
            .await,
        Err(LedgerError::EvidenceRequired)
    );
    let settle = books.consume(
        id,
        "chain:tx:1",
        vec![
            (books.custody_usdt, amount(50 * UNIT)),
            (books.fee_usdt, amount(2 * UNIT)),
        ],
    );
    assert_eq!(
        books.ledger.consume(&settle).await,
        Err(LedgerError::EvidenceRequired)
    );
    assert_eq!(books.balance(&alice, usdt).await, (48 * UNIT, 52 * UNIT));
    assert_eq!(
        books
            .ledger
            .record_execution(id, ExecutionState::NotSubmitted, "provider:op:1")
            .await,
        Err(LedgerError::ReservationState)
    );
    // The authenticated provider adapter records execution; settlement then
    // consumes the reservation: custody and fee revenue.
    books
        .outcome(id, operation, ProviderOutcome::Executed)
        .await
        .unwrap();
    let settled = books.ledger.consume(&settle).await.unwrap();
    assert!(books.ledger.consume(&settle).await.unwrap().replayed);
    assert_eq!(
        books.ledger.consume(&settle).await.unwrap().journal_id,
        settled.journal_id
    );
    assert_eq!(books.balance(&alice, usdt).await, (48 * UNIT, 0));
    assert_eq!(books.account(books.custody_usdt).await, (50 * UNIT) as i128);
    assert_eq!(books.account(books.fee_usdt).await, (2 * UNIT) as i128);
    let view = books.ledger.reservation(id).await.unwrap().unwrap();
    assert_eq!(
        (view.status.as_str(), view.execution),
        ("SETTLED", ExecutionState::Executed)
    );
    assert_eq!(view.consumed, units(52 * UNIT));
    assert_eq!(
        books
            .ledger
            .consume(&books.consume(id, "chain:tx:again", vec![(books.custody_usdt, amount(1))]))
            .await,
        Err(LedgerError::ReservationState),
        "a reservation is consumed once"
    );

    // Cancelled before submission: released in full without provider evidence.
    let cancel = books
        .ledger
        .reserve(&books.reserve("withdrawal:2", &alice, usdt, 20 * UNIT))
        .await
        .unwrap();
    books
        .ledger
        .release(&books.release(
            cancel.reservation_id,
            "cancel:2",
            20 * UNIT,
            ReleaseEvidence::PermittedCancellation,
        ))
        .await
        .unwrap();
    // Submitted, then the provider confirms non-execution: released.
    let (failed, _, failed_operation) = books
        .hold("withdrawal:3", &alice, usdt, 30 * UNIT)
        .await
        .unwrap();
    books
        .ledger
        .record_execution(
            failed.reservation_id,
            ExecutionState::Submitted,
            "provider:op:3",
        )
        .await
        .unwrap();
    books
        .outcome(
            failed.reservation_id,
            failed_operation,
            ProviderOutcome::NotExecuted,
        )
        .await
        .unwrap();
    books
        .ledger
        .release(&books.release(
            failed.reservation_id,
            "rejected:3",
            30 * UNIT,
            ReleaseEvidence::DefinitiveNonExecution,
        ))
        .await
        .unwrap();
    assert_eq!(books.balance(&alice, usdt).await, (48 * UNIT, 0));
    let failed_view = books
        .ledger
        .reservation(failed.reservation_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(failed_view.status, "RELEASED");

    // Partial settlement, then the unneeded remainder is released.
    let (partial, _, partial_operation) = books
        .hold("withdrawal:4", &alice, usdt, 10 * UNIT)
        .await
        .unwrap();
    let rid = partial.reservation_id;
    books
        .outcome(rid, partial_operation, ProviderOutcome::Executed)
        .await
        .unwrap();
    assert_eq!(
        books
            .ledger
            .consume(&books.consume(
                rid,
                "chain:tx:4a",
                vec![(books.custody_usdt, amount(11 * UNIT))]
            ))
            .await,
        Err(LedgerError::ReservationState),
        "settlement cannot exceed the reservation"
    );
    assert_eq!(
        books
            .ledger
            .consume(&books.consume(
                rid,
                "chain:tx:4b",
                vec![(alice.usdt.available, amount(UNIT))]
            ))
            .await,
        Err(LedgerError::NotPermitted),
        "reserved funds settle to platform accounts only"
    );
    books
        .ledger
        .consume(&books.consume(
            rid,
            "chain:tx:4",
            vec![(books.custody_usdt, amount(8 * UNIT))],
        ))
        .await
        .unwrap();
    books
        .ledger
        .release(&books.release(
            rid,
            "remainder:4",
            2 * UNIT,
            ReleaseEvidence::SettlementRemainder,
        ))
        .await
        .unwrap();
    let view = books.ledger.reservation(rid).await.unwrap().unwrap();
    assert_eq!(
        (
            view.status.as_str(),
            view.consumed,
            view.released,
            view.remaining
        ),
        (
            "SETTLED",
            units(8 * UNIT),
            units(2 * UNIT),
            MinorUnits::ZERO
        )
    );
    assert_eq!(books.balance(&alice, usdt).await, (40 * UNIT, 0));
    assert!(books.violations().await.is_empty());
    assert_eq!(books.count("ledger.reservation_movements").await, 9);
    let cited: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ledger.reservation_movements WHERE provider_outcome_id IS NOT NULL",
    )
    .fetch_one(&books.pool)
    .await
    .unwrap();
    assert_eq!(
        cited, 4,
        "both settlements, the confirmed non-execution and the remainder cite provider evidence"
    );
}

/// Finding 3: a workflow cannot turn an uncertain outcome into a refund or a
/// settlement, whatever evidence label or reference it supplies. Only the
/// authenticated provider adapter records the outcome, bound to the provider
/// operation of the reservation's own transaction.
#[sqlx::test(migrator = "MIGRATOR")]
async fn settlement_and_release_need_authenticated_provider_evidence(pool: PgPool) {
    let books = setup(pool).await;
    let alice = books.customer("alice").await;
    let usdc = books.usdc;
    books
        .deposit("dep", &alice, usdc, 100 * UNIT)
        .await
        .unwrap();
    let (held, transaction, operation) = books
        .hold("withdrawal:1", &alice, usdc, 40 * UNIT)
        .await
        .unwrap();
    let id = held.reservation_id;
    for state in [ExecutionState::Submitted, ExecutionState::Unknown] {
        books
            .ledger
            .record_execution(id, state, "provider:op:1")
            .await
            .unwrap();
    }
    let before = books.records().await;

    for (key, evidence, expected) in [
        (
            "forged:1",
            ReleaseEvidence::DefinitiveNonExecution,
            LedgerError::EvidenceRequired,
        ),
        (
            "forged:2",
            ReleaseEvidence::PermittedCancellation,
            LedgerError::ReservationState,
        ),
        (
            "forged:3",
            ReleaseEvidence::SettlementRemainder,
            LedgerError::ReservationState,
        ),
    ] {
        assert_eq!(
            books
                .ledger
                .release(&books.release(id, key, 40 * UNIT, evidence))
                .await,
            Err(expected),
            "{key}"
        );
    }
    assert_eq!(
        books
            .ledger
            .consume(&books.consume(
                id,
                "chain:forged",
                vec![(books.custody_usdc, amount(40 * UNIT))]
            ))
            .await,
        Err(LedgerError::EvidenceRequired)
    );
    let poster = Some("deypay_ledger_poster");
    for (sql, expected) in [
        (
            format!(
                "SELECT ledger.record_execution_state('{id}', 'EXECUTED', 'provider says paid')"
            ),
            "LE015",
        ),
        (
            format!(
                "SELECT * FROM ledger.release_reservation('{id}', 'forged:4', 'RELEASE', '40000000',
                 'DEFINITIVE_NON_EXECUTION', 'signed-looking-receipt', 'SYSTEM', NULL, NULL)"
            ),
            "LE015",
        ),
        (
            format!(
                "SELECT * FROM ledger.record_provider_outcome('{id}', 'NOT_EXECUTED', '{operation}',
                 'forged', sha256('forged'), now())"
            ),
            "42501",
        ),
    ] {
        assert_eq!(
            sqlstate(&books.pool, poster, &sql).await.as_deref(),
            Some(expected),
            "{sql}"
        );
    }
    assert_eq!(
        books.records().await,
        before,
        "denied operations write nothing"
    );
    assert_eq!(books.balance(&alice, usdc).await, (60 * UNIT, 40 * UNIT));

    // Evidence must name the provider operation of this reservation's transaction.
    let (other, _, other_operation) = books
        .hold("withdrawal:2", &alice, usdc, 10 * UNIT)
        .await
        .unwrap();
    assert_eq!(
        books
            .outcome(id, other_operation, ProviderOutcome::NotExecuted)
            .await,
        Err(LedgerError::EvidenceRequired)
    );
    let untracked = books
        .ledger
        .reserve(&books.reserve("withdrawal:3", &alice, usdc, 5 * UNIT))
        .await
        .unwrap();
    assert_eq!(
        books
            .outcome(
                untracked.reservation_id,
                operation,
                ProviderOutcome::NotExecuted
            )
            .await,
        Err(LedgerError::EvidenceRequired),
        "a reservation without a transaction cannot be evidenced"
    );

    // One login may not both record provider evidence and move reserved funds.
    test_role(
        &books.pool,
        DUAL_LOGIN,
        "deypay_ledger_poster, deypay_ledger_evidence",
    )
    .await;
    let dual = session_as(&books.pool, DUAL_LOGIN);
    let dual_evidence = PgLedger::acting_as(dual.clone(), LedgerRole::Evidence);
    assert_eq!(
        dual_evidence
            .record_provider_outcome(&books.provider_evidence(
                id,
                operation,
                ProviderOutcome::NotExecuted
            ))
            .await,
        Err(LedgerError::NotPermitted)
    );

    // The adapter records non-execution; identical replays are idempotent and
    // a contradicting outcome is refused for investigation.
    let (outcome_id, replayed) = books
        .outcome(id, operation, ProviderOutcome::NotExecuted)
        .await
        .unwrap();
    assert!(!replayed);
    assert_eq!(
        books
            .outcome(id, operation, ProviderOutcome::NotExecuted)
            .await
            .unwrap(),
        (outcome_id, true)
    );
    assert_eq!(
        books
            .outcome(id, operation, ProviderOutcome::Executed)
            .await,
        Err(LedgerError::DuplicateAccountingEvent)
    );
    // One provider operation evidences one reservation.
    let mut sibling = books.reserve("withdrawal:4", &alice, usdc, 5 * UNIT);
    sibling.transaction = Some(transaction);
    let sibling = books.ledger.reserve(&sibling).await.unwrap();
    assert_eq!(
        books
            .outcome(sibling.reservation_id, operation, ProviderOutcome::Executed)
            .await,
        Err(LedgerError::DuplicateAccountingEvent)
    );
    // The outcome is definitive: no more workflow states, and no settlement.
    assert_eq!(
        books
            .ledger
            .record_execution(id, ExecutionState::Submitted, "provider:op:1:retry")
            .await,
        Err(LedgerError::ReservationState)
    );
    assert_eq!(
        books
            .ledger
            .consume(&books.consume(
                id,
                "chain:late",
                vec![(books.custody_usdc, amount(40 * UNIT))]
            ))
            .await,
        Err(LedgerError::EvidenceRequired)
    );
    assert_eq!(
        PgLedger::acting_as(dual, LedgerRole::Poster)
            .release(&books.release(
                id,
                "dual:1",
                40 * UNIT,
                ReleaseEvidence::DefinitiveNonExecution
            ))
            .await,
        Err(LedgerError::NotPermitted)
    );
    // The workflow releases against it, once.
    let release = books.release(
        id,
        "rejected:1",
        40 * UNIT,
        ReleaseEvidence::DefinitiveNonExecution,
    );
    let released = books.ledger.release(&release).await.unwrap();
    assert!(books.ledger.release(&release).await.unwrap().replayed);
    let cited: Option<Uuid> = sqlx::query_scalar(
        "SELECT provider_outcome_id FROM ledger.reservation_movements WHERE journal_id = $1",
    )
    .bind(released.journal_id)
    .fetch_one(&books.pool)
    .await
    .unwrap();
    assert_eq!(cited, Some(outcome_id));
    assert_eq!(books.balance(&alice, usdc).await, (80 * UNIT, 20 * UNIT));
    assert_eq!(
        books
            .ledger
            .reservation(other.reservation_id)
            .await
            .unwrap()
            .unwrap()
            .status,
        "ACTIVE"
    );
    for sql in [
        "UPDATE ledger.provider_outcomes SET outcome = 'EXECUTED'",
        "DELETE FROM ledger.provider_outcomes",
    ] {
        assert_eq!(
            sqlstate(&books.pool, None, sql).await.as_deref(),
            Some("55000"),
            "{sql}"
        );
    }
    assert!(books.violations().await.is_empty());
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn reversals_compensate_with_maker_checker_approval(pool: PgPool) {
    let books = setup(pool).await;
    let alice = books.customer("alice").await;
    let bob = books.customer("bob").await;
    let carol = books.customer("carol").await;
    books
        .deposit("dep", &alice, books.usdc, 100 * UNIT)
        .await
        .unwrap();
    let mistaken = books
        .ledger
        .transfer(&books.transfer("tr:1", &alice, &bob, books.usdc, 10 * UNIT))
        .await
        .unwrap();
    let entries_before: Vec<(String, String)> = sqlx::query_as(
        "SELECT direction, amount_minor::text FROM ledger.entries WHERE journal_id = $1 ORDER BY line_number",
    )
    .bind(mistaken.journal_id)
    .fetch_all(&books.pool)
    .await
    .unwrap();

    let control = &books.control;
    let approve = &books.approve;
    let outsider = books.operator(OUTSIDER_LOGIN);
    assert_eq!(
        outsider
            .request_reversal(
                mistaken.journal_id,
                "Sent in error",
                "case:1",
                books.outsider
            )
            .await,
        Err(LedgerError::ApprovalRequired)
    );
    let request = control
        .request_reversal(mistaken.journal_id, "Sent in error", "case:1", books.maker)
        .await
        .unwrap();
    assert_eq!(
        approve.execute_correction(request, books.checker).await,
        Err(LedgerError::ApprovalRequired)
    );
    assert_eq!(
        control
            .decide_correction(request, books.maker, true, "Self approval")
            .await,
        Err(LedgerError::ApprovalRequired),
        "nobody approves their own correction"
    );
    assert_eq!(
        outsider
            .decide_correction(request, books.outsider, true, "No permission")
            .await,
        Err(LedgerError::ApprovalRequired)
    );
    assert_eq!(
        approve
            .decide_correction(request, books.checker, true, "Evidence reviewed")
            .await
            .unwrap(),
        "APPROVED"
    );
    let outcome = approve
        .execute_correction(request, books.checker)
        .await
        .unwrap();
    let CorrectionOutcome::Executed { journal_id } = &outcome else {
        panic!("{outcome:?}")
    };
    let journal_id = *journal_id;
    assert_eq!(
        control
            .execute_correction(request, books.maker)
            .await
            .unwrap(),
        outcome,
        "executes once"
    );
    assert_eq!(books.balance(&alice, books.usdc).await, (100 * UNIT, 0));
    assert_eq!(books.balance(&bob, books.usdc).await, (0, 0));
    let entries_after: Vec<(String, String)> = sqlx::query_as(
        "SELECT direction, amount_minor::text FROM ledger.entries WHERE journal_id = $1 ORDER BY line_number",
    )
    .bind(mistaken.journal_id)
    .fetch_all(&books.pool)
    .await
    .unwrap();
    assert_eq!(
        entries_before, entries_after,
        "the original journal is untouched"
    );
    let reversal_of: Option<Uuid> =
        sqlx::query_scalar("SELECT reversal_of_journal_id FROM ledger.journals WHERE id = $1")
            .bind(journal_id)
            .fetch_one(&books.pool)
            .await
            .unwrap();
    assert_eq!(reversal_of, Some(mistaken.journal_id));
    assert_eq!(
        control
            .request_reversal(mistaken.journal_id, "Again", "case:1", books.maker)
            .await,
        Err(LedgerError::InvalidReversal)
    );
    assert_eq!(
        control
            .request_reversal(journal_id, "Undo", "case:1", books.maker)
            .await,
        Err(LedgerError::InvalidReversal)
    );

    // The recipient already spent the funds: a recovery case, not an overdraft.
    let second = books
        .ledger
        .transfer(&books.transfer("tr:2", &alice, &bob, books.usdc, 30 * UNIT))
        .await
        .unwrap();
    books
        .ledger
        .transfer(&books.transfer("tr:3", &bob, &carol, books.usdc, 25 * UNIT))
        .await
        .unwrap();
    let request = control
        .request_reversal(second.journal_id, "Disputed", "case:2", books.maker)
        .await
        .unwrap();
    approve
        .decide_correction(request, books.checker, true, "Investigated")
        .await
        .unwrap();
    let CorrectionOutcome::RecoveryRequired { recovery_case_id } = approve
        .execute_correction(request, books.checker)
        .await
        .unwrap()
    else {
        panic!("expected a recovery case")
    };
    assert_eq!(
        books.balance(&bob, books.usdc).await,
        (5 * UNIT, 0),
        "nothing was debited"
    );
    let (status, shortfall): (String, String) = sqlx::query_as(
        "SELECT status, shortfall->0->>'shortfall_minor' FROM ledger.recovery_cases WHERE id = $1",
    )
    .bind(recovery_case_id)
    .fetch_one(&books.pool)
    .await
    .unwrap();
    assert_eq!(
        (status.as_str(), shortfall),
        ("OPEN", (25 * UNIT).to_string())
    );

    // Reservation movements are corrected by their own lifecycle, never reversed.
    let held = books
        .ledger
        .reserve(&books.reserve("withdrawal:9", &alice, books.usdc, UNIT))
        .await
        .unwrap();
    assert_eq!(
        control
            .request_reversal(held.journal_id, "Wrong", "case:3", books.maker)
            .await,
        Err(LedgerError::InvalidReversal)
    );

    // A balanced adjustment needs the same approval before it posts.
    let adjustment = control
        .request_adjustment(
            &[
                (
                    books.suspense_usdc,
                    books.usdc,
                    Direction::Debit,
                    amount(5 * UNIT),
                ),
                (
                    carol.usdc.available,
                    books.usdc,
                    Direction::Credit,
                    amount(5 * UNIT),
                ),
            ],
            "Goodwill credit",
            "case:4",
            books.maker,
        )
        .await
        .unwrap();
    assert_eq!(
        books.balance(&carol, books.usdc).await,
        (25 * UNIT, 0),
        "nothing posts before approval"
    );
    approve
        .decide_correction(adjustment, books.checker, true, "Approved by finance")
        .await
        .unwrap();
    assert!(matches!(
        control
            .execute_correction(adjustment, books.maker)
            .await
            .unwrap(),
        CorrectionOutcome::Executed { .. }
    ));
    assert_eq!(books.balance(&carol, books.usdc).await, (30 * UNIT, 0));
    let unbalanced = control
        .request_adjustment(
            &[
                (
                    books.suspense_usdc,
                    books.usdc,
                    Direction::Debit,
                    amount(5 * UNIT),
                ),
                (
                    carol.usdc.available,
                    books.usdc,
                    Direction::Credit,
                    amount(4 * UNIT),
                ),
            ],
            "Wrong",
            "case:5",
            books.maker,
        )
        .await;
    assert_eq!(unbalanced, Err(LedgerError::UnbalancedJournal));
    let audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit.audit_events WHERE action LIKE 'LEDGER_CORRECTION_%'",
    )
    .fetch_one(&books.pool)
    .await
    .unwrap();
    // Two executed corrections (requested, approved, executed) and the reversal
    // that became a recovery case (requested, approved).
    assert_eq!(audits, 8);
    assert!(books.violations().await.is_empty());
}

/// Finding 2: maker-checker rests on the authenticated database login, not
/// on operator UUIDs supplied by the caller.
#[sqlx::test(migrator = "MIGRATOR")]
async fn operator_identity_is_bound_to_the_authenticated_session(pool: PgPool) {
    let books = setup(pool).await;
    let alice = books.customer("alice").await;
    let bob = books.customer("bob").await;
    books
        .deposit("dep", &alice, books.usdc, 10 * UNIT)
        .await
        .unwrap();
    let sent = books
        .ledger
        .transfer(&books.transfer("tr:1", &alice, &bob, books.usdc, 4 * UNIT))
        .await
        .unwrap();
    let period = ("2000-01-01T00:00:00Z", "2100-01-01T00:00:00Z");
    let before = books.records().await;

    // A controller login bound to nobody cannot act for anyone, whatever UUID it states.
    let service = PgLedger::acting_as(books.pool.clone(), LedgerRole::Controller);
    assert_eq!(
        service
            .request_reversal(sent.journal_id, "Impersonated", "case:1", books.maker)
            .await,
        Err(LedgerError::OperatorNotAuthenticated)
    );
    assert_eq!(
        service
            .open_platform_account("1150", books.usdc, "primary", books.maker)
            .await,
        Err(LedgerError::OperatorNotAuthenticated)
    );
    assert_eq!(
        service
            .start_reconciliation("LEDGER_INTEGRITY", None, period, books.maker)
            .await,
        Err(LedgerError::OperatorNotAuthenticated)
    );
    assert_eq!(books.records().await, before, "nothing was recorded");

    let request = books
        .control
        .request_reversal(sent.journal_id, "Sent in error", "case:1", books.maker)
        .await
        .unwrap();
    // The maker cannot approve or execute by naming the checker.
    assert_eq!(
        books
            .control
            .decide_correction(request, books.checker, true, "Forged approval")
            .await,
        Err(LedgerError::OperatorNotAuthenticated)
    );
    assert_eq!(
        books
            .control
            .execute_correction(request, books.checker)
            .await,
        Err(LedgerError::OperatorNotAuthenticated)
    );
    // Nor become the checker: role membership does not change the session user,
    // and the maker's login is not a member of the checker's.
    assert_eq!(
        sqlstate(
            &books.maker_session,
            None,
            &format!("SET ROLE {CHECKER_LOGIN}")
        )
        .await
        .as_deref(),
        Some("42501")
    );
    // Nor rebind its login to the checker.
    assert_eq!(
        sqlstate(
            &books.maker_session,
            Some("deypay_ledger_controller"),
            &format!(
                "INSERT INTO ledger.operator_logins(login_name, user_id, approval_reference)
                 VALUES ('{MAKER_LOGIN}', '{}', 'self-bound')",
                books.checker
            )
        )
        .await
        .as_deref(),
        Some("42501")
    );
    assert_eq!(
        books
            .control
            .decide_correction(request, books.maker, true, "Self approval")
            .await,
        Err(LedgerError::ApprovalRequired)
    );

    // A reconciliation run stays with the operator who started it.
    let run = books
        .control
        .start_reconciliation("LEDGER_INTEGRITY", None, period, books.maker)
        .await
        .unwrap();
    assert_eq!(
        books.approve.reconcile_ledger_integrity(run).await,
        Err(LedgerError::ApprovalRequired)
    );
    assert_eq!(
        service.reconcile_ledger_integrity(run).await,
        Err(LedgerError::OperatorNotAuthenticated)
    );
    assert_eq!(books.control.reconcile_ledger_integrity(run).await, Ok(0));

    // Revoked permission: the checker's assignment is withdrawn.
    sqlx::query("UPDATE audit.admin_assignments SET revoked_at = now() WHERE user_id = $1")
        .bind(books.checker)
        .execute(&books.pool)
        .await
        .unwrap();
    assert_eq!(
        books
            .approve
            .decide_correction(request, books.checker, true, "Revoked")
            .await,
        Err(LedgerError::ApprovalRequired)
    );
    sqlx::query(
        "INSERT INTO audit.admin_assignments(user_id, role_id)
         SELECT user_id, role_id FROM audit.admin_assignments WHERE user_id = $1 LIMIT 1",
    )
    .bind(books.checker)
    .execute(&books.pool)
    .await
    .unwrap();
    assert_eq!(
        books
            .approve
            .decide_correction(request, books.checker, true, "Evidence reviewed")
            .await
            .unwrap(),
        "APPROVED"
    );
    // Unauthorized execution: an authenticated operator without correction permissions.
    assert_eq!(
        books
            .operator(OUTSIDER_LOGIN)
            .execute_correction(request, books.outsider)
            .await,
        Err(LedgerError::ApprovalRequired)
    );
    // A revoked login binding ends the operator's authority at once, and the
    // binding history cannot be rewritten.
    sqlx::query(
        "UPDATE ledger.operator_logins SET revoked_at = now(), revoked_by = session_user WHERE login_name = $1",
    )
    .bind(CHECKER_LOGIN)
    .execute(&books.pool)
    .await
    .unwrap();
    assert_eq!(
        books
            .approve
            .execute_correction(request, books.checker)
            .await,
        Err(LedgerError::OperatorNotAuthenticated)
    );
    for sql in [
        format!(
            "UPDATE ledger.operator_logins SET revoked_at = NULL, revoked_by = NULL WHERE login_name = '{CHECKER_LOGIN}'"
        ),
        format!(
            "UPDATE ledger.operator_logins SET user_id = '{}' WHERE login_name = '{MAKER_LOGIN}'",
            books.checker
        ),
        "DELETE FROM ledger.operator_logins".to_owned(),
    ] {
        assert_eq!(
            sqlstate(&books.pool, None, &sql).await.as_deref(),
            Some("55000"),
            "{sql}"
        );
    }

    // The authorized path still works, attributed to the authenticated logins.
    assert!(matches!(
        books
            .control
            .execute_correction(request, books.maker)
            .await
            .unwrap(),
        CorrectionOutcome::Executed { .. }
    ));
    assert_eq!(books.balance(&alice, books.usdc).await, (10 * UNIT, 0));
    let trail: Vec<(String, Uuid, String)> = sqlx::query_as(
        "SELECT action, actor_id, metadata->>'database_login' FROM audit.audit_events
         WHERE resource_id = $1 ORDER BY created_at, id",
    )
    .bind(request)
    .fetch_all(&books.pool)
    .await
    .unwrap();
    assert_eq!(
        trail,
        vec![
            (
                "LEDGER_CORRECTION_REQUESTED".to_owned(),
                books.maker,
                MAKER_LOGIN.to_owned()
            ),
            (
                "LEDGER_CORRECTION_APPROVED".to_owned(),
                books.checker,
                CHECKER_LOGIN.to_owned()
            ),
            (
                "LEDGER_CORRECTION_EXECUTED".to_owned(),
                books.maker,
                MAKER_LOGIN.to_owned()
            ),
        ]
    );
    assert_eq!(books.count("ledger.correction_requests").await, 1);
    assert!(books.violations().await.is_empty());
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn failures_and_crashes_leave_no_partial_posting(pool: PgPool) {
    let books = setup(pool).await;
    let alice = books.customer("alice").await;
    let bob = books.customer("bob").await;
    books
        .deposit("dep", &alice, books.usdc, 10 * UNIT)
        .await
        .unwrap();
    let before = books.records().await;
    // A failing transfer writes nothing.
    assert_eq!(
        books
            .ledger
            .transfer(&books.transfer("tr:big", &alice, &bob, books.usdc, 11 * UNIT))
            .await,
        Err(LedgerError::InsufficientFunds)
    );
    // A caller transaction that rolls back takes the whole posting with it.
    let post = format!(
        "SELECT * FROM ledger.post_journal('DEPOSIT_CONFIRMED', 'rolled-back', 'RECOGNITION', NULL,
         ARRAY['{}','{}']::uuid[], ARRAY['{u}','{u}']::uuid[], ARRAY['DEBIT','CREDIT'], ARRAY['5','5'],
         NULL, 'SYSTEM', NULL, NULL)",
        books.custody_usdc,
        alice.usdc.available,
        u = books.usdc
    );
    let mut tx = books.pool.begin().await.unwrap();
    sqlx::query("SET LOCAL ROLE deypay_ledger_poster")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::raw_sql(&post).execute(&mut *tx).await.unwrap();
    tx.rollback().await.unwrap();
    // A process that dies mid-transaction commits nothing.
    let mut victim = books.pool.acquire().await.unwrap();
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *victim)
        .await
        .unwrap();
    sqlx::raw_sql("BEGIN").execute(&mut *victim).await.unwrap();
    sqlx::raw_sql(&post).execute(&mut *victim).await.unwrap();
    let killed: bool = sqlx::query_scalar("SELECT pg_terminate_backend($1)")
        .bind(pid)
        .fetch_one(&books.pool)
        .await
        .unwrap();
    assert!(killed);
    drop(victim.detach());
    assert_eq!(books.records().await, before);
    assert_eq!(books.balance(&alice, books.usdc).await, (10 * UNIT, 0));
    // The interrupted event posts exactly once when retried.
    let mut tx = books.pool.begin().await.unwrap();
    sqlx::query("SET LOCAL ROLE deypay_ledger_poster")
        .execute(&mut *tx)
        .await
        .unwrap();
    let replayed: bool = sqlx::query_scalar(&post.replace("SELECT *", "SELECT replayed"))
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert!(!replayed);
    assert_eq!(books.balance(&alice, books.usdc).await, (10 * UNIT + 5, 0));
    assert!(books.violations().await.is_empty());
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn reconciliation_records_every_discrepancy(pool: PgPool) {
    let books = setup(pool).await;
    let alice = books.customer("alice").await;
    let control = &books.control;
    let evidence = [7_u8; 32];
    let period = ("2000-01-01T00:00:00Z", "2100-01-01T00:00:00Z");
    books
        .deposit("chain:tx:1", &alice, books.usdc, 100 * UNIT)
        .await
        .unwrap();
    books
        .deposit("chain:tx:2", &alice, books.usdc, 20 * UNIT)
        .await
        .unwrap();

    // Internal reconciliation of a consistent ledger.
    let run = control
        .start_reconciliation("LEDGER_INTEGRITY", None, period, books.maker)
        .await
        .unwrap();
    assert_eq!(control.reconcile_ledger_integrity(run).await.unwrap(), 0);
    assert_eq!(
        control
            .reconcile_customer_liabilities(run, books.usdc)
            .await
            .unwrap(),
        "MATCHED"
    );
    assert_eq!(
        control
            .complete_reconciliation(run, &[], None)
            .await
            .unwrap(),
        "COMPLETED"
    );
    assert_eq!(
        control.reconcile_ledger_integrity(run).await,
        Err(LedgerError::ReservationState),
        "a completed run is closed"
    );

    // Provider statement lines and balances against the books.
    let observed = "2026-11-26T10:00:00Z";
    let run = control
        .start_reconciliation("PROVIDER_TRANSACTIONS", None, period, books.maker)
        .await
        .unwrap();
    let tx = |reference: &'static str, value: u128| {
        let control = control.clone();
        let custody = books.custody_usdc;
        let usdc = books.usdc;
        async move {
            control
                .record_transaction_evidence(
                    run,
                    reference,
                    custody,
                    usdc,
                    Direction::Debit,
                    amount(value),
                    ("statement:line", evidence),
                    observed,
                )
                .await
                .unwrap()
        }
    };
    assert_eq!(tx("chain:tx:1", 100 * UNIT).await, "MATCHED");
    assert_eq!(tx("chain:tx:1", 100 * UNIT).await, "DUPLICATE");
    assert_eq!(tx("chain:tx:unknown", 5 * UNIT).await, "MISSING_INTERNAL");
    assert_eq!(tx("chain:tx:9", 5 * UNIT).await, "MISSING_INTERNAL");
    assert_eq!(
        control
            .record_balance_evidence(
                run,
                books.custody_usdc,
                units(120 * UNIT),
                "custody:report",
                evidence,
                observed
            )
            .await
            .unwrap(),
        "MATCHED"
    );
    assert_eq!(
        control
            .record_balance_evidence(
                run,
                books.custody_usdc,
                units(119 * UNIT),
                "custody:report:2",
                evidence,
                observed
            )
            .await
            .unwrap(),
        "AMOUNT_MISMATCH"
    );
    assert_eq!(
        books
            .approve
            .record_balance_evidence(
                run,
                books.custody_usdc,
                units(120 * UNIT),
                "custody:report:3",
                evidence,
                observed
            )
            .await,
        Err(LedgerError::ApprovalRequired),
        "only the operator running the reconciliation adds evidence to it"
    );
    assert_eq!(
        control
            .complete_reconciliation(run, &["DEPOSIT_CONFIRMED"], Some(books.custody_usdc))
            .await
            .unwrap(),
        "COMPLETED_WITH_DISCREPANCIES"
    );
    let statuses: Vec<String> = sqlx::query_scalar(
        "SELECT match_status FROM operations.reconciliation_items WHERE reconciliation_run_id = $1 ORDER BY created_at, id",
    )
    .bind(run)
    .fetch_all(&books.pool)
    .await
    .unwrap();
    assert!(
        statuses.contains(&"MISSING_EXTERNAL".to_owned()),
        "chain:tx:2 had no statement line: {statuses:?}"
    );
    let open_cases: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM operations.reconciliation_cases c JOIN operations.reconciliation_items i
         ON i.id = c.reconciliation_item_id WHERE i.reconciliation_run_id = $1 AND c.status = 'OPEN'",
    )
    .bind(run)
    .fetch_one(&books.pool)
    .await
    .unwrap();
    assert_eq!(open_cases, 5);
    assert_eq!(
        books.account(books.custody_usdc).await,
        (120 * UNIT) as i128,
        "nothing was adjusted to match"
    );

    // A liability without backing is a critical shortfall that halts spending.
    let unbacked = PostingCommand {
        identity: EventIdentity::new("UNIDENTIFIED_CREDIT", "manual:1", "RECOGNITION").unwrap(),
        source_transaction: None,
        journal: JournalDraft::new(vec![
            line(books.suspense_usdc, books.usdc, Direction::Debit, 7 * UNIT),
            line(
                alice.usdc.available,
                books.usdc,
                Direction::Credit,
                7 * UNIT,
            ),
        ])
        .unwrap(),
        description: None,
        actor: Actor::System,
        correlation_id: None,
    };
    books.ledger.post(&unbacked).await.unwrap();
    let run = control
        .start_reconciliation("CUSTOMER_LIABILITY", None, period, books.maker)
        .await
        .unwrap();
    assert_eq!(
        control
            .reconcile_customer_liabilities(run, books.usdc)
            .await
            .unwrap(),
        "SHORTFALL"
    );
    assert_eq!(
        books
            .ledger
            .reserve(&books.reserve("withdrawal:held", &alice, books.usdc, UNIT))
            .await,
        Err(LedgerError::AssetOnHold)
    );
    let (hold, case): (Uuid, Uuid) = sqlx::query_as(
        "SELECT h.id, h.reconciliation_case_id FROM ledger.asset_holds h WHERE h.active AND h.asset_id = $1",
    )
    .bind(books.usdc)
    .fetch_one(&books.pool)
    .await
    .unwrap();
    let controller = Some("deypay_ledger_controller");
    let lift = format!(
        "SELECT ledger.lift_asset_hold('{hold}', 'Investigated', '{}')",
        books.maker
    );
    assert_eq!(
        sqlstate(&books.maker_session, controller, &lift)
            .await
            .as_deref(),
        Some("LE007")
    );
    assert_eq!(
        sqlstate(&books.pool, controller, &lift).await.as_deref(),
        Some("LE016"),
        "an unbound login cannot lift a hold by naming an operator"
    );
    let resolve_without_evidence = format!(
        "SELECT operations.update_case('{case}', 'RESOLVED', NULL, 'Explained', NULL, '{}')",
        books.maker
    );
    assert_eq!(
        sqlstate(&books.maker_session, controller, &resolve_without_evidence)
            .await
            .as_deref(),
        Some("LE006")
    );
    let resolve = format!(
        "SELECT operations.update_case('{case}', 'RESOLVED', NULL, 'Funded from treasury', 'treasury:transfer:1', '{}')",
        books.maker
    );
    assert_eq!(
        sqlstate(&books.maker_session, controller, &resolve).await,
        None
    );
    assert_eq!(
        sqlstate(&books.maker_session, controller, &lift).await,
        None
    );
    books
        .ledger
        .reserve(&books.reserve("withdrawal:released", &alice, books.usdc, UNIT))
        .await
        .unwrap();

    // Drifted projections are found by rebuilding balances from entries.
    sqlx::query(
        "UPDATE ledger.balances SET credit_total = credit_total + 1, balance_minor = balance_minor + 1 WHERE account_id = $1",
    )
    .bind(alice.usdc.available)
    .execute(&books.pool)
    .await
    .unwrap();
    let run = control
        .start_reconciliation("LEDGER_INTEGRITY", None, period, books.maker)
        .await
        .unwrap();
    assert_eq!(control.reconcile_ledger_integrity(run).await.unwrap(), 1);
    assert_eq!(
        books.violations().await,
        vec!["PROJECTION_DRIFT".to_owned()]
    );
    for table in [
        "operations.reconciliation_items",
        "operations.reconciliation_cases",
        "operations.reconciliation_case_events",
    ] {
        assert_eq!(
            sqlstate(&books.pool, None, &format!("DELETE FROM {table}"))
                .await
                .as_deref(),
            Some("55000"),
            "{table}"
        );
    }
}

/// Model-based randomized test (ledger spec §39.11): a random mix of deposits,
/// transfers, reservations, evidenced settlements and releases always agrees
/// with an independent in-memory model, and every ledger invariant holds.
#[sqlx::test(migrator = "MIGRATOR")]
async fn randomized_operations_preserve_every_invariant(pool: PgPool) {
    let books = setup(pool).await;
    let mut customers = Vec::new();
    for name in ["r0", "r1", "r2"] {
        customers.push(books.customer(name).await);
    }
    let assets = [books.usdc, books.usdt];
    let mut available = [[0_u128; 2]; 3];
    let mut reserved = [[0_u128; 2]; 3];
    let mut open: Vec<(Uuid, Uuid, usize, usize, u128)> = Vec::new();
    let mut seed = 0x2545_f491_4f6c_dd1d_u64;
    let mut next = |bound: u64| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed % bound
    };
    for step in 0..160 {
        let who = next(3) as usize;
        let which = next(2) as usize;
        let asset = assets[which];
        let value = 1 + u128::from(next(50)) * UNIT / 10;
        match next(5) {
            0 => {
                books
                    .deposit(&format!("dep:{step}"), &customers[who], asset, value)
                    .await
                    .unwrap();
                available[who][which] += value;
            }
            1 => {
                let to = (who + 1 + next(2) as usize) % 3;
                let result = books
                    .ledger
                    .transfer(&books.transfer(
                        &format!("tr:{step}"),
                        &customers[who],
                        &customers[to],
                        asset,
                        value,
                    ))
                    .await;
                if available[who][which] >= value {
                    result.unwrap();
                    available[who][which] -= value;
                    available[to][which] += value;
                } else {
                    assert_eq!(result, Err(LedgerError::InsufficientFunds));
                }
            }
            2 => {
                let result = books
                    .hold(&format!("res:{step}"), &customers[who], asset, value)
                    .await;
                if available[who][which] >= value {
                    let (receipt, _, operation) = result.unwrap();
                    open.push((receipt.reservation_id, operation, who, which, value));
                    available[who][which] -= value;
                    reserved[who][which] += value;
                } else {
                    assert_eq!(result.err(), Some(LedgerError::InsufficientFunds));
                }
            }
            3 if !open.is_empty() => {
                let (id, operation, owner, index, held) =
                    open.swap_remove(next(open.len() as u64) as usize);
                let custody = books.custody(assets[index]);
                books
                    .outcome(id, operation, ProviderOutcome::Executed)
                    .await
                    .unwrap();
                books
                    .ledger
                    .consume(&books.consume(
                        id,
                        &format!("settle:{step}"),
                        vec![(custody, amount(held))],
                    ))
                    .await
                    .unwrap();
                reserved[owner][index] -= held;
            }
            4 if !open.is_empty() => {
                let (id, _, owner, index, held) =
                    open.swap_remove(next(open.len() as u64) as usize);
                books
                    .ledger
                    .release(&books.release(
                        id,
                        &format!("cancel:{step}"),
                        held,
                        ReleaseEvidence::PermittedCancellation,
                    ))
                    .await
                    .unwrap();
                reserved[owner][index] -= held;
                available[owner][index] += held;
            }
            _ => {}
        }
    }
    for (who, customer) in customers.iter().enumerate() {
        for (which, asset) in assets.iter().enumerate() {
            assert_eq!(
                books.balance(customer, *asset).await,
                (available[who][which], reserved[who][which])
            );
        }
    }
    // Value only entered through backed deposits and left through custody, so
    // custody equals customer liabilities exactly for each asset.
    for (which, asset) in assets.iter().enumerate() {
        let liabilities: u128 = (0..3)
            .map(|who| available[who][which] + reserved[who][which])
            .sum();
        assert_eq!(
            books.account(books.custody(*asset)).await,
            liabilities as i128
        );
    }
    assert!(books.violations().await.is_empty());
    let drift: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ledger.balances b JOIN ledger.reconstructed_balances() r USING (account_id)
         WHERE b.balance_minor <> r.balance_minor",
    )
    .fetch_one(&books.pool)
    .await
    .unwrap();
    assert_eq!(drift, 0);
}

/// Finding 4: the documented restricted roles install the ledger. A
/// non-superuser migration administrator runs every migration and the grants
/// (whose verification must pass), and the result starts fail-closed.
#[sqlx::test(migrations = false)]
async fn restricted_roles_install_the_ledger(pool: PgPool) {
    sqlx::raw_sql(ROLES).execute(&pool).await.unwrap();
    test_role(&pool, MIGRATION_LOGIN, "deypay_ledger_owner").await;
    let database: String = sqlx::query_scalar("SELECT current_database()")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::raw_sql(&format!(
        "GRANT CREATE ON DATABASE \"{database}\" TO {MIGRATION_LOGIN};
         GRANT CREATE ON SCHEMA public TO {MIGRATION_LOGIN};"
    ))
    .execute(&pool)
    .await
    .unwrap();
    let migration = session_as(&pool, MIGRATION_LOGIN);
    let mut conn = migration.acquire().await.unwrap();
    let (login, superuser): (String, bool) = sqlx::query_as(
        "SELECT session_user::text, rolsuper FROM pg_roles WHERE rolname = session_user",
    )
    .fetch_one(&mut *conn)
    .await
    .unwrap();
    assert_eq!((login.as_str(), superuser), (MIGRATION_LOGIN, false));
    MIGRATOR.run(&mut *conn).await.unwrap();
    sqlx::raw_sql(GRANTS).execute(&mut *conn).await.unwrap();
    drop(conn);
    migration.close().await;

    let owner: String = sqlx::query_scalar(
        "SELECT tableowner::text FROM pg_tables WHERE schemaname = 'ledger' AND tablename = 'entries'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(owner, MIGRATION_LOGIN);
    let foreign: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pg_proc WHERE pronamespace IN ('ledger'::regnamespace, 'operations'::regnamespace)
           AND proowner <> 'deypay_ledger_owner'::regrole",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        foreign, 0,
        "every ledger routine runs as deypay_ledger_owner"
    );
    for table in [
        "ledger.capability_activations",
        "ledger.operator_logins",
        "ledger.journals",
    ] {
        let rows: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {table}"))
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            rows, 0,
            "{table}: a new installation is empty and fail-closed"
        );
    }
}
