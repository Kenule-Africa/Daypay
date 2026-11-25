//! Database evidence for the customer profile operations (IMP-P3-010).
//! Statements run as the restricted runtime role unless a test inspects or
//! arranges state as the schema owner.
use deypay_application::profile::{
    AccountStatus, ImageRemoval, NormalizedImage, Outcome, PreferenceChange, ProfileError,
    ProfileImageStore, ProfileStore,
};
use deypay_infrastructure::{MIGRATOR, profiles::PgProfiles};
use sqlx::{PgPool, postgres::PgPoolOptions};
use uuid::Uuid;

const OPERATIONS: [&str; 7] = [
    "customers.customer_preferences(uuid)",
    "customers.customer_profile(uuid)",
    "customers.profile_image(uuid,uuid)",
    "customers.remove_profile_image(uuid)",
    "customers.rename_customer(uuid,text,text)",
    "customers.store_profile_image(uuid,bytea,integer,integer)",
    "customers.update_customer_preferences(uuid,boolean,boolean,boolean,boolean)",
];
const HELPERS: [&str; 5] = [
    "customers.lock_profile_image(uuid)",
    "customers.lock_profile_owner(uuid)",
    "customers.names_editable(uuid,text)",
    "customers.utc_text(timestamp with time zone)",
    "customers.valid_profile_name(text)",
];

async fn runtime(pool: &PgPool) -> sqlx::Result<PgPool> {
    for script in [
        include_str!("../../../../../infrastructure/postgres/phase-two-grants.psql"),
        include_str!("../../../../../infrastructure/postgres/profile-grants.psql"),
        include_str!("../../../../../infrastructure/postgres/registration-sandbox.psql"),
    ] {
        sqlx::raw_sql(script).execute(pool).await?;
    }
    PgPoolOptions::new()
        .max_connections(8)
        .after_connect(|conn, _| {
            Box::pin(async move {
                sqlx::query("SET ROLE deypay_app").execute(conn).await?;
                Ok(())
            })
        })
        .connect_with((*pool.connect_options()).clone())
        .await
}

async fn customer(pool: &PgPool, name: &str) -> sqlx::Result<Uuid> {
    let email = format!("{name}@example.test");
    sqlx::query("SELECT identity.register_customer($1,'+2348012345678',$2,'$argon2id$fixture','Ada','Okafor','NG','1.0','1.0',true,true)")
        .bind(&email).bind(name).execute(pool).await?;
    sqlx::query_scalar(
        "UPDATE identity.users SET email_verified_at=clock_timestamp() WHERE email=$1 RETURNING id",
    )
    .bind(email)
    .fetch_one(pool)
    .await
}

fn jpeg(fill: u8) -> NormalizedImage {
    let mut bytes = vec![0xff, 0xd8, 0xff, 0xe0, fill, fill];
    bytes.extend([0xff, 0xd9]);
    NormalizedImage::new(bytes, 16, 8).unwrap()
}

