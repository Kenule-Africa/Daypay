//! The central backend boundary every financial operation must pass
//! (IMP-P4-006 with IMP-P4-007; architecture §25.1, API §7.1).
//!
//! [`authorize_financial_operation`] combines the separate checks. Each must
//! pass, and a pass in one never overrides a denial in another:
//!
//! 1. Eligibility ([`crate::restrictions`]): account status, active
//!    restrictions, KYC approval and the approved eligibility policy, all read
//!    from server-side evidence.
//! 2. Limits ([`crate::limits`]): rules the request can be checked against
//!    exactly. Windowed limits are denied until capacity can be consumed
//!    atomically in the accepting transaction.
//!
//! Authentication is the caller's precondition: `customer_id` must come from
//! the authenticated session, never from the request. Ownership, transaction
//! confirmation and risk controls remain separate steps.
//!
//! No financial operation exists yet (Phase 5 onwards), so nothing calls this
//! in production. With the shipped [`NoApprovedEligibilityPolicy`] and
//! [`NoApprovedLimits`], every call is denied.
//!
//! [`NoApprovedEligibilityPolicy`]: deypay_domain::restrictions::NoApprovedEligibilityPolicy
//! [`NoApprovedLimits`]: crate::limits::NoApprovedLimits
use crate::limits::{TransactionLimitSource, check_transaction_limits};
use crate::restrictions::{EligibilityEvidenceSource, evaluate_financial_access};
use deypay_domain::limits::{LimitDenial, TransactionAmount, WithinLimits};
use deypay_domain::money::Money;
use deypay_domain::operations::FinancialOperation;
use deypay_domain::restrictions::{Denial, EligibilityConfirmed, EligibilityPolicy};
use uuid::Uuid;

/// Which check denied the operation. Internal; customers see
/// [`FinancialDenial::customer_code`] only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinancialDenial {
    Eligibility(Denial),
    Limits(LimitDenial),
}

impl FinancialDenial {
    /// The API error code (API §10.2). It never reveals a restriction, a rule
    /// or a configuration gap.
    pub const fn customer_code(self) -> &'static str {
        match self {
            Self::Eligibility(denial) => denial.for_customer().code(),
            Self::Limits(LimitDenial::Exceeded { .. }) => "TRANSACTION_LIMIT_EXCEEDED",
            Self::Limits(_) => "SERVICE_UNAVAILABLE",
        }
    }
}

/// Both checks passed for one customer, operation and amount at evaluation
/// time. Only [`authorize_financial_operation`] creates it, and it is neither
/// `Clone` nor `Copy`. It consumes no limit capacity and is not transaction
/// confirmation.
#[derive(Debug, PartialEq, Eq)]
pub struct FinancialAuthorization {
    eligibility: EligibilityConfirmed,
    limits: WithinLimits,
}

impl FinancialAuthorization {
    pub const fn customer_id(&self) -> Uuid {
        self.eligibility.customer_id()
    }

    pub const fn operation(&self) -> FinancialOperation {
        self.eligibility.operation()
    }

    pub const fn amount(&self) -> Money {
        self.limits.amount()
    }
}

