//! PostgreSQL eligibility evidence (IMP-P4-006), read as the restricted
//! runtime role with only the reviewed grants applied.
use super::support::{customer, runtime, sql_state};
use deypay_application::financial_authorization::{FinancialDenial, authorize_financial_operation};
use deypay_application::limits::{ApplicableLimits, NoApprovedLimits, TransactionLimitSource};
use deypay_application::repositories::RepositoryError;
use deypay_application::restrictions::evaluate_financial_access;
use deypay_domain::entities::Asset;
use deypay_domain::limits::{LimitDenial, LimitPeriod, LimitRule, TransactionAmount};
use deypay_domain::operations::FinancialOperation;
use deypay_domain::restrictions::{
    Denial, EligibilityConfirmed, EligibilityEvidence, EligibilityPolicy,
    NoApprovedEligibilityPolicy,
};
use deypay_domain::units::MinorUnits;
use deypay_infrastructure::MIGRATOR;
use deypay_infrastructure::restrictions::PgEligibilityEvidence;
use sqlx::PgPool;
use uuid::Uuid;

/// Test-only stand-in for a future approved policy, so that the evidence
/// checks can be observed on their own.
struct PermitAll;
impl EligibilityPolicy for PermitAll {
    fn establishes(&self, _: FinancialOperation, _: &EligibilityEvidence) -> bool {
        true
    }
}

async fn evaluate(
    evidence: &PgEligibilityEvidence,
    policy: &impl EligibilityPolicy,
    customer_id: Uuid,
) -> Result<EligibilityConfirmed, Denial> {
    evaluate_financial_access(
        evidence,
        policy,
        customer_id,
        FinancialOperation::Withdrawal,
    )
    .await
}

async fn kyc_case(pool: &PgPool, owner: Uuid, status: &str) -> sqlx::Result<()> {
    sqlx::query("INSERT INTO compliance.kyc_cases(user_id,status) VALUES ($1,$2)")
        .bind(owner)
        .bind(status)
        .execute(pool)
        .await
        .map(drop)
}

async fn restrict(pool: &PgPool, owner: Uuid, restriction_type: &str) -> sqlx::Result<Uuid> {
    sqlx::query_scalar(
        "INSERT INTO compliance.account_restrictions(user_id,restriction_type,reason_code,active)
        VALUES ($1,$2,'TEST_REASON',true) RETURNING id",
    )
    .bind(owner)
    .bind(restriction_type)
    .fetch_one(pool)
    .await
}

