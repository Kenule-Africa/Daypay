use deypay_application::registration::{
    EnrollmentFuture, NoticeVersions, PasswordEnrollment, Registration, RegistrationError, register,
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

const NOTICES: NoticeVersions<'static> = NoticeVersions {
    terms: "terms-fixture",
    privacy: "privacy-fixture",
};

fn request<'a>(email: &'a str, username: &'a str) -> Registration<'a> {
    Registration {
        email,
        username,
        phone: "+2348012345678",
        first_name: "Test",
        last_name: "Customer",
        country_code: "NG",
        terms_version: NOTICES.terms,
        privacy_version: NOTICES.privacy,
        accepted_at_unix: 1700000000,
    }
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
    let has_access: bool = sqlx::query_scalar(
        "SELECT has_table_privilege('deypay_app','identity.password_credentials','SELECT')
             OR has_table_privilege('deypay_app','customers.registration_consents','UPDATE')
             OR has_table_privilege('deypay_app','identity.users','INSERT')",
    )
    .fetch_one(&pool)
    .await?;
    assert!(!has_access);
    let mut tx = pool.begin().await?;
    sqlx::query("SET LOCAL ROLE deypay_app")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SELECT identity.register_customer('runtime@example.test','+2348012345678','runtime',$1,'Test','Customer','NG','fixture-terms','fixture-privacy',1700000000)").bind(FixturePasswords.enroll("accepted fixture secret").await.unwrap()).execute(&mut *tx).await?;
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
    let repo = PgRegistrationRepository(pool.clone());
    let input = request(" Person@Example.Test ", "@Sample");
    for _ in 0..2 {
        register(
            &repo,
            &FixturePasswords,
            &NOTICES,
            &input,
            "accepted fixture secret",
        )
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
    assert_eq!(counts, (1, 1, 1, 1, 1, 2, 0));
    let notices: (String, String) =
        sqlx::query_as("SELECT terms_version,privacy_version FROM customers.registration_consents")
            .fetch_one(&pool)
            .await?;
    assert_eq!(notices, (NOTICES.terms.into(), NOTICES.privacy.into()));
    let accepted: i64 = sqlx::query_scalar(
        "SELECT extract(epoch FROM accepted_at)::bigint FROM customers.registration_consents",
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(accepted, input.accepted_at_unix);
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
        &NOTICES,
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
async fn simultaneous_normalized_email_replays_enqueue_one_pair(pool: PgPool) -> sqlx::Result<()> {
    let repo = PgRegistrationRepository(pool.clone());
    let one = request("One@Example.Test", "first");
    let two = request(" one@example.test ", "second");
    let secret = "accepted fixture secret";
    let (a, b) = tokio::join!(
        register(&repo, &FixturePasswords, &NOTICES, &one, secret),
        register(&repo, &FixturePasswords, &NOTICES, &two, secret)
    );
    assert_eq!((a, b), (Ok(()), Ok(())));
    let counts:(i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM identity.users),
        (SELECT count(*) FROM customers.usernames),(SELECT count(*) FROM integrations.outbox_events)").fetch_one(&pool).await?;
    assert_eq!(counts, (1, 1, 2));
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn competing_handles_claim_once_and_reserved_names_leave_no_identity(
    pool: PgPool,
) -> sqlx::Result<()> {
    let repo = PgRegistrationRepository(pool.clone());
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
        &NOTICES,
        &request("reserved@example.test", "@Reserved-Fixture"),
        secret,
    )
    .await
    .unwrap();
    let first = request("first@example.test", "@MiXeD");
    let second = request("second@example.test", "mixed");
    let (a, b) = tokio::join!(
        register(&repo, &FixturePasswords, &NOTICES, &first, secret),
        register(&repo, &FixturePasswords, &NOTICES, &second, secret)
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
    let repo = PgRegistrationRepository(pool.clone());
    let mut input = request("person@example.test", "person");
    let secret = "accepted fixture secret";
    input.phone = "08012345678";
    assert_eq!(
        register(&repo, &FixturePasswords, &NOTICES, &input, secret).await,
        Err(RegistrationError::InvalidInput)
    );
    input.phone = "+2348012345678";
    for bad in ["", "old"] {
        input.terms_version = bad;
        assert_eq!(
            register(&repo, &FixturePasswords, &NOTICES, &input, secret).await,
            Err(RegistrationError::InvalidInput)
        );
    }
    input.terms_version = NOTICES.terms;
    input.privacy_version = "old";
    assert_eq!(
        register(&repo, &FixturePasswords, &NOTICES, &input, secret).await,
        Err(RegistrationError::InvalidInput)
    );
    input.privacy_version = NOTICES.privacy;
    assert_eq!(
        register(&repo, &FixturePasswords, &NOTICES, &input, "weak").await,
        Err(RegistrationError::InvalidInput)
    );
    sqlx::query("ALTER TABLE integrations.outbox_events ADD CHECK (event_type <> 'PHONE_VERIFICATION_REQUESTED')").execute(&pool).await?;
    assert_eq!(
        register(&repo, &FixturePasswords, &NOTICES, &input, secret).await,
        Err(RegistrationError::Unavailable)
    );
    let rows: i64 = sqlx::query_scalar("SELECT (SELECT count(*) FROM identity.users)+(SELECT count(*) FROM integrations.outbox_events)+(SELECT count(*) FROM audit.audit_events)").fetch_one(&pool).await?;
    assert_eq!(rows, 0);
    Ok(())
}
