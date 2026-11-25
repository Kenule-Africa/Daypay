//! Transaction limit checks (IMP-P4-007).
//!
//! Loads the limit rules in force for a customer's operation and applies
//! [`deypay_domain::limits::check`]. Missing, unavailable or undefined limit
//! configuration denies. A passing check is not an authorization and consumes
//! no capacity.
//!
//! Future limit consumption must follow database design §10.6 inside the
//! transaction that accepts the operation: lock each customer/window bucket in
//! one consistent order, check and consume capacity there, count pending and
//! uncertain amounts, and release unused capacity once on definitive
//! non-execution. Querying past activity outside that transaction cannot
//! enforce a windowed limit under concurrency.
//!
//! No financial operation calls this yet, and none may be activated in Phase 4.
use crate::repositories::RepositoryError;
use deypay_domain::limits::{self, LimitDenial, LimitRule, TransactionAmount, WithinLimits};
use deypay_domain::operations::FinancialOperation;
use std::future::Future;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplicableLimits {
    /// No approved limit configuration exists.
    Undefined,
    /// The enabled rules in force for the customer and operation.
    Rules(Vec<LimitRule>),
}

pub trait TransactionLimitSource {
    fn applicable_limits(
        &self,
        customer_id: Uuid,
        operation: FinancialOperation,
    ) -> impl Future<Output = Result<ApplicableLimits, RepositoryError>> + Send;
}

/// The only source available. Thresholds, scope types, tiers and windows are
/// not approved, so no limit configuration exists and every check is denied.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoApprovedLimits;

impl TransactionLimitSource for NoApprovedLimits {
    fn applicable_limits(
        &self,
        _customer_id: Uuid,
        _operation: FinancialOperation,
    ) -> impl Future<Output = Result<ApplicableLimits, RepositoryError>> + Send {
        std::future::ready(Ok(ApplicableLimits::Undefined))
    }
}

/// Checks `amount` against the customer's limits for `operation`.
pub async fn check_transaction_limits(
    source: &impl TransactionLimitSource,
    customer_id: Uuid,
    operation: FinancialOperation,
    amount: TransactionAmount,
) -> Result<WithinLimits, LimitDenial> {
    match source.applicable_limits(customer_id, operation).await {
        Ok(ApplicableLimits::Rules(rules)) => limits::check(operation, amount, &rules),
        Ok(ApplicableLimits::Undefined) => Err(LimitDenial::LimitsUndefined),
        Err(_) => Err(LimitDenial::LimitsUnavailable),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deypay_domain::entities::Asset;
    use deypay_domain::limits::LimitPeriod;
    use deypay_domain::units::MinorUnits;
    use std::future::ready;
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};

    const CUSTOMER: Uuid = Uuid::from_u128(0xC1);
    const USDC: Uuid = Uuid::from_u128(0xA1);

    fn run<F: Future>(future: F) -> F::Output {
        match pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(output) => output,
            Poll::Pending => panic!("test doubles complete synchronously"),
        }
    }

    struct Source(Result<ApplicableLimits, RepositoryError>);
    impl TransactionLimitSource for Source {
        fn applicable_limits(
            &self,
            _customer_id: Uuid,
            _operation: FinancialOperation,
        ) -> impl Future<Output = Result<ApplicableLimits, RepositoryError>> + Send {
            ready(self.0.clone())
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

    fn check(
        source: &impl TransactionLimitSource,
        units: u128,
    ) -> Result<WithinLimits, LimitDenial> {
        run(check_transaction_limits(
            source,
            CUSTOMER,
            FinancialOperation::Withdrawal,
            amount(units),
        ))
    }

    #[test]
    fn unavailable_or_undefined_limits_deny() {
        assert_eq!(
            check(&NoApprovedLimits, 1),
            Err(LimitDenial::LimitsUndefined)
        );
        for error in [
            RepositoryError::StorageUnavailable,
            RepositoryError::InvalidRecord,
            RepositoryError::Conflict,
        ] {
            assert_eq!(
                check(&Source(Err(error)), 1),
                Err(LimitDenial::LimitsUnavailable)
            );
        }
        assert_eq!(
            check(&Source(Ok(ApplicableLimits::Rules(vec![]))), 1),
            Err(LimitDenial::NoApplicableLimits)
        );
    }

    #[test]
    fn loaded_rules_are_applied_exactly() {
        let rule = LimitRule {
            id: Uuid::from_u128(1),
            operation: FinancialOperation::Withdrawal,
            asset_id: Some(USDC),
            period: LimitPeriod::PerTransaction,
            limit: MinorUnits::new(500).unwrap(),
        };
        let source = Source(Ok(ApplicableLimits::Rules(vec![rule])));
        assert!(check(&source, 500).is_ok());
        assert_eq!(
            check(&source, 501),
            Err(LimitDenial::Exceeded { rule_id: rule.id })
        );
    }
}
