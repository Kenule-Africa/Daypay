//! Financial invariant tests for the Phase 5 ledger (IMP-P5-017), run against
//! real PostgreSQL through the production grants: postings use the
//! deypay_ledger_poster role and corrections/reconciliation use
//! deypay_ledger_controller. Assets, customers and operators are test fixtures.
use deypay_application::ledger::*;
use deypay_domain::ledger::{Direction, EventIdentity, JournalDraft, PostingLine};
use deypay_domain::units::{MinorUnits, PostingAmount};
use deypay_infrastructure::MIGRATOR;
use deypay_infrastructure::ledger::{CorrectionOutcome, LedgerRole, PgLedger};
use sqlx::{PgPool, Row};
use uuid::Uuid;

const ROLES: &str = include_str!("../../../../infrastructure/postgres/ledger-roles.psql");
const GRANTS: &str = include_str!("../../../../infrastructure/postgres/ledger-grants.psql");
/// One whole USDC or USDT at the approved six decimal places.
const UNIT: u128 = 1_000_000;

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
    ledger: PgLedger,
    control: PgLedger,
    usdc: Uuid,
    usdt: Uuid,
    custody_usdc: Uuid,
    custody_usdt: Uuid,
    fee_usdt: Uuid,
    suspense_usdc: Uuid,
    maker: Uuid,
    checker: Uuid,
    outsider: Uuid,
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