/// A fingerprint of every stored restriction, to show evaluation changes none.
async fn restriction_history(pool: &PgPool) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar(
        "SELECT md5(string_agg(r::text, '|' ORDER BY r.id)) FROM compliance.account_restrictions r",
    )
    .fetch_one(pool)
    .await
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn active_restrictions_deny_and_lifted_ones_are_kept(pool: PgPool) -> sqlx::Result<()> {
    let evidence = PgEligibilityEvidence::new(runtime(&pool).await?);
    let owner = customer(&pool, "restricted@example.test", "KYC_APPROVED").await?;
    kyc_case(&pool, owner, "APPROVED").await?;

    // The evidence is clear, yet no approved policy establishes eligibility.
    assert_eq!(
        evaluate(&evidence, &NoApprovedEligibilityPolicy, owner).await,
        Err(Denial::EligibilityNotEstablished)
    );
    assert!(evaluate(&evidence, &PermitAll, owner).await.is_ok());

    // A restriction of a type nobody has defined still denies.
    let first = restrict(&pool, owner, "UNREVIEWED_TYPE").await?;
    let second = restrict(&pool, owner, "WITHDRAWAL_HOLD").await?;
    assert_eq!(
        evaluate(&evidence, &PermitAll, owner).await,
        Err(Denial::ActiveRestriction)
    );

    let before = restriction_history(&pool).await?;
    let _ = evaluate(&evidence, &PermitAll, owner).await;
    assert_eq!(restriction_history(&pool).await?, before);

    // Lifting one of two still leaves an active restriction.
    let lift = "UPDATE compliance.account_restrictions SET active = false, lifted_at = now() WHERE id = $1";
    sqlx::query(lift).bind(first).execute(&pool).await?;
    assert_eq!(
        evaluate(&evidence, &PermitAll, owner).await,
        Err(Denial::ActiveRestriction)
    );
    sqlx::query(lift).bind(second).execute(&pool).await?;
    assert!(evaluate(&evidence, &PermitAll, owner).await.is_ok());

    // Lifted restrictions remain as history.
    let kept: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM compliance.account_restrictions
        WHERE user_id = $1 AND NOT active AND lifted_at IS NOT NULL",
    )
    .bind(owner)
    .fetch_one(&pool)
    .await?;
    assert_eq!(kept, 2);
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn account_and_kyc_conditions_deny(pool: PgPool) -> sqlx::Result<()> {
    let evidence = PgEligibilityEvidence::new(runtime(&pool).await?);
    for (status, denial) in [
        ("CLOSED", Denial::AccountClosed),
        ("SUSPENDED", Denial::AccountSuspended),
        ("RESTRICTED", Denial::AccountRestricted),
        ("REGISTERED", Denial::KycApprovalAbsent),
        ("CONTACT_VERIFIED", Denial::KycApprovalAbsent),
        ("KYC_PENDING", Denial::KycApprovalAbsent),
        ("KYC_REJECTED", Denial::KycApprovalAbsent),
    ] {
        let email = format!("{}@example.test", status.to_lowercase());
        let owner = customer(&pool, &email, status).await?;
        kyc_case(&pool, owner, "APPROVED").await?;
        assert_eq!(
            evaluate(&evidence, &PermitAll, owner).await,
            Err(denial),
            "{status}"
        );
    }

    // An approved account still needs an approved KYC case, and only one.
    let approved = customer(&pool, "approved@example.test", "KYC_APPROVED").await?;
    assert_eq!(
        evaluate(&evidence, &PermitAll, approved).await,
        Err(Denial::KycApprovalAbsent)
    );
    kyc_case(&pool, approved, "MANUAL_REVIEW").await?;
    assert_eq!(
        evaluate(&evidence, &PermitAll, approved).await,
        Err(Denial::KycApprovalAbsent)
    );
    kyc_case(&pool, approved, "APPROVED").await?;
    assert_eq!(
        evaluate(&evidence, &PermitAll, approved).await,
        Err(Denial::InvalidEvidence)
    );

    // An unknown customer has no evidence, which is never approval.
    assert_eq!(
        evaluate(&evidence, &PermitAll, Uuid::from_u128(0xBAD)).await,
        Err(Denial::InvalidEvidence)
    );
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn the_runtime_role_reads_restrictions_minimally(pool: PgPool) -> sqlx::Result<()> {
    let runtime = runtime(&pool).await?;
    let owner = customer(&pool, "private@example.test", "KYC_APPROVED").await?;
    restrict(&pool, owner, "UNREVIEWED_TYPE").await?;
    // Filtering on the granted user_id isolates each refused column or command.
    for statement in [
        "SELECT reason_code FROM compliance.account_restrictions WHERE user_id = $1",
        "SELECT restriction_type FROM compliance.account_restrictions WHERE user_id = $1",
        "SELECT created_by FROM compliance.account_restrictions WHERE user_id = $1",
        "UPDATE compliance.account_restrictions SET active = false, lifted_at = now() WHERE user_id = $1",
        "DELETE FROM compliance.account_restrictions WHERE user_id = $1",
        "INSERT INTO compliance.account_restrictions(user_id,restriction_type,reason_code,active)
            VALUES ($1,'X','Y',false)",
    ] {
        let error = sqlx::query(statement)
            .bind(owner)
            .execute(&runtime)
            .await
            .unwrap_err();
        assert_eq!(sql_state(&error).as_deref(), Some("42501"), "{statement}");
    }
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn unreadable_evidence_denies(pool: PgPool) -> sqlx::Result<()> {
    let evidence = PgEligibilityEvidence::new(runtime(&pool).await?);
    let owner = customer(&pool, "outage@example.test", "KYC_APPROVED").await?;
    kyc_case(&pool, owner, "APPROVED").await?;
    assert!(evaluate(&evidence, &PermitAll, owner).await.is_ok());
    sqlx::query(
        "REVOKE SELECT (user_id, active) ON compliance.account_restrictions FROM deypay_app",
    )
    .execute(&pool)
    .await?;
    assert_eq!(
        evaluate(&evidence, &PermitAll, owner).await,
        Err(Denial::EvidenceUnavailable)
    );
    Ok(())
}

/// A per-transaction withdrawal limit, standing in for approved configuration.
struct OneRule(LimitRule);
impl TransactionLimitSource for OneRule {
    async fn applicable_limits(
        &self,
        _customer_id: Uuid,
        _operation: FinancialOperation,
    ) -> Result<ApplicableLimits, RepositoryError> {
        Ok(ApplicableLimits::Rules(vec![self.0]))
    }
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn the_central_boundary_uses_each_customers_own_evidence(pool: PgPool) -> sqlx::Result<()> {
    let evidence = PgEligibilityEvidence::new(runtime(&pool).await?);
    let alice = customer(&pool, "alice-funds@example.test", "KYC_APPROVED").await?;
    let bob = customer(&pool, "bob-funds@example.test", "KYC_APPROVED").await?;
    for owner in [alice, bob] {
        kyc_case(&pool, owner, "APPROVED").await?;
    }
    restrict(&pool, alice, "UNREVIEWED_TYPE").await?;
    let usdc = Asset {
        id: Uuid::from_u128(0xA1),
        symbol: "USDC".to_owned(),
        accounting_precision: 6,
        enabled: true,
    };
    let rule = LimitRule {
        id: Uuid::from_u128(7),
        operation: FinancialOperation::Withdrawal,
        asset_id: Some(usdc.id),
        period: LimitPeriod::PerTransaction,
        limit: MinorUnits::new(1_000).unwrap(),
    };
    let limits = OneRule(rule);
    let amount =
        |units| TransactionAmount::from_units(MinorUnits::new(units).unwrap(), &usdc).unwrap();
    let withdraw = FinancialOperation::Withdrawal;

    // Alice's restriction denies her even though her limit check would pass.
    assert_eq!(
        authorize_financial_operation(&evidence, &PermitAll, &limits, alice, withdraw, amount(1))
            .await,
        Err(FinancialDenial::Eligibility(Denial::ActiveRestriction))
    );
    // Bob is judged on his own evidence: eligible, then limited exactly.
    let granted =
        authorize_financial_operation(&evidence, &PermitAll, &limits, bob, withdraw, amount(1_000))
            .await
            .unwrap();
    assert_eq!(
        (granted.customer_id(), granted.amount().units.value()),
        (bob, 1_000)
    );
    assert_eq!(
        authorize_financial_operation(&evidence, &PermitAll, &limits, bob, withdraw, amount(1_001))
            .await,
        Err(FinancialDenial::Limits(LimitDenial::Exceeded {
            rule_id: rule.id
        }))
    );
    // The shipped policy and limit source deny Bob too.
    assert_eq!(
        authorize_financial_operation(
            &evidence,
            &NoApprovedEligibilityPolicy,
            &NoApprovedLimits,
            bob,
            withdraw,
            amount(1)
        )
        .await,
        Err(FinancialDenial::Eligibility(
            Denial::EligibilityNotEstablished
        ))
    );
    // Evidence that cannot be read denies.
    sqlx::query(
        "REVOKE SELECT (user_id, active) ON compliance.account_restrictions FROM deypay_app",
    )
    .execute(&pool)
    .await?;
    assert_eq!(
        authorize_financial_operation(&evidence, &PermitAll, &limits, bob, withdraw, amount(1))
            .await,
        Err(FinancialDenial::Eligibility(Denial::EvidenceUnavailable))
    );
    Ok(())
}
