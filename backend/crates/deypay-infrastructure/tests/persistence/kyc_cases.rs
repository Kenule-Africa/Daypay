//! PostgreSQL KYC case store (IMP-P4-005 prerequisite). Every store call runs as
//! the restricted runtime role with only the reviewed grants applied.
use super::support::{runtime, sql_state};
use deypay_application::kyc::{
    self as service, CustomerKycCases, KycCaseStatus, KycCaseStore, KycCaseView, KycServiceError,
    NoApprovedRequirements, SubmissionReadiness, SubmissionRequirements,
};
use deypay_application::repositories::{KycRepository, RepositoryError};
use deypay_domain::kyc::{KycActor, KycCaseCommand, KycCaseState, KycCaseTransition};
use deypay_infrastructure::MIGRATOR;
use deypay_infrastructure::kyc::PgKycCaseStore;
use sqlx::PgPool;
use std::time::Duration;
use uuid::Uuid;

async fn customer(pool: &PgPool, email: &str) -> sqlx::Result<Uuid> {
    super::support::customer(pool, email, "REGISTERED").await
}

async fn case_count(pool: &PgPool, owner: Uuid) -> sqlx::Result<i64> {
    sqlx::query_scalar("SELECT count(*) FROM compliance.kyc_cases WHERE user_id = $1")
        .bind(owner)
        .fetch_one(pool)
        .await
}

async fn status(pool: &PgPool, case_id: Uuid) -> sqlx::Result<String> {
    sqlx::query_scalar("SELECT status FROM compliance.kyc_cases WHERE id = $1")
        .bind(case_id)
        .fetch_one(pool)
        .await
}

/// Test-only: puts a case into any status, as a reviewer or provider path would.
async fn force_status(pool: &PgPool, case_id: Uuid, status: &str) -> sqlx::Result<()> {
    sqlx::query("UPDATE compliance.kyc_cases SET status = $2 WHERE id = $1")
        .bind(case_id)
        .bind(status)
        .execute(pool)
        .await
        .map(drop)
}

fn transition(
    owner: Uuid,
    from: KycCaseStatus,
    command: KycCaseCommand,
    actor: KycActor,
) -> KycCaseTransition {
    KycCaseState {
        owner_id: owner,
        status: from,
    }
    .transition(from, command, actor)
    .unwrap()
}