fn sql_state(error: &sqlx::Error) -> Option<String> {
    error
        .as_database_error()
        .and_then(|e| e.code())
        .map(|c| c.into_owned())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn operations_are_pinned_definers_executable_only_by_the_runtime(
    pool: PgPool,
) -> sqlx::Result<()> {
    runtime(&pool).await?;
    let definers: Vec<(String, Vec<String>, String)> = sqlx::query_as(
        "SELECT p.oid::regprocedure::text, coalesce(p.proconfig, '{}'), pg_get_userbyid(p.proowner)
         FROM pg_proc p WHERE p.pronamespace = 'customers'::regnamespace AND p.prosecdef ORDER BY 1",
    )
    .fetch_all(&pool)
    .await?;
    let names: Vec<&str> = definers.iter().map(|d| d.0.as_str()).collect();
    assert_eq!(names, OPERATIONS, "no other privileged customer operation");
    for (name, config, owner) in &definers {
        assert_eq!(config, &["search_path=pg_catalog, pg_temp"], "{name}");
        assert_ne!(owner, "deypay_app");
    }
    for function in OPERATIONS.iter().chain(&HELPERS) {
        let (runtime, public): (bool, bool) = sqlx::query_as(
            "SELECT has_function_privilege('deypay_app', $1, 'EXECUTE'),
                    has_function_privilege('public', $1, 'EXECUTE')",
        )
        .bind(function)
        .fetch_one(&pool)
        .await?;
        assert_eq!(
            (runtime, public),
            (OPERATIONS.contains(function), false),
            "{function}"
        );
    }
    // Helpers keep a pinned path too, although only definers can reach them.
    let unpinned: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pg_proc p WHERE p.pronamespace = 'customers'::regnamespace
         AND p.oid::regprocedure::text = ANY($1)
         AND NOT coalesce(p.proconfig, '{}') @> ARRAY['search_path=pg_catalog, pg_temp']",
    )
    .bind(HELPERS.as_slice())
    .fetch_one(&pool)
    .await?;
    assert_eq!(unpinned, 0);
    // Applying the authentication grants afterwards still passes their safety check.
    sqlx::raw_sql(include_str!(
        "../../../../../infrastructure/postgres/auth-grants.psql"
    ))
    .execute(&pool)
    .await?;
    assert!(
        sqlx::query_scalar::<_, bool>("SELECT identity.auth_acl_ok()")
            .fetch_one(&pool)
            .await?
    );
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn the_runtime_reaches_profile_rows_only_through_the_operations(
    pool: PgPool,
) -> sqlx::Result<()> {
    let runtime = runtime(&pool).await?;
    let ada = customer(&pool, "ada").await?;
    for statement in [
        "SELECT first_name FROM customers.profiles",
        "SELECT email FROM identity.users",
        "SELECT phone_e164 FROM identity.users",
        "SELECT * FROM customers.preferences",
        "SELECT content FROM customers.profile_images",
        "UPDATE customers.profiles SET first_name = 'X'",
        "UPDATE customers.usernames SET username = 'other'",
        "INSERT INTO customers.preferences(user_id) VALUES (gen_random_uuid())",
        "DELETE FROM customers.profile_images",
        "UPDATE identity.users SET status = 'KYC_APPROVED'",
        "SELECT customers.lock_profile_owner(gen_random_uuid())",
        "SELECT customers.names_editable(gen_random_uuid(), 'CONTACT_VERIFIED')",
        "CREATE FUNCTION customers.shadow() RETURNS int LANGUAGE sql AS 'SELECT 1'",
    ] {
        let error = sqlx::query(statement).execute(&runtime).await.unwrap_err();
        assert_eq!(sql_state(&error).as_deref(), Some("42501"), "{statement}");
    }
    let store = PgProfiles(runtime);
    let profile = store.profile(ada).await.unwrap().unwrap();
    assert_eq!(profile.account_status, AccountStatus::ContactVerified);
    assert_eq!(
        (profile.first_name.as_str(), profile.username.as_deref()),
        ("Ada", Some("ada"))
    );
    assert!(store.profile(Uuid::nil()).await.unwrap().is_none());
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn writes_are_confined_to_the_given_customer_and_an_eligible_account(
    pool: PgPool,
) -> sqlx::Result<()> {
    let store = PgProfiles(runtime(&pool).await?);
    let ada = customer(&pool, "ada").await?;
    let bob = customer(&pool, "bob").await?;
    assert_eq!(
        store.rename(ada, Some("Adaeze"), None).await,
        Ok(Outcome::Updated)
    );
    store.store_image(ada, &jpeg(1)).await.unwrap().unwrap();
    let change = PreferenceChange {
        push_notifications: Some(true),
        ..Default::default()
    };
    assert_eq!(
        store.update_preferences(ada, change).await,
        Ok(Outcome::Updated)
    );
    // Bob's operations never reach Ada's rows.
    assert_eq!(store.remove_image(bob).await, Ok(ImageRemoval::Absent));
    assert!(store.image(bob, None).await.unwrap().is_none());
    assert!(store.image(ada, None).await.unwrap().is_some());
    assert_eq!(store.profile(bob).await.unwrap().unwrap().first_name, "Ada");
    assert!(
        !store
            .preferences(bob)
            .await
            .unwrap()
            .unwrap()
            .0
            .push_notifications
    );
    // Unknown, unverified, suspended and closed accounts cannot write.
    let unverified: Uuid = sqlx::query_scalar(
        "INSERT INTO identity.users(email,status) VALUES ('new@example.test','CONTACT_UNVERIFIED') RETURNING id",
    )
    .fetch_one(&pool)
    .await?;
    // A session-eligible status without verified email is still refused.
    let unproven: Uuid = sqlx::query_scalar(
        "INSERT INTO identity.users(email,status) VALUES ('kyc@example.test','KYC_PENDING') RETURNING id",
    )
    .fetch_one(&pool)
    .await?;
    sqlx::query("INSERT INTO customers.profiles(user_id,first_name,last_name,country_code) VALUES ($1,'Kay','Why','NG')")
        .bind(unproven)
        .execute(&pool)
        .await?;
    sqlx::query("UPDATE identity.users SET status='SUSPENDED' WHERE id=$1")
        .bind(bob)
        .execute(&pool)
        .await?;
    for target in [Uuid::nil(), unverified, unproven, bob] {
        assert_eq!(
            store.rename(target, Some("X"), None).await,
            Ok(Outcome::Ineligible)
        );
        assert_eq!(
            store.update_preferences(target, change).await,
            Ok(Outcome::Ineligible)
        );
        assert_eq!(store.store_image(target, &jpeg(2)).await, Ok(None));
        assert_eq!(
            store.remove_image(target).await,
            Ok(ImageRemoval::Ineligible)
        );
    }
    // KYC_PENDING may still manage its image and preferences, but names are locked.
    sqlx::query("UPDATE identity.users SET status='KYC_PENDING' WHERE id=$1")
        .bind(ada)
        .execute(&pool)
        .await?;
    assert_eq!(
        store.rename(ada, Some("Other"), None).await,
        Ok(Outcome::NamesLocked)
    );
    assert!(store.store_image(ada, &jpeg(3)).await.unwrap().is_some());
    let audit: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT actor_id, action FROM audit.audit_events WHERE action LIKE 'PROFILE%' OR action='PREFERENCES_CHANGED' ORDER BY created_at",
    )
    .fetch_all(&pool)
    .await?;
    assert_eq!(
        audit,
        [
            (ada, "PROFILE_NAME_CHANGED".to_owned()),
            (ada, "PROFILE_IMAGE_STORED".to_owned()),
            (ada, "PREFERENCES_CHANGED".to_owned()),
            (ada, "PROFILE_IMAGE_STORED".to_owned()),
        ]
    );
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn the_database_refuses_values_the_api_would_refuse(pool: PgPool) -> sqlx::Result<()> {
    let runtime = runtime(&pool).await?;
    let ada = customer(&pool, "ada").await?;
    for (first, last) in [
        (None, None),
        (Some(""), None),
        (Some(" Ada"), None),
        (None, Some("A\u{200b}da")),
        (None, Some("\u{202e}Ada")),
        (Some("Ada\tO"), None),
    ] {
        let error = sqlx::query("SELECT customers.rename_customer($1,$2,$3)")
            .bind(ada)
            .bind(first)
            .bind(last)
            .execute(&runtime)
            .await
            .unwrap_err();
        assert_eq!(
            sql_state(&error).as_deref(),
            Some("22023"),
            "{first:?} {last:?}"
        );
    }
    let long = "a".repeat(101);
    let error = sqlx::query("SELECT customers.rename_customer($1,$2,NULL)")
        .bind(ada)
        .bind(long)
        .execute(&runtime)
        .await
        .unwrap_err();
    assert_eq!(sql_state(&error).as_deref(), Some("22023"));
    let error = sqlx::query("SELECT customers.update_customer_preferences($1,NULL,NULL,NULL,NULL)")
        .bind(ada)
        .execute(&runtime)
        .await
        .unwrap_err();
    assert_eq!(sql_state(&error).as_deref(), Some("22023"));
    // Only bounded JPEG output can be stored.
    for (bytes, width, height) in [
        (b"\x89PNG\r\n\x1a\n\xff\xd9".to_vec(), 8, 8),
        (b"\xff\xd8\xff<script>".to_vec(), 8, 8),
        (
            [
                &b"\xff\xd8\xff"[..],
                &vec![0; 153_600][..],
                &b"\xff\xd9"[..],
            ]
            .concat(),
            8,
            8,
        ),
        (b"\xff\xd8\xff\xff\xd9".to_vec(), 513, 8),
        (b"\xff\xd8\xff\xff\xd9".to_vec(), 8, 0),
    ] {
        let error = sqlx::query("SELECT * FROM customers.store_profile_image($1,$2,$3,$4)")
            .bind(ada)
            .bind(bytes)
            .bind(width)
            .bind(height)
            .execute(&runtime)
            .await
            .unwrap_err();
        assert_eq!(sql_state(&error).as_deref(), Some("23514"));
    }
    let store = PgProfiles(runtime);
    assert_eq!(
        store.rename(ada, Some(""), None).await,
        Err(ProfileError::Rejected)
    );
    let rows: i64 = sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM customers.profile_images) + (SELECT count(*) FROM customers.preferences)
              + (SELECT count(*) FROM audit.audit_events WHERE action <> 'CUSTOMER_REGISTERED')",
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(rows, 0);
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn concurrent_replacements_keep_one_image_and_a_complete_history(
    pool: PgPool,
) -> sqlx::Result<()> {
    let runtime = runtime(&pool).await?;
    let ada = customer(&pool, "ada").await?;
    let mut writers = tokio::task::JoinSet::new();
    for fill in 0..10_u8 {
        let store = PgProfiles(runtime.clone());
        writers.spawn(async move {
            if fill % 3 == 2 {
                store.remove_image(ada).await.map(|_| ())
            } else {
                store.store_image(ada, &jpeg(fill)).await.map(|_| ())
            }
        });
    }
    while let Some(result) = writers.join_next().await {
        assert_eq!(result.unwrap(), Ok(()));
    }
    let images: i64 = sqlx::query_scalar("SELECT count(*) FROM customers.profile_images")
        .fetch_one(&pool)
        .await?;
    assert!(images <= 1);
    // Serialized writers supersede or remove each version at most once, and
    // the only version never superseded is the stored one.
    let stored: Vec<(Uuid, Option<Uuid>)> = sqlx::query_as(
        "SELECT resource_id, (metadata->>'replaced_version')::uuid FROM audit.audit_events
         WHERE action='PROFILE_IMAGE_STORED'",
    )
    .fetch_all(&pool)
    .await?;
    let removed: Vec<Uuid> = sqlx::query_scalar(
        "SELECT resource_id FROM audit.audit_events WHERE action='PROFILE_IMAGE_REMOVED'",
    )
    .fetch_all(&pool)
    .await?;
    let versions: Vec<Uuid> = stored.iter().map(|s| s.0).collect();
    let mut ended: Vec<Uuid> = stored.iter().filter_map(|s| s.1).chain(removed).collect();
    let count = ended.len();
    ended.sort_unstable();
    ended.dedup();
    assert_eq!(ended.len(), count, "a version was superseded twice");
    assert!(ended.iter().all(|v| versions.contains(v)));
    let live: Vec<Uuid> = versions
        .into_iter()
        .filter(|v| !ended.contains(v))
        .collect();
    let current: Vec<Uuid> = sqlx::query_scalar("SELECT version FROM customers.profile_images")
        .fetch_all(&pool)
        .await?;
    assert_eq!(live, current);
    Ok(())
}

/// Two replacements that start while the current image row is locked must
/// still record a single chain: the second writer sees the first's version.
#[sqlx::test(migrator = "MIGRATOR")]
async fn a_waiting_replacement_supersedes_the_version_committed_before_it(
    pool: PgPool,
) -> sqlx::Result<()> {
    let runtime = runtime(&pool).await?;
    let ada = customer(&pool, "ada").await?;
    let store = PgProfiles(runtime.clone());
    let first = store.store_image(ada, &jpeg(1)).await.unwrap().unwrap();
    let mut holder = pool.begin().await?;
    sqlx::query("SELECT 1 FROM customers.profile_images WHERE user_id=$1 FOR UPDATE")
        .bind(ada)
        .execute(&mut *holder)
        .await?;
    let mut writers = tokio::task::JoinSet::new();
    for fill in [2_u8, 3] {
        let store = PgProfiles(runtime.clone());
        writers.spawn(async move { store.store_image(ada, &jpeg(fill)).await });
    }
    // Release the row only after both writers are blocked on a lock.
    let database: String = sqlx::query_scalar("SELECT current_database()")
        .fetch_one(&pool)
        .await?;
    loop {
        let waiting: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM pg_stat_activity WHERE datname=$1 AND wait_event_type='Lock'",
        )
        .bind(&database)
        .fetch_one(&pool)
        .await?;
        if waiting >= 2 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    holder.commit().await?;
    while let Some(result) = writers.join_next().await {
        assert!(result.unwrap().unwrap().is_some());
    }
    let replaced: Vec<Option<Uuid>> = sqlx::query_scalar(
        "SELECT (metadata->>'replaced_version')::uuid FROM audit.audit_events
         WHERE action='PROFILE_IMAGE_STORED' ORDER BY (metadata->>'replaced_version') IS NOT NULL",
    )
    .fetch_all(&pool)
    .await?;
    assert_eq!(replaced.len(), 3);
    assert_eq!(replaced[0], None);
    assert!(replaced.contains(&Some(first.version)));
    let mut superseded: Vec<Uuid> = replaced.into_iter().flatten().collect();
    superseded.sort_unstable();
    superseded.dedup();
    assert_eq!(superseded.len(), 2, "each version is superseded once");
    Ok(())
}
