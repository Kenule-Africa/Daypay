use deypay_application::registration::{
    EnrollmentFuture, PasswordEnrollment, Registration, RegistrationError, register,
};
use deypay_infrastructure::{MIGRATOR, registration::PgRegistrationRepository};
use sqlx::PgPool;

// Only the trusted password-enrollment boundary is a fixture. Persistence uses PostgreSQL.
struct FixturePasswords;

impl PasswordEnrollment for FixturePasswords {
    fn enroll<'a>(&'a self, password: &'a str) -> EnrollmentFuture<'a> {
        Box::pin(async move {
            if password != "accepted fixture secret" {
                return Err(RegistrationError::InvalidInput);
            }
            Ok(
                "$argon2id$v=19$m=32,t=2,p=1$cmVnaXN0cmF0aW9uLXRlc3Qtc2FsdA$lKshSWBnLJI671HubV8nU9eEA9jnQS2kWCXyMDNEDyc"
                    .to_owned(),
            )
        })
    }
}

const SANDBOX: &str = include_str!("../../../../infrastructure/postgres/registration-sandbox.psql");
async fn sandbox(pool: &PgPool) -> sqlx::Result<PgRegistrationRepository> {
    sqlx::raw_sql(SANDBOX).execute(pool).await?;
    Ok(PgRegistrationRepository(pool.clone()))
}