/// Authorizes `amount` of `operation` for the authenticated customer.
/// Eligibility is checked first, so an ineligible customer's limits are never
/// read.
pub async fn authorize_financial_operation(
    evidence: &impl EligibilityEvidenceSource,
    policy: &impl EligibilityPolicy,
    limits: &impl TransactionLimitSource,
    customer_id: Uuid,
    operation: FinancialOperation,
    amount: TransactionAmount,
) -> Result<FinancialAuthorization, FinancialDenial> {
    let eligibility = evaluate_financial_access(evidence, policy, customer_id, operation)
        .await
        .map_err(FinancialDenial::Eligibility)?;
    let limits = check_transaction_limits(limits, customer_id, operation, amount)
        .await
        .map_err(FinancialDenial::Limits)?;
    Ok(FinancialAuthorization {
        eligibility,
        limits,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::limits::{ApplicableLimits, NoApprovedLimits};
    use crate::repositories::{Lookup, RepositoryError};
    use FinancialOperation::*;
    use deypay_domain::entities::Asset;
    use deypay_domain::limits::{LimitPeriod, LimitRule};
    use deypay_domain::restrictions::{
        EligibilityEvidence, EligibilityRecord, NoApprovedEligibilityPolicy,
    };
    use deypay_domain::units::MinorUnits;
    use std::future::{Future, ready};
    use std::pin::pin;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::task::{Context, Poll, Waker};

    const CUSTOMER: Uuid = Uuid::from_u128(0xC1);
    const USDC: Uuid = Uuid::from_u128(0xA1);

    fn run<F: Future>(future: F) -> F::Output {
        match pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(output) => output,
            Poll::Pending => panic!("test doubles complete synchronously"),
        }
    }

    struct Evidence(&'static str, i64, &'static [&'static str]);
    impl EligibilityEvidenceSource for Evidence {
        fn eligibility_record(
            &self,
            _customer_id: Uuid,
        ) -> impl Future<Output = Lookup<EligibilityRecord>> + Send {
            ready(Ok(Some(EligibilityRecord {
                account_status: self.0.to_owned(),
                active_restrictions: self.1,
                kyc_case_statuses: self.2.iter().map(|s| (*s).to_owned()).collect(),
            })))
        }
    }
    const CLEAR: Evidence = Evidence("KYC_APPROVED", 0, &["APPROVED"]);

    /// Stands in for a future approved policy covering only `permitted`.
    struct Permits(&'static [FinancialOperation]);
    impl EligibilityPolicy for Permits {
        fn establishes(&self, operation: FinancialOperation, _: &EligibilityEvidence) -> bool {
            self.0.contains(&operation)
        }
    }

    struct Limits {
        result: Result<ApplicableLimits, RepositoryError>,
        reads: AtomicUsize,
    }
    impl Limits {
        fn new(result: Result<ApplicableLimits, RepositoryError>) -> Self {
            Self {
                result,
                reads: AtomicUsize::new(0),
            }
        }

        fn per_transaction(limit: u128) -> Self {
            Self::new(Ok(ApplicableLimits::Rules(vec![rule(
                LimitPeriod::PerTransaction,
                limit,
            )])))
        }
    }
    impl TransactionLimitSource for Limits {
        fn applicable_limits(
            &self,
            _customer_id: Uuid,
            _operation: FinancialOperation,
        ) -> impl Future<Output = Result<ApplicableLimits, RepositoryError>> + Send {
            self.reads.fetch_add(1, Ordering::SeqCst);
            ready(self.result.clone())
        }
    }

    fn rule(period: LimitPeriod, limit: u128) -> LimitRule {
        LimitRule {
            id: Uuid::from_u128(7),
            operation: Withdrawal,
            asset_id: Some(USDC),
            period,
            limit: MinorUnits::new(limit).unwrap(),
        }
    }

    fn amount(units: u128) -> TransactionAmount {
        let asset = Asset {
            id: USDC,
            symbol: "USDC".to_owned(),
            accounting_precision: 6,
            enabled: true,
        };
        TransactionAmount::from_units(MinorUnits::new(units).unwrap(), &asset).unwrap()
    }

    fn authorize(
        evidence: &Evidence,
        policy: &impl EligibilityPolicy,
        limits: &Limits,
        operation: FinancialOperation,
        units: u128,
    ) -> Result<FinancialAuthorization, FinancialDenial> {
        run(authorize_financial_operation(
            evidence,
            policy,
            limits,
            CUSTOMER,
            operation,
            amount(units),
        ))
    }

    #[test]
    fn the_shipped_components_deny_every_operation() {
        for operation in FinancialOperation::ALL {
            let denied = run(authorize_financial_operation(
                &CLEAR,
                &NoApprovedEligibilityPolicy,
                &NoApprovedLimits,
                CUSTOMER,
                operation,
                amount(1),
            ));
            assert_eq!(
                denied,
                Err(FinancialDenial::Eligibility(
                    Denial::EligibilityNotEstablished
                ))
            );
        }
        // Even with eligibility established, no limit configuration exists.
        assert_eq!(
            run(authorize_financial_operation(
                &CLEAR,
                &Permits(&[Withdrawal]),
                &NoApprovedLimits,
                CUSTOMER,
                Withdrawal,
                amount(1),
            )),
            Err(FinancialDenial::Limits(LimitDenial::LimitsUndefined))
        );
    }

    #[test]
    fn an_eligibility_denial_is_final_and_limits_are_not_read() {
        let generous = Limits::per_transaction(u128::from(u64::MAX));
        for (evidence, denial) in [
            (
                Evidence("KYC_APPROVED", 1, &["APPROVED"]),
                Denial::ActiveRestriction,
            ),
            (
                Evidence("SUSPENDED", 0, &["APPROVED"]),
                Denial::AccountSuspended,
            ),
            (Evidence("CLOSED", 0, &["APPROVED"]), Denial::AccountClosed),
            (Evidence("KYC_APPROVED", 0, &[]), Denial::KycApprovalAbsent),
            // Conflicting evidence: either side without approval denies.
            (
                Evidence("KYC_REJECTED", 0, &["APPROVED"]),
                Denial::KycApprovalAbsent,
            ),
            (
                Evidence("KYC_APPROVED", 0, &["REJECTED"]),
                Denial::KycApprovalAbsent,
            ),
            (
                Evidence("UNKNOWN", 0, &["APPROVED"]),
                Denial::InvalidEvidence,
            ),
        ] {
            assert_eq!(
                authorize(
                    &evidence,
                    &Permits(&FinancialOperation::ALL),
                    &generous,
                    Withdrawal,
                    1
                ),
                Err(FinancialDenial::Eligibility(denial))
            );
        }
        // An operation the policy does not establish is denied the same way.
        assert_eq!(
            authorize(&CLEAR, &Permits(&[Withdrawal]), &generous, BankPayment, 1),
            Err(FinancialDenial::Eligibility(
                Denial::EligibilityNotEstablished
            ))
        );
        assert_eq!(generous.reads.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn a_limit_denial_overrides_established_eligibility() {
        let policy = Permits(&[Withdrawal]);
        for (limits, units, denial) in [
            (
                Limits::per_transaction(500),
                501,
                LimitDenial::Exceeded {
                    rule_id: Uuid::from_u128(7),
                },
            ),
            (
                Limits::new(Ok(ApplicableLimits::Rules(vec![rule(
                    LimitPeriod::Windowed,
                    u128::from(u64::MAX),
                )]))),
                1,
                LimitDenial::RequiresAtomicConsumption {
                    rule_id: Uuid::from_u128(7),
                },
            ),
            (
                Limits::new(Ok(ApplicableLimits::Rules(vec![]))),
                1,
                LimitDenial::NoApplicableLimits,
            ),
            (
                Limits::new(Err(RepositoryError::StorageUnavailable)),
                1,
                LimitDenial::LimitsUnavailable,
            ),
        ] {
            assert_eq!(
                authorize(&CLEAR, &policy, &limits, Withdrawal, units),
                Err(FinancialDenial::Limits(denial))
            );
        }
    }

    #[test]
    fn an_authorization_binds_customer_operation_and_amount() {
        let granted = authorize(
            &CLEAR,
            &Permits(&[Withdrawal]),
            &Limits::per_transaction(500),
            Withdrawal,
            500,
        )
        .unwrap();
        assert_eq!(
            (
                granted.customer_id(),
                granted.operation(),
                granted.amount().units.value()
            ),
            (CUSTOMER, Withdrawal, 500)
        );
    }

    #[test]
    fn customers_see_only_coarse_codes() {
        use FinancialDenial::*;
        for (denial, code) in [
            (Eligibility(Denial::ActiveRestriction), "ACCOUNT_RESTRICTED"),
            (Eligibility(Denial::AccountClosed), "ACCOUNT_RESTRICTED"),
            (Eligibility(Denial::KycApprovalAbsent), "KYC_REQUIRED"),
            (
                Eligibility(Denial::EvidenceUnavailable),
                "SERVICE_UNAVAILABLE",
            ),
            (
                Limits(LimitDenial::Exceeded {
                    rule_id: Uuid::nil(),
                }),
                "TRANSACTION_LIMIT_EXCEEDED",
            ),
            (
                Limits(LimitDenial::ValuationUndefined {
                    rule_id: Uuid::nil(),
                }),
                "SERVICE_UNAVAILABLE",
            ),
            (
                Limits(LimitDenial::RequiresAtomicConsumption {
                    rule_id: Uuid::nil(),
                }),
                "SERVICE_UNAVAILABLE",
            ),
            (Limits(LimitDenial::LimitsUndefined), "SERVICE_UNAVAILABLE"),
        ] {
            assert_eq!(denial.customer_code(), code, "{denial:?}");
        }
    }
}