struct Ready;
impl SubmissionRequirements for Ready {
    async fn readiness(
        &self,
        _owner_id: Uuid,
        _case_id: Uuid,
    ) -> Result<SubmissionReadiness, RepositoryError> {
        Ok(SubmissionReadiness::Ready)
    }
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn a_first_case_is_one_owned_draft(pool: PgPool) -> sqlx::Result<()> {
    let store = PgKycCaseStore::new(runtime(&pool).await?);
    let owner = customer(&pool, "kyc-owner@example.test").await?;
    let other = customer(&pool, "kyc-other@example.test").await?;

    let view = service::open_first_case(&store, owner).await.unwrap();
    assert_eq!(view.status, KycCaseStatus::Draft);
    let (user_id, stored, untouched): (Uuid, String, bool) = sqlx::query_as(
        "SELECT user_id, status, submitted_at IS NULL AND reviewed_at IS NULL
            AND reviewer_id IS NULL AND rejection_reason_code IS NULL AND risk_tier IS NULL
        FROM compliance.kyc_cases WHERE id = $1",
    )
    .bind(view.case_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(
        (user_id, stored.as_str(), untouched),
        (owner, "DRAFT", true)
    );

    // No case opens beside an existing one, whatever its status.
    for existing in KycCaseStatus::ALL {
        force_status(&pool, view.case_id, existing.as_str()).await?;
        assert_eq!(
            service::open_first_case(&store, owner).await,
            Err(KycServiceError::CaseAlreadyExists)
        );
        assert_eq!(case_count(&pool, owner).await?, 1);
        assert_eq!(status(&pool, view.case_id).await?, existing.as_str());
    }

    let others = service::open_first_case(&store, other).await.unwrap();
    assert_ne!(others.case_id, view.case_id);

    // An unknown customer fails the foreign key and leaves nothing behind.
    let unknown = Uuid::from_u128(0xDEAD);
    assert_eq!(
        service::open_first_case(&store, unknown).await,
        Err(KycServiceError::InvalidRecord)
    );
    assert_eq!(case_count(&pool, unknown).await?, 0);
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn concurrent_first_case_requests_insert_once(pool: PgPool) -> sqlx::Result<()> {
    let runtime = runtime(&pool).await?;
    let owner = customer(&pool, "kyc-race@example.test").await?;
    let mut requests = tokio::task::JoinSet::new();
    for _ in 0..24 {
        let store = PgKycCaseStore::new(runtime.clone());
        requests.spawn(async move { service::open_first_case(&store, owner).await });
    }
    let results = requests.join_all().await;
    assert_eq!(
        results.iter().filter(|r| r.is_ok()).count(),
        1,
        "{results:?}"
    );
    assert!(
        results
            .iter()
            .all(|r| r.is_ok() || *r == Err(KycServiceError::CaseAlreadyExists))
    );
    assert_eq!(case_count(&pool, owner).await?, 1);
    Ok(())
}

/// Waits until a request in this database is blocked on an advisory lock.
async fn wait_for_blocked_request(pool: &PgPool) -> sqlx::Result<()> {
    for _ in 0..500 {
        let waiting: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM pg_locks l JOIN pg_database d ON d.oid = l.database
            WHERE d.datname = current_database() AND l.locktype = 'advisory' AND NOT l.granted",
        )
        .fetch_one(pool)
        .await?;
        if waiting > 0 {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("the second request never waited for the first");
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn first_case_requests_for_one_customer_are_serialized(pool: PgPool) -> sqlx::Result<()> {
    let runtime = runtime(&pool).await?;
    for commit in [true, false] {
        let owner = customer(&pool, &format!("kyc-serial-{commit}@example.test")).await?;
        // The first request has inserted but not yet committed.
        let mut first = runtime.begin().await?;
        let inserted: Option<Uuid> =
            sqlx::query_scalar("SELECT case_id FROM compliance.open_first_kyc_case($1)")
                .bind(owner)
                .fetch_optional(&mut *first)
                .await?;
        assert!(inserted.is_some());

        let store = PgKycCaseStore::new(runtime.clone());
        let second = tokio::spawn(async move { service::open_first_case(&store, owner).await });
        wait_for_blocked_request(&pool).await?;
        assert!(!second.is_finished());

        if commit {
            first.commit().await?;
            // The waiting request sees the committed case and inserts nothing.
            assert_eq!(
                second.await.unwrap(),
                Err(KycServiceError::CaseAlreadyExists)
            );
        } else {
            first.rollback().await?;
            // The rolled-back insert left nothing, so the waiting request proceeds.
            let view: KycCaseView = second.await.unwrap().unwrap();
            assert_ne!(Some(view.case_id), inserted);
        }
        assert_eq!(case_count(&pool, owner).await?, 1);
    }
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn failed_writes_persist_nothing(pool: PgPool) -> sqlx::Result<()> {
    let store = PgKycCaseStore::new(runtime(&pool).await?);
    let owner = customer(&pool, "kyc-failure@example.test").await?;
    sqlx::raw_sql(
        "CREATE FUNCTION public.fail_kyc_write() RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN RAISE EXCEPTION 'injected failure'; END; $$;
        CREATE TRIGGER fail_kyc_insert AFTER INSERT ON compliance.kyc_cases
            FOR EACH ROW EXECUTE FUNCTION public.fail_kyc_write();",
    )
    .execute(&pool)
    .await?;
    assert_eq!(
        service::open_first_case(&store, owner).await,
        Err(KycServiceError::Unavailable)
    );
    assert_eq!(case_count(&pool, owner).await?, 0);

    sqlx::raw_sql(
        "DROP TRIGGER fail_kyc_insert ON compliance.kyc_cases;
        CREATE TRIGGER fail_kyc_update AFTER UPDATE ON compliance.kyc_cases
            FOR EACH ROW EXECUTE FUNCTION public.fail_kyc_write();",
    )
    .execute(&pool)
    .await?;
    let view = service::open_first_case(&store, owner).await.unwrap();
    let submit = transition(
        owner,
        KycCaseStatus::Draft,
        KycCaseCommand::Submit,
        KycActor::Customer { customer_id: owner },
    );
    assert_eq!(
        store.apply_transition(owner, view.case_id, submit).await,
        Err(RepositoryError::StorageUnavailable)
    );
    assert_eq!(status(&pool, view.case_id).await?, "DRAFT");
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn customer_submission_is_conditional_and_owner_scoped(pool: PgPool) -> sqlx::Result<()> {
    let store = PgKycCaseStore::new(runtime(&pool).await?);
    let owner = customer(&pool, "kyc-submit@example.test").await?;
    let other = customer(&pool, "kyc-intruder@example.test").await?;
    let case_id = service::open_first_case(&store, owner)
        .await
        .unwrap()
        .case_id;
    let customer = KycActor::Customer { customer_id: owner };
    let from_draft = transition(
        owner,
        KycCaseStatus::Draft,
        KycCaseCommand::Submit,
        customer,
    );

    // Another customer, an unknown case and a stale status all write nothing.
    assert_eq!(
        store.apply_transition(other, case_id, from_draft).await,
        Ok(false)
    );
    let missing = Uuid::from_u128(0xFEED);
    assert_eq!(
        store.apply_transition(owner, missing, from_draft).await,
        Ok(false)
    );
    force_status(&pool, case_id, "INFORMATION_REQUESTED").await?;
    assert_eq!(
        store.apply_transition(owner, case_id, from_draft).await,
        Ok(false)
    );
    assert_eq!(status(&pool, case_id).await?, "INFORMATION_REQUESTED");

    let from_request = transition(
        owner,
        KycCaseStatus::InformationRequested,
        KycCaseCommand::Submit,
        customer,
    );
    assert_eq!(
        store.apply_transition(owner, case_id, from_request).await,
        Ok(true)
    );
    let (stored, submitted_at_unset, updated_in_order): (String, bool, bool) = sqlx::query_as(
        "SELECT status, submitted_at IS NULL, updated_at >= created_at
        FROM compliance.kyc_cases WHERE id = $1",
    )
    .bind(case_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(
        (stored.as_str(), submitted_at_unset, updated_in_order),
        ("SUBMITTED", true, true)
    );
    assert_eq!(
        store.apply_transition(owner, case_id, from_request).await,
        Ok(false)
    );

    force_status(&pool, case_id, "DRAFT").await?;
    assert_eq!(
        store.apply_transition(owner, case_id, from_draft).await,
        Ok(true)
    );
    assert_eq!(status(&pool, case_id).await?, "SUBMITTED");
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn provider_and_reviewer_transitions_are_never_written(pool: PgPool) -> sqlx::Result<()> {
    let store = PgKycCaseStore::new(runtime(&pool).await?);
    let owner = customer(&pool, "kyc-review@example.test").await?;
    let reviewer = KycActor::Reviewer {
        reviewer_id: customer(&pool, "kyc-reviewer@example.test").await?,
    };
    let case_id = service::open_first_case(&store, owner)
        .await
        .unwrap()
        .case_id;
    for (from, command, actor) in [
        (
            KycCaseStatus::Submitted,
            KycCaseCommand::DispatchToProvider,
            KycActor::System,
        ),
        (
            KycCaseStatus::PendingProvider,
            KycCaseCommand::RecordProviderResult,
            KycActor::System,
        ),
        (
            KycCaseStatus::ManualReview,
            KycCaseCommand::Approve,
            reviewer,
        ),
        (
            KycCaseStatus::ManualReview,
            KycCaseCommand::Reject,
            reviewer,
        ),
        (
            KycCaseStatus::ManualReview,
            KycCaseCommand::RequestInformation,
            reviewer,
        ),
    ] {
        force_status(&pool, case_id, from.as_str()).await?;
        let refused = transition(owner, from, command, actor);
        assert_eq!(
            store.apply_transition(owner, case_id, refused).await,
            Err(RepositoryError::InvalidRecord)
        );
        assert_eq!(status(&pool, case_id).await?, from.as_str());
    }
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn the_runtime_role_writes_only_through_the_fixed_operations(
    pool: PgPool,
) -> sqlx::Result<()> {
    let runtime = runtime(&pool).await?;
    let owner = customer(&pool, "kyc-privilege@example.test").await?;
    let store = PgKycCaseStore::new(runtime.clone());
    let case_id = service::open_first_case(&store, owner)
        .await
        .unwrap()
        .case_id;

    for statement in [
        "INSERT INTO compliance.kyc_cases(user_id,status) VALUES ($1,'APPROVED')",
        "UPDATE compliance.kyc_cases SET status = 'APPROVED' WHERE user_id = $1",
        "DELETE FROM compliance.kyc_cases WHERE user_id = $1",
        "SELECT reviewer_id FROM compliance.kyc_cases WHERE user_id = $1",
    ] {
        let error = sqlx::query(statement)
            .bind(owner)
            .execute(&runtime)
            .await
            .unwrap_err();
        assert_eq!(sql_state(&error).as_deref(), Some("42501"), "{statement}");
    }
    // The submission operation itself refuses every non-customer source status.
    for from in [
        "SUBMITTED",
        "PENDING_PROVIDER",
        "MANUAL_REVIEW",
        "APPROVED",
        "REJECTED",
        "EXPIRED",
        "draft",
    ] {
        let error = sqlx::query("SELECT compliance.submit_kyc_case($1, $2, $3)")
            .bind(owner)
            .bind(case_id)
            .bind(from)
            .execute(&runtime)
            .await
            .unwrap_err();
        assert_eq!(sql_state(&error).as_deref(), Some("23514"), "{from}");
    }
    // A transaction-wide snapshot could miss a concurrent case, so it is refused.
    let mut repeatable = runtime.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .execute(&mut *repeatable)
        .await?;
    let error = sqlx::query("SELECT * FROM compliance.open_first_kyc_case($1)")
        .bind(Uuid::from_u128(1))
        .execute(&mut *repeatable)
        .await
        .unwrap_err();
    assert_eq!(sql_state(&error).as_deref(), Some("0A000"));
    repeatable.rollback().await?;

    let public: (bool, bool) = sqlx::query_as(
        "SELECT has_function_privilege('public', 'compliance.open_first_kyc_case(uuid)', 'EXECUTE'),
            has_function_privilege('public', 'compliance.submit_kyc_case(uuid,uuid,text)', 'EXECUTE')",
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(public, (false, false));
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn case_lookups_are_owner_scoped(pool: PgPool) -> sqlx::Result<()> {
    let store = PgKycCaseStore::new(runtime(&pool).await?);
    let owner = customer(&pool, "kyc-lookup@example.test").await?;
    let other = customer(&pool, "kyc-lookup-other@example.test").await?;
    let view = service::open_first_case(&store, owner).await.unwrap();

    assert_eq!(service::current_case(&store, owner).await, Ok(Some(view)));
    assert_eq!(service::current_case(&store, other).await, Ok(None));
    assert!(store.kyc_case(other, view.case_id).await.unwrap().is_none());
    assert_eq!(
        service::customer_case(&store, other, view.case_id).await,
        Err(KycServiceError::NotFound)
    );

    // A second case breaks the one-case rule, so no case is chosen.
    sqlx::query("INSERT INTO compliance.kyc_cases(user_id,status) VALUES ($1,'DRAFT')")
        .bind(owner)
        .execute(&pool)
        .await?;
    assert_eq!(
        store.customer_kyc_case(owner).await,
        Err(RepositoryError::InvalidRecord)
    );
    assert_eq!(
        service::submit_current_case(&store, &Ready, owner).await,
        Err(KycServiceError::InvalidRecord)
    );
    assert_eq!(status(&pool, view.case_id).await?, "DRAFT");
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn submission_through_the_service_fails_closed_then_writes_once(
    pool: PgPool,
) -> sqlx::Result<()> {
    let store = PgKycCaseStore::new(runtime(&pool).await?);
    let owner = customer(&pool, "kyc-service@example.test").await?;
    let view = service::open_first_case(&store, owner).await.unwrap();

    // The shipped requirement source refuses every submission.
    assert_eq!(
        service::submit_current_case(&store, &NoApprovedRequirements, owner).await,
        Err(KycServiceError::SubmissionRequirementsUndefined)
    );
    assert_eq!(status(&pool, view.case_id).await?, "DRAFT");

    assert_eq!(
        service::submit_current_case(&store, &Ready, owner).await,
        Ok(KycCaseView {
            case_id: view.case_id,
            status: KycCaseStatus::Submitted
        })
    );
    assert!(matches!(
        service::submit_current_case(&store, &Ready, owner).await,
        Err(KycServiceError::Rejected(_))
    ));
    let approved: i64 =
        sqlx::query_scalar("SELECT count(*) FROM compliance.kyc_cases WHERE status = 'APPROVED'")
            .fetch_one(&pool)
            .await?;
    assert_eq!(approved, 0);
    Ok(())
}
