//! Customer restriction evaluation (IMP-P4-006).
//!
//! Loads eligibility evidence from authoritative server-side storage and
//! applies [`deypay_domain::restrictions::evaluate`]. A load failure, an unknown
//! customer or unusable evidence denies access; missing evidence is never read
//! as approval. Callers must take `customer_id` from the authenticated session,
//! and no request field can override the outcome.
//!
//! No financial operation calls this yet, and none may be activated in Phase 4.
use crate::repositories::{Lookup, RepositoryError};
use deypay_domain::operations::FinancialOperation;
use deypay_domain::restrictions::{
    self, Denial, EligibilityConfirmed, EligibilityEvidence, EligibilityPolicy, EligibilityRecord,
};
use std::future::Future;
use uuid::Uuid;

/// Reads a customer's eligibility evidence in one consistent snapshot.
pub trait EligibilityEvidenceSource {
    /// Returns `None` when the customer does not exist.
    fn eligibility_record(
        &self,
        customer_id: Uuid,
    ) -> impl Future<Output = Lookup<EligibilityRecord>> + Send;
}

/// Evaluates whether the customer may perform `operation`.
pub async fn evaluate_financial_access(
    evidence: &impl EligibilityEvidenceSource,
    policy: &impl EligibilityPolicy,
    customer_id: Uuid,
    operation: FinancialOperation,
) -> Result<EligibilityConfirmed, Denial> {
    let record = match evidence.eligibility_record(customer_id).await {
        Ok(Some(record)) => record,
        Ok(None) | Err(RepositoryError::InvalidRecord) => return Err(Denial::InvalidEvidence),
        Err(RepositoryError::StorageUnavailable | RepositoryError::Conflict) => {
            return Err(Denial::EvidenceUnavailable);
        }
    };
    let evidence = EligibilityEvidence::try_from(&record)?;
    restrictions::evaluate(customer_id, operation, &evidence, policy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use deypay_domain::restrictions::NoApprovedEligibilityPolicy;
    use std::future::ready;
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};

    const CUSTOMER: Uuid = Uuid::from_u128(0xC1);

    fn run<F: Future>(future: F) -> F::Output {
        match pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(output) => output,
            Poll::Pending => panic!("test doubles complete synchronously"),
        }
    }

    struct Evidence(Lookup<EligibilityRecord>);
    impl EligibilityEvidenceSource for Evidence {
        fn eligibility_record(
            &self,
            _customer_id: Uuid,
        ) -> impl Future<Output = Lookup<EligibilityRecord>> + Send {
            ready(self.0.clone())
        }
    }

    struct PermitAll;
    impl EligibilityPolicy for PermitAll {
        fn establishes(&self, _: FinancialOperation, _: &EligibilityEvidence) -> bool {
            true
        }
    }

    fn record(status: &str, restrictions: i64, kyc: &[&str]) -> EligibilityRecord {
        EligibilityRecord {
            account_status: status.to_owned(),
            active_restrictions: restrictions,
            kyc_case_statuses: kyc.iter().map(|s| (*s).to_owned()).collect(),
        }
    }

    fn evaluate(
        source: Evidence,
        policy: &impl EligibilityPolicy,
    ) -> Result<EligibilityConfirmed, Denial> {
        run(evaluate_financial_access(
            &source,
            policy,
            CUSTOMER,
            FinancialOperation::Withdrawal,
        ))
    }

    #[test]
    fn eligibility_that_cannot_be_established_is_denied() {
        for (source, denial) in [
            (
                Evidence(Err(RepositoryError::StorageUnavailable)),
                Denial::EvidenceUnavailable,
            ),
            (
                Evidence(Err(RepositoryError::Conflict)),
                Denial::EvidenceUnavailable,
            ),
            (
                Evidence(Err(RepositoryError::InvalidRecord)),
                Denial::InvalidEvidence,
            ),
            (Evidence(Ok(None)), Denial::InvalidEvidence),
            (
                Evidence(Ok(Some(record("UNKNOWN", 0, &["APPROVED"])))),
                Denial::InvalidEvidence,
            ),
            (
                Evidence(Ok(Some(record("KYC_APPROVED", 0, &["APPROVED", "DRAFT"])))),
                Denial::InvalidEvidence,
            ),
        ] {
            // Even a policy that establishes everything cannot fill the gap.
            assert_eq!(evaluate(source, &PermitAll), Err(denial));
        }
    }

    #[test]
    fn stored_restrictions_deny_whatever_the_policy() {
        for (record, denial) in [
            (
                record("KYC_APPROVED", 1, &["APPROVED"]),
                Denial::ActiveRestriction,
            ),
            (
                record("SUSPENDED", 0, &["APPROVED"]),
                Denial::AccountSuspended,
            ),
            (record("CLOSED", 0, &["APPROVED"]), Denial::AccountClosed),
            (
                record("RESTRICTED", 0, &["APPROVED"]),
                Denial::AccountRestricted,
            ),
            (
                record("CONTACT_VERIFIED", 0, &[]),
                Denial::KycApprovalAbsent,
            ),
        ] {
            assert_eq!(
                evaluate(Evidence(Ok(Some(record))), &PermitAll),
                Err(denial)
            );
        }
    }

    #[test]
    fn clear_evidence_still_needs_an_approved_policy() {
        let clear = || Evidence(Ok(Some(record("KYC_APPROVED", 0, &["APPROVED"]))));
        assert_eq!(
            evaluate(clear(), &NoApprovedEligibilityPolicy),
            Err(Denial::EligibilityNotEstablished)
        );
        let confirmed = evaluate(clear(), &PermitAll).unwrap();
        assert_eq!(
            (confirmed.customer_id(), confirmed.operation()),
            (CUSTOMER, FinancialOperation::Withdrawal)
        );
    }
}