async fn setup(pool: PgPool) -> Books {
    sqlx::raw_sql(ROLES).execute(&pool).await.unwrap();
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
    let control = PgLedger::acting_as(pool.clone(), LedgerRole::Controller);
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
    Books {
        ledger: PgLedger::acting_as(pool.clone(), LedgerRole::Poster),
        control,
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

/// Runs SQL as `role` (or the test administrator) and returns the SQLSTATE of
/// any error, including one raised by a deferred check at commit.
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
    for (role, sql) in [
        (
            "deypay_app",
            "SELECT count(*) FROM ledger.entries".to_owned(),
        ),
        ("deypay_app", post.clone()),
        (
            "deypay_app",
            "SELECT * FROM ledger.customer_balance(gen_random_uuid(), gen_random_uuid())"
                .to_owned(),
        ),
        ("deypay_ledger_poster", insert.clone()),
        ("deypay_ledger_poster", raise.clone()),
        ("deypay_ledger_poster", internal.clone()),
        ("deypay_ledger_poster", correction),
        ("deypay_ledger_controller", post.clone()),
        ("deypay_ledger_controller", insert.clone()),
        ("deypay_ledger_controller", internal),
        ("deypay_ledger_reader", post),
        ("deypay_ledger_reader", raise),
    ] {
        assert_eq!(
            sqlstate(&books.pool, Some(role), &sql).await.as_deref(),
            Some("42501"),
            "{role}: {sql}"
        );
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
            .control
            .open_platform_account("1130", ngn, "primary", books.outsider)
            .await,
        Err(LedgerError::ApprovalRequired)
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
    let held = books
        .ledger
        .reserve(&books.reserve("withdrawal:1", &alice, usdt, 52 * UNIT))
        .await
        .unwrap();
    assert_eq!(books.balance(&alice, usdt).await, (48 * UNIT, 52 * UNIT));
    let again = books
        .ledger
        .reserve(&books.reserve("withdrawal:1", &alice, usdt, 52 * UNIT))
        .await
        .unwrap();
    assert!(again.replayed && again.reservation_id == held.reservation_id);
    assert_eq!(
        books
            .ledger
            .reserve(&books.reserve("withdrawal:1", &alice, usdt, 53 * UNIT))
            .await,
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
    assert_eq!(
        books
            .ledger
            .release(&books.release(
                id,
                "cancel:1",
                52 * UNIT,
                ReleaseEvidence::PermittedCancellation
            ))
            .await,
        Err(LedgerError::ReservationState)
    );
    assert_eq!(
        books
            .ledger
            .release(&books.release(
                id,
                "remainder:1",
                52 * UNIT,
                ReleaseEvidence::SettlementRemainder
            ))
            .await,
        Err(LedgerError::ReservationState)
    );
    assert_eq!(books.balance(&alice, usdt).await, (48 * UNIT, 52 * UNIT));
    assert_eq!(
        books
            .ledger
            .record_execution(id, ExecutionState::NotSubmitted, "provider:op:1")
            .await,
        Err(LedgerError::ReservationState)
    );
    // Verified settlement consumes the reservation: custody and fee revenue.
    let settle = books.consume(
        id,
        "chain:tx:1",
        vec![
            (books.custody_usdt, amount(50 * UNIT)),
            (books.fee_usdt, amount(2 * UNIT)),
        ],
    );
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

    // Cancelled before submission: released in full.
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
    // Submitted, then definitively not executed: released.
    let failed = books
        .ledger
        .reserve(&books.reserve("withdrawal:3", &alice, usdt, 30 * UNIT))
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
    let partial = books
        .ledger
        .reserve(&books.reserve("withdrawal:4", &alice, usdt, 10 * UNIT))
        .await
        .unwrap();
    let rid = partial.reservation_id;
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
    let movements: i64 = books.count("ledger.reservation_movements").await;
    assert_eq!(movements, 9);
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
    assert_eq!(
        control
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
        control.execute_correction(request, books.checker).await,
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
        control
            .decide_correction(request, books.outsider, true, "No permission")
            .await,
        Err(LedgerError::ApprovalRequired)
    );
    assert_eq!(
        control
            .decide_correction(request, books.checker, true, "Evidence reviewed")
            .await
            .unwrap(),
        "APPROVED"
    );
    let outcome = control
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
    control
        .decide_correction(request, books.checker, true, "Investigated")
        .await
        .unwrap();
    let CorrectionOutcome::RecoveryRequired { recovery_case_id } = control
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
    control
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

#[sqlx::test(migrator = "MIGRATOR")]
async fn failures_and_crashes_leave_no_partial_posting(pool: PgPool) {
    let books = setup(pool).await;
    let alice = books.customer("alice").await;
    let bob = books.customer("bob").await;
    books
        .deposit("dep", &alice, books.usdc, 10 * UNIT)
        .await
        .unwrap();
    let tables = [
        "ledger.journals",
        "ledger.entries",
        "ledger.accounting_events",
        "integrations.outbox_events",
        "ledger.reservations",
    ];
    let mut before = Vec::new();
    for table in tables {
        before.push(books.count(table).await);
    }
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
    for (table, count) in tables.iter().zip(&before) {
        assert_eq!(books.count(table).await, *count, "{table}");
    }
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
    let lift = format!(
        "SELECT ledger.lift_asset_hold('{hold}', 'Investigated', '{}')",
        books.maker
    );
    assert_eq!(
        sqlstate(&books.pool, Some("deypay_ledger_controller"), &lift)
            .await
            .as_deref(),
        Some("LE007")
    );
    let resolve_without_evidence = format!(
        "SELECT operations.update_case('{case}', 'RESOLVED', NULL, 'Explained', NULL, '{}')",
        books.maker
    );
    assert_eq!(
        sqlstate(
            &books.pool,
            Some("deypay_ledger_controller"),
            &resolve_without_evidence
        )
        .await
        .as_deref(),
        Some("LE006")
    );
    let resolve = format!(
        "SELECT operations.update_case('{case}', 'RESOLVED', NULL, 'Funded from treasury', 'treasury:transfer:1', '{}')",
        books.maker
    );
    assert_eq!(
        sqlstate(&books.pool, Some("deypay_ledger_controller"), &resolve).await,
        None
    );
    assert_eq!(
        sqlstate(&books.pool, Some("deypay_ledger_controller"), &lift).await,
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
/// transfers, reservations, settlements and releases always agrees with an
/// independent in-memory model, and every ledger invariant holds afterwards.
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
    let mut open: Vec<(Uuid, usize, usize, u128)> = Vec::new();
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
                    .ledger
                    .reserve(&books.reserve(&format!("res:{step}"), &customers[who], asset, value))
                    .await;
                if available[who][which] >= value {
                    open.push((result.unwrap().reservation_id, who, which, value));
                    available[who][which] -= value;
                    reserved[who][which] += value;
                } else {
                    assert_eq!(result, Err(LedgerError::InsufficientFunds));
                }
            }
            3 if !open.is_empty() => {
                let (id, owner, index, held) = open.swap_remove(next(open.len() as u64) as usize);
                let custody = books.custody(assets[index]);
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
                let (id, owner, index, held) = open.swap_remove(next(open.len() as u64) as usize);
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