fn request<'a>(email: &'a str, username: &'a str) -> Registration<'a> {
    Registration {
        email,
        username,
        phone: "+2348012345678",
        first_name: "Test",
        last_name: "Customer",
        country_code: "NG",
        terms_version: "1.0",
        privacy_version: "1.0",
        accept_terms: true,
        acknowledge_privacy_notice: true,
    }
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn legal_release_activation_and_historical_evidence(pool: PgPool) -> sqlx::Result<()> {
    let repo = sandbox(&pool).await?;
    let old = request("first@example.test", "first");
    register(&repo, &FixturePasswords, &old, "accepted fixture secret")
        .await
        .unwrap();
    // Metadata alone cannot publish the embedded drafts.
    assert!(sqlx::query("INSERT INTO customers.registration_legal_releases(scope,terms_version,privacy_version,wording_version,approval_reference,published_at)
        VALUES ('published','1.0','1.0','1.0','test-only approval',now())").execute(&pool).await.is_err());
    sqlx::raw_sql("INSERT INTO customers.legal_document_revisions(document_id,version,title,content)
         VALUES ('DEP-LEG-001','test-next','Test-only replacement',convert_to('Synthetic test text','UTF8'));
         INSERT INTO customers.registration_legal_releases(scope,terms_version,privacy_version,wording_version)
         VALUES ('sandbox','test-next','1.0','1.0');
         UPDATE customers.current_registration_release SET release_id=(SELECT id FROM customers.registration_legal_releases WHERE terms_version='test-next') WHERE scope='sandbox'").execute(&pool).await?;
    sqlx::raw_sql(SANDBOX).execute(&pool).await?;
    assert_eq!(
        register(
            &repo,
            &FixturePasswords,
            &request("new@example.test", "new"),
            "accepted fixture secret"
        )
        .await,
        Err(RegistrationError::InvalidInput)
    );
    let saved:String=sqlx::query_scalar("SELECT d.terms_version FROM customers.registration_consent_evidence e JOIN customers.registration_legal_releases d ON e.release_id=d.id").fetch_one(&pool).await?;
    assert_eq!(saved, "1.0");
    let legacy=sqlx::query_scalar::<_,bool>("SELECT to_regprocedure('identity.register_customer(text,text,text,text,text,text,text,text,text,bigint)') IS NULL").fetch_one(&pool).await?;
    assert!(legacy);
    Ok(())
}

#[sqlx::test(migrations = false)]
async fn migration_preserves_existing_consent_without_fabricating_evidence(
    pool: PgPool,
) -> sqlx::Result<()> {
    for migration in MIGRATOR.iter().filter(|m| m.version < 202611190001) {
        sqlx::raw_sql(&migration.sql).execute(&pool).await?;
    }
    sqlx::query("SELECT identity.register_customer('legacy@example.test','+2348012345678','legacy',$1,'Legacy','Customer','NG','old_terms','old_privacy',1700000000)")
        .bind(FixturePasswords.enroll("accepted fixture secret").await.unwrap()).execute(&pool).await?;
    let migration = MIGRATOR.iter().find(|m| m.version == 202611190001).unwrap();
    sqlx::raw_sql(&migration.sql).execute(&pool).await?;
    let preserved:bool=sqlx::query_scalar("SELECT terms_version='old_terms' AND privacy_version='old_privacy' AND extract(epoch FROM accepted_at)=1700000000
        AND NOT EXISTS (SELECT FROM customers.registration_consent_evidence) FROM customers.registration_consents").fetch_one(&pool).await?;
    assert!(preserved);
    Ok(())
}

#[sqlx::test(migrations = false)]
async fn mvp_upgrade_preserves_phone_history_and_progresses_only_verified_contacts(
    pool: PgPool,
) -> sqlx::Result<()> {
    let version = 202611200001;
    for migration in MIGRATOR.iter().filter(|m| m.version < version) {
        sqlx::raw_sql(&migration.sql).execute(&pool).await?;
    }
    let repo = sandbox(&pool).await?;
    register(
        &repo,
        &FixturePasswords,
        &request("old@example.test", "old"),
        "accepted fixture secret",
    )
    .await
    .unwrap();
    let mut tx = pool.begin().await?;
    sqlx::raw_sql("UPDATE identity.users SET email_verified_at=now() WHERE email='old@example.test';
        INSERT INTO identity.users(email,status,email_verified_at)
        VALUES ('registered@example.test','REGISTERED',now()), ('risk@example.test','SUSPENDED',now()),
               ('kyc@example.test','KYC_PENDING',now()), ('waiting@example.test','CONTACT_UNVERIFIED',NULL);
        INSERT INTO identity.users(email,status,phone_e164,phone_verified_at)
        VALUES ('phoneonly@example.test','CONTACT_VERIFIED','+2348012349999',now());
        CREATE TEMP TABLE previous_contacts ON COMMIT DROP AS SELECT * FROM identity.users;
        CREATE TEMP TABLE previous_phone_events ON COMMIT DROP AS
            SELECT * FROM integrations.outbox_events WHERE event_type='PHONE_VERIFICATION_REQUESTED';")
        .execute(&mut *tx).await?;
    let mvp = MIGRATOR.iter().find(|m| m.version == version).unwrap();
    sqlx::raw_sql(&mvp.sql).execute(&mut *tx).await?;
    assert!(sqlx::query_scalar::<_,bool>("SELECT bool_and(
        u.email_verified_at IS NOT DISTINCT FROM p.email_verified_at AND
        u.phone_e164 IS NOT DISTINCT FROM p.phone_e164 AND u.phone_verified_at IS NOT DISTINCT FROM p.phone_verified_at AND
        u.status=CASE WHEN p.email IN ('old@example.test','registered@example.test') THEN 'CONTACT_VERIFIED'
                      WHEN p.email='phoneonly@example.test' THEN 'CONTACT_UNVERIFIED' ELSE p.status END)
        FROM previous_contacts p JOIN identity.users u USING(id)").fetch_one(&mut *tx).await?);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM audit.audit_events WHERE action='MVP_CONTACT_STATUS_ALIGNED'"
        )
        .fetch_one(&mut *tx)
        .await?,
        3
    );
    // The second poll runs with only the historical phone intent still pending.
    for _ in 0..2 {
        sqlx::query("SELECT integrations.dispatch_verification_email(5)")
            .execute(&mut *tx)
            .await?;
    }
    assert!(sqlx::query_scalar::<_,bool>("SELECT count(*)=1 AND bool_and(to_jsonb(p)=to_jsonb(e))
        AND NOT EXISTS(SELECT FROM integrations.jobs j JOIN previous_phone_events h ON j.outbox_event_id=h.id)
        FROM previous_phone_events p JOIN integrations.outbox_events e USING(id)").fetch_one(&mut *tx).await?);
    tx.commit().await?;
    let unproven = sqlx::query(
        "UPDATE identity.users SET status='CONTACT_VERIFIED' WHERE email='phoneonly@example.test'",
    )
    .execute(&pool)
    .await
    .unwrap_err();
    assert_eq!(
        unproven.as_database_error().and_then(|e| e.constraint()),
        Some("mvp_contact_verified_requires_email")
    );
    let error = sqlx::query("INSERT INTO integrations.outbox_events(aggregate_type,aggregate_id,event_type,payload)
        SELECT 'CUSTOMER',id,'PHONE_VERIFICATION_REQUESTED','{}' FROM identity.users WHERE email='old@example.test'")
        .execute(&pool).await.unwrap_err();
    assert_eq!(
        error.as_database_error().and_then(|e| e.constraint()),
        Some("mvp_phone_verification_deferred")
    );
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn restricted_runtime_can_register_without_access_to_secrets(
    pool: PgPool,
) -> sqlx::Result<()> {
    sqlx::raw_sql(include_str!(
        "../../../../infrastructure/postgres/phase-two-grants.psql"
    ))
    .execute(&pool)
    .await?;
    sqlx::raw_sql(include_str!(
        "../../../../infrastructure/postgres/registration-grants.psql"
    ))
    .execute(&pool)
    .await?;
    sqlx::raw_sql(SANDBOX).execute(&pool).await?;
    let has_access: bool = sqlx::query_scalar(
        "SELECT has_table_privilege('deypay_app','identity.password_credentials','SELECT')
             OR has_table_privilege('deypay_app','customers.registration_consents','UPDATE')
             OR has_table_privilege('deypay_app','identity.users','INSERT')
             OR has_table_privilege('deypay_app','customers.registration_consent_evidence','UPDATE')
             OR has_table_privilege('deypay_app','customers.legal_document_revisions','UPDATE')
             OR has_table_privilege('deypay_app','customers.current_registration_release','UPDATE')",
    )
    .fetch_one(&pool)
    .await?;
    assert!(!has_access);
    let mut tx = pool.begin().await?;
    sqlx::query("SET LOCAL ROLE deypay_app")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SELECT identity.register_customer('runtime@example.test','+2348012345678','runtime',$1,'Test','Customer','NG','1.0','1.0',true,true)").bind(FixturePasswords.enroll("accepted fixture secret").await.unwrap()).execute(&mut *tx).await?;
    tx.commit().await?;
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM identity.users")
            .fetch_one(&pool)
            .await?,
        "CONTACT_UNVERIFIED"
    );
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn registration_is_atomic_and_replay_safe(pool: PgPool) -> sqlx::Result<()> {
    let repo = sandbox(&pool).await?;
    let input = request(" Person@Example.Test ", "@Sample");
    for _ in 0..2 {
        register(&repo, &FixturePasswords, &input, "accepted fixture secret")
            .await
            .unwrap();
    }
    let created: (String, String, bool, bool) = sqlx::query_as(
        "SELECT email,status,email_verified_at IS NULL,phone_verified_at IS NULL FROM identity.users"
    ).fetch_one(&pool).await?;
    assert_eq!(
        created,
        (
            "person@example.test".into(),
            "CONTACT_UNVERIFIED".into(),
            true,
            true
        )
    );
    let counts: (i64,i64,i64,i64,i64,i64,i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM customers.profiles),(SELECT count(*) FROM customers.usernames),
                (SELECT count(*) FROM identity.password_credentials),(SELECT count(*) FROM customers.registration_consents),
                (SELECT count(*) FROM audit.audit_events),(SELECT count(*) FROM integrations.outbox_events),
                (SELECT count(*) FROM identity.sessions)+(SELECT count(*) FROM wallets.wallet_profiles)+(SELECT count(*) FROM payments.transactions)"
    ).fetch_one(&pool).await?;
    assert_eq!(counts, (1, 1, 1, 1, 1, 1, 0));
    let notices: (String, String) =
        sqlx::query_as("SELECT terms_version,privacy_version FROM customers.registration_consents")
            .fetch_one(&pool)
            .await?;
    assert_eq!(notices, ("1.0".into(), "1.0".into()));
    let valid:bool=sqlx::query_scalar("SELECT
        e.terms_accepted AND e.privacy_acknowledged AND e.channel='REGISTRATION_API'
        AND e.request_id=a.correlation_id AND e.event_id IS NOT NULL
        AND c.accepted_at BETWEEN u.created_at AND c.recorded_at
        AND c.recorded_at <= clock_timestamp() AND r.scope='sandbox'
        AND r.terms_id='DEP-LEG-001' AND r.privacy_id='DEP-LEG-002' AND r.wording_id='DEP-LEG-003'
        AND r.wording_version='1.0' AND p.first_name='Test' AND p.last_name='Customer' AND p.country_code='NG'
        FROM customers.registration_consent_evidence e JOIN customers.registration_consents c USING(user_id)
        JOIN customers.registration_legal_releases r ON r.id=e.release_id
        JOIN identity.users u ON u.id=e.user_id JOIN customers.profiles p USING(user_id)
        JOIN audit.audit_events a ON a.resource_id=u.id").fetch_one(&pool).await?;
    assert!(valid);
    for sql in [
        "DELETE FROM customers.registration_consent_evidence",
        "UPDATE customers.legal_document_revisions SET title=title",
        "TRUNCATE customers.registration_consent_evidence",
        "UPDATE customers.registration_legal_releases SET wording_version='changed'",
    ] {
        assert!(sqlx::query(sql).execute(&pool).await.is_err());
    }
    assert!(
        sqlx::query("DELETE FROM customers.registration_consents")
            .execute(&pool)
            .await
            .is_err()
    );
    sqlx::query("UPDATE identity.users SET phone_verified_at=now()")
        .execute(&pool)
        .await?;
    register(
        &repo,
        &FixturePasswords,
        &request("other@example.test", "other"),
        "accepted fixture secret",
    )
    .await
    .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM identity.users")
            .fetch_one(&pool)
            .await?,
        1
    );
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn simultaneous_normalized_email_replays_enqueue_one_email(pool: PgPool) -> sqlx::Result<()> {
    let repo = sandbox(&pool).await?;
    let one = request("One@Example.Test", "first");
    let two = request(" one@example.test ", "second");
    let secret = "accepted fixture secret";
    let (a, b) = tokio::join!(
        register(&repo, &FixturePasswords, &one, secret),
        register(&repo, &FixturePasswords, &two, secret)
    );
    assert_eq!((a, b), (Ok(()), Ok(())));
    let counts:(i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM identity.users),
        (SELECT count(*) FROM customers.usernames),(SELECT count(*) FROM integrations.outbox_events)").fetch_one(&pool).await?;
    assert_eq!(counts, (1, 1, 1));
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn competing_handles_claim_once_and_reserved_names_leave_no_identity(
    pool: PgPool,
) -> sqlx::Result<()> {
    let repo = sandbox(&pool).await?;
    let invalid_key =
        sqlx::query("INSERT INTO customers.username_keys VALUES ('@Reserved-Fixture',true)")
            .execute(&pool)
            .await
            .unwrap_err();
    assert_eq!(
        invalid_key.as_database_error().and_then(|e| e.constraint()),
        Some("username_keys_canonical")
    );
    sqlx::query("INSERT INTO customers.username_keys VALUES ('reserved-fixture',true)")
        .execute(&pool)
        .await?;
    let secret = "accepted fixture secret";
    register(
        &repo,
        &FixturePasswords,
        &request("reserved@example.test", "@Reserved-Fixture"),
        secret,
    )
    .await
    .unwrap();
    let first = request("first@example.test", "@MiXeD");
    let second = request("second@example.test", "mixed");
    let (a, b) = tokio::join!(
        register(&repo, &FixturePasswords, &first, secret),
        register(&repo, &FixturePasswords, &second, secret)
    );
    assert_eq!(a, Ok(()));
    assert_eq!(b, Ok(()));
    let accounts: i64 = sqlx::query_scalar("SELECT count(*) FROM identity.users")
        .fetch_one(&pool)
        .await?;
    assert_eq!(accounts, 1);
    let handles: Vec<String> =
        sqlx::query_scalar("SELECT normalized_username FROM customers.usernames")
            .fetch_all(&pool)
            .await?;
    assert_eq!(handles, ["mixed"]);
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn invalid_requests_and_persistence_failures_leave_no_partial_identity(
    pool: PgPool,
) -> sqlx::Result<()> {
    let repo = sandbox(&pool).await?;
    let mut input = request("person@example.test", "person");
    let secret = "accepted fixture secret";
    input.phone = "08012345678";
    assert_eq!(
        register(&repo, &FixturePasswords, &input, secret).await,
        Err(RegistrationError::InvalidInput)
    );
    input.phone = "+2348012345678";
    for bad in ["", "old"] {
        input.terms_version = bad;
        assert_eq!(
            register(&repo, &FixturePasswords, &input, secret).await,
            Err(RegistrationError::InvalidInput)
        );
    }
    input.terms_version = "1.0";
    input.privacy_version = "old";
    assert_eq!(
        register(&repo, &FixturePasswords, &input, secret).await,
        Err(RegistrationError::InvalidInput)
    );
    input.privacy_version = "1.0";
    assert_eq!(
        register(&repo, &FixturePasswords, &input, "weak").await,
        Err(RegistrationError::InvalidInput)
    );
    sqlx::query("ALTER TABLE integrations.outbox_events ADD CHECK (event_type <> 'EMAIL_VERIFICATION_REQUESTED')").execute(&pool).await?;
    assert_eq!(
        register(&repo, &FixturePasswords, &input, secret).await,
        Err(RegistrationError::Unavailable)
    );
    let rows: i64 = sqlx::query_scalar("SELECT (SELECT count(*) FROM identity.users)+(SELECT count(*) FROM integrations.outbox_events)+(SELECT count(*) FROM audit.audit_events)+(SELECT count(*) FROM customers.registration_consent_evidence)").fetch_one(&pool).await?;
    assert_eq!(rows, 0);
    Ok(())
}
