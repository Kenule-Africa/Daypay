//! Security evidence for the KYC case database operations (IMP-P4-005
//! prerequisite review). Statements run as the restricted runtime role unless a
//! test inspects the catalog as the schema owner.
use super::support::{customer, runtime, sql_state};
use deypay_infrastructure::MIGRATOR;
use sqlx::{PgPool, postgres::PgPoolOptions};
use uuid::Uuid;

const FUNCTIONS: [&str; 2] = [
    "compliance.open_first_kyc_case(uuid)",
    "compliance.submit_kyc_case(uuid,uuid,text)",
];

async fn open(runtime: &PgPool, owner: Uuid) -> sqlx::Result<Option<Uuid>> {
    sqlx::query_scalar("SELECT case_id FROM compliance.open_first_kyc_case($1)")
        .bind(owner)
        .fetch_optional(runtime)
        .await
}

async fn submit(runtime: &PgPool, owner: Uuid, case_id: Uuid, from: &str) -> sqlx::Result<bool> {
    sqlx::query_scalar("SELECT compliance.submit_kyc_case($1, $2, $3)")
        .bind(owner)
        .bind(case_id)
        .bind(from)
        .fetch_one(runtime)
        .await
}

async fn audit_count(pool: &PgPool) -> sqlx::Result<i64> {
    sqlx::query_scalar("SELECT count(*) FROM audit.audit_events WHERE resource_type = 'KYC_CASE'")
        .fetch_one(pool)
        .await
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn definer_functions_are_pinned_and_executable_only_by_the_runtime(
    pool: PgPool,
) -> sqlx::Result<()> {
    runtime(&pool).await?;
    let rows: Vec<(String, bool, Vec<String>, String, Vec<String>)> = sqlx::query_as(
        "SELECT p.oid::regprocedure::text, p.prosecdef, coalesce(p.proconfig, '{}'),
            pg_get_userbyid(p.proowner),
            ARRAY(SELECT CASE a.grantee WHEN 0 THEN 'PUBLIC' ELSE pg_get_userbyid(a.grantee) END
                  FROM aclexplode(p.proacl) a WHERE a.privilege_type = 'EXECUTE' ORDER BY 1)
        FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace
        WHERE n.nspname = 'compliance' AND p.prosecdef ORDER BY 1",
    )
    .fetch_all(&pool)
    .await?;
    // Exactly these two privileged functions exist in the compliance schema.
    let names: Vec<&str> = rows.iter().map(|r| r.0.as_str()).collect();
    assert_eq!(names, FUNCTIONS);
    for (name, definer, config, owner, grantees) in rows {
        assert!(definer, "{name}");
        // A fixed path with pg_temp last: no caller-created object can shadow
        // a built-in or the schema-qualified tables the functions use.
        assert_eq!(config, ["search_path=pg_catalog, pg_temp"], "{name}");
        let mut expected = vec!["deypay_app".to_owned(), owner.clone()];
        expected.sort();
        assert_eq!(grantees, expected, "{name}: no PUBLIC or other grantee");
        // The runtime must not own or be able to redefine the function.
        assert_ne!(owner, "deypay_app");
    }
    for function in FUNCTIONS {
        for role in ["public", "pg_read_all_settings", "pg_monitor"] {
            let allowed: bool =
                sqlx::query_scalar("SELECT has_function_privilege($1, $2, 'EXECUTE')")
                    .bind(role)
                    .bind(function)
                    .fetch_one(&pool)
                    .await?;
            assert!(!allowed, "{role} may execute {function}");
        }
    }
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn the_runtime_cannot_touch_reviewer_provider_or_document_evidence(
    pool: PgPool,
) -> sqlx::Result<()> {
    let runtime = runtime(&pool).await?;
    let owner = customer(&pool, "kyc-evidence@example.test", "REGISTERED").await?;
    open(&runtime, owner).await?.unwrap();
    for statement in [
        "UPDATE compliance.kyc_cases SET reviewer_id = $1 WHERE user_id = $1",
        "UPDATE compliance.kyc_cases SET rejection_reason_code = 'X' WHERE user_id = $1",
        "UPDATE compliance.kyc_cases SET reviewed_at = now() WHERE user_id = $1",
        "UPDATE compliance.kyc_cases SET risk_tier = 'LOW' WHERE user_id = $1",
        "UPDATE compliance.kyc_cases SET submitted_at = now() WHERE user_id = $1",
        "SELECT reviewer_id FROM compliance.kyc_cases WHERE user_id = $1",
        "SELECT rejection_reason_code FROM compliance.kyc_cases WHERE user_id = $1",
        "SELECT risk_tier FROM compliance.kyc_cases WHERE user_id = $1",
        "SELECT kyc_case_id FROM compliance.verification_results WHERE kyc_case_id = $1",
        "SELECT storage_reference FROM compliance.kyc_documents WHERE kyc_case_id = $1",
        "INSERT INTO compliance.kyc_documents(kyc_case_id,document_type,storage_reference,verification_status)
            VALUES ($1,'X','Y','Z')",
        "UPDATE audit.audit_events SET action = 'X' WHERE actor_id = $1",
        "DELETE FROM audit.audit_events WHERE actor_id = $1",
    ] {
        let error = sqlx::query(statement)
            .bind(owner)
            .execute(&runtime)
            .await
            .unwrap_err();
        assert_eq!(sql_state(&error).as_deref(), Some("42501"), "{statement}");
    }
    // Nor can it redefine a privileged operation.
    let error = sqlx::query(
        "CREATE OR REPLACE FUNCTION compliance.submit_kyc_case(uuid,uuid,text) RETURNS boolean
            LANGUAGE sql AS 'SELECT true'",
    )
    .execute(&runtime)
    .await
    .unwrap_err();
    assert_eq!(sql_state(&error).as_deref(), Some("42501"));
    Ok(())
}

/// The database cannot tell which customer the runtime is serving. It confines
/// what may be written, but customer identity comes from the authenticated
/// application boundary.
#[sqlx::test(migrator = "MIGRATOR")]
async fn the_database_trusts_the_runtime_for_customer_identity(pool: PgPool) -> sqlx::Result<()> {
    let runtime = runtime(&pool).await?;
    let alice = customer(&pool, "alice-kyc@example.test", "REGISTERED").await?;
    let bob = customer(&pool, "bob-kyc@example.test", "REGISTERED").await?;
    let alices = open(&runtime, alice).await?.unwrap();

    // Trusted: any caller holding the runtime role may open a DRAFT for Bob.
    let bobs = open(&runtime, bob).await?.unwrap();
    let (owner, status): (Uuid, String) =
        sqlx::query_as("SELECT user_id, status FROM compliance.kyc_cases WHERE id = $1")
            .bind(bobs)
            .fetch_one(&pool)
            .await?;
    assert_eq!((owner, status.as_str()), (bob, "DRAFT"));

    // Confined: a case cannot be written under another owner, decided, or
    // moved from a non-customer status.
    let before = audit_count(&pool).await?;
    assert!(!submit(&runtime, bob, alices, "DRAFT").await?);
    assert!(!submit(&runtime, alice, bobs, "DRAFT").await?);
    for from in ["MANUAL_REVIEW", "PENDING_PROVIDER", "APPROVED", "REJECTED"] {
        let error = submit(&runtime, alice, alices, from).await.unwrap_err();
        assert_eq!(sql_state(&error).as_deref(), Some("23514"), "{from}");
    }
    assert_eq!(audit_count(&pool).await?, before);
    let statuses: Vec<String> =
        sqlx::query_scalar("SELECT status FROM compliance.kyc_cases ORDER BY status")
            .fetch_all(&pool)
            .await?;
    assert_eq!(statuses, ["DRAFT", "DRAFT"]);
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn malformed_calls_fail_closed_without_evidence(pool: PgPool) -> sqlx::Result<()> {
    let runtime = runtime(&pool).await?;
    let owner = customer(&pool, "kyc-malformed@example.test", "REGISTERED").await?;
    let case_id = open(&runtime, owner).await?.unwrap();
    let before = audit_count(&pool).await?;

    for (statement, state) in [
        (
            "SELECT * FROM compliance.open_first_kyc_case(NULL)",
            "23502",
        ),
        (
            "SELECT * FROM compliance.open_first_kyc_case('00000000-0000-0000-0000-000000000000')",
            "23503",
        ),
        ("SELECT compliance.submit_kyc_case($1, $1, NULL)", "23514"),
        (
            "SELECT compliance.submit_kyc_case($1, $1, 'draft')",
            "23514",
        ),
        (
            "SELECT compliance.submit_kyc_case($1, $1, 'SUBMITTED')",
            "23514",
        ),
    ] {
        let query = sqlx::query(statement);
        let query = if statement.contains("$1") {
            query.bind(owner)
        } else {
            query
        };
        let error = query.execute(&runtime).await.unwrap_err();
        assert_eq!(sql_state(&error).as_deref(), Some(state), "{statement}");
    }
    let unknown = Uuid::from_u128(0xFEED);
    assert!(!submit(&runtime, owner, unknown, "DRAFT").await?);
    let null_owner: bool =
        sqlx::query_scalar("SELECT compliance.submit_kyc_case(NULL, $1, 'DRAFT')")
            .bind(case_id)
            .fetch_one(&runtime)
            .await?;
    assert!(!null_owner);
    assert_eq!(audit_count(&pool).await?, before);
    Ok(())
}

/// Applying `kyc-grants.psql` without the Phase 2 schema grants leaves the
/// operations unusable rather than partially open.
#[sqlx::test(migrator = "MIGRATOR")]
async fn grants_applied_out_of_order_fail_closed(pool: PgPool) -> sqlx::Result<()> {
    sqlx::raw_sql(include_str!(
        "../../../../../infrastructure/postgres/kyc-grants.psql"
    ))
    .execute(&pool)
    .await?;
    let runtime = PgPoolOptions::new()
        .max_connections(1)
        .after_connect(|conn, _| {
            Box::pin(async move {
                sqlx::query("SET ROLE deypay_app").execute(conn).await?;
                Ok(())
            })
        })
        .connect_with((*pool.connect_options()).clone())
        .await?;
    let owner = customer(&pool, "kyc-order@example.test", "REGISTERED").await?;
    let error = open(&runtime, owner).await.unwrap_err();
    assert_eq!(sql_state(&error).as_deref(), Some("42501"));
    assert_eq!(audit_count(&pool).await?, 0);
    Ok(())
}
