//! Customer restriction engine (IMP-P4-006).
//!
//! Decides whether server-side evidence denies a customer's financial
//! operation. It is the deny side of the central eligibility check
//! (architecture §25.1) and covers the account status, KYC status and account
//! restriction layers of API §7.1. Limits are checked separately (IMP-P4-007).
//!
//! Every input is authoritative server-side data; nothing here accepts a
//! client-supplied flag. Evaluation denies unless every layer passes, in this
//! order:
//!
//! 1. A closed, suspended or restricted account.
//! 2. Any active account restriction. Restriction types have no approved
//!    vocabulary, so every active restriction denies every operation, whatever
//!    its type.
//! 3. Absent KYC approval. The account status must be `KYC_APPROVED` and the
//!    customer's only KYC case must be `APPROVED`; neither alone suffices.
//! 4. The approved eligibility policy. None exists, so
//!    [`NoApprovedEligibilityPolicy`] never establishes eligibility and every
//!    evaluation is denied.
//!
//! A restriction or account condition cannot be overridden by KYC status or by
//! the policy, because they are checked first. Evaluation only reads evidence;
//! it never lifts, edits or deletes a restriction.
use crate::kyc::KycCaseStatus;
use crate::operations::FinancialOperation;
use std::str::FromStr;
use uuid::Uuid;

/// Account statuses (FRS §8.4, `identity.users.status`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountStatus {
    Registered,
    ContactUnverified,
    ContactVerified,
    KycRequired,
    KycPending,
    KycApproved,
    KycRejected,
    Restricted,
    Suspended,
    Closed,
}

impl AccountStatus {
    pub const ALL: [Self; 10] = [
        Self::Registered,
        Self::ContactUnverified,
        Self::ContactVerified,
        Self::KycRequired,
        Self::KycPending,
        Self::KycApproved,
        Self::KycRejected,
        Self::Restricted,
        Self::Suspended,
        Self::Closed,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Registered => "REGISTERED",
            Self::ContactUnverified => "CONTACT_UNVERIFIED",
            Self::ContactVerified => "CONTACT_VERIFIED",
            Self::KycRequired => "KYC_REQUIRED",
            Self::KycPending => "KYC_PENDING",
            Self::KycApproved => "KYC_APPROVED",
            Self::KycRejected => "KYC_REJECTED",
            Self::Restricted => "RESTRICTED",
            Self::Suspended => "SUSPENDED",
            Self::Closed => "CLOSED",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnknownAccountStatus;

impl FromStr for AccountStatus {
    type Err = UnknownAccountStatus;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|status| status.as_str() == value)
            .ok_or(UnknownAccountStatus)
    }
}

/// Eligibility evidence as stored, read in one consistent snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EligibilityRecord {
    pub account_status: String,
    pub active_restrictions: i64,
    /// The status of each KYC case the customer holds. Reading two is enough
    /// to detect a breach of the one-case rule.
    pub kyc_case_statuses: Vec<String>,
}

/// Validated eligibility evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EligibilityEvidence {
    pub account_status: AccountStatus,
    pub active_restrictions: u64,
    pub kyc_case_status: Option<KycCaseStatus>,
}

impl TryFrom<&EligibilityRecord> for EligibilityEvidence {
    type Error = Denial;

    /// Unknown statuses, a negative count or more than one KYC case make the
    /// evidence unusable, which denies access rather than guess.
    fn try_from(record: &EligibilityRecord) -> Result<Self, Denial> {
        let kyc_case_status = match record.kyc_case_statuses.as_slice() {
            [] => None,
            [status] => Some(status.parse().map_err(|_| Denial::InvalidEvidence)?),
            _ => return Err(Denial::InvalidEvidence),
        };
        Ok(Self {
            account_status: record
                .account_status
                .parse()
                .map_err(|_| Denial::InvalidEvidence)?,
            active_restrictions: u64::try_from(record.active_restrictions)
                .map_err(|_| Denial::InvalidEvidence)?,
            kyc_case_status,
        })
    }
}

/// Why financial access is denied. Internal: for logs, audit and authorized
/// staff. Customers see only [`Denial::for_customer`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Denial {
    /// Evidence could not be loaded.
    EvidenceUnavailable,
    /// Evidence was missing or failed validation.
    InvalidEvidence,
    AccountClosed,
    AccountSuspended,
    /// The account status is `RESTRICTED`.
    AccountRestricted,
    /// At least one active account restriction applies.
    ActiveRestriction,
    /// The account status or KYC case does not show KYC approval.
    KycApprovalAbsent,
    /// No restriction applies, but the eligibility policy did not establish
    /// eligibility. Always the case until a policy is approved.
    EligibilityNotEstablished,
}

/// The coarse reason a customer may see. It never reveals a restriction's
/// type or reason code, or which account condition applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomerDenial {
    AccountRestricted,
    KycRequired,
    Unavailable,
}

impl CustomerDenial {
    /// The API error code (API §10.2; `SERVICE_UNAVAILABLE` as the registration
    /// endpoint already uses it).
    pub const fn code(self) -> &'static str {
        match self {
            Self::AccountRestricted => "ACCOUNT_RESTRICTED",
            Self::KycRequired => "KYC_REQUIRED",
            Self::Unavailable => "SERVICE_UNAVAILABLE",
        }
    }
}

impl Denial {
    pub const fn for_customer(self) -> CustomerDenial {
        match self {
            Self::AccountClosed
            | Self::AccountSuspended
            | Self::AccountRestricted
            | Self::ActiveRestriction => CustomerDenial::AccountRestricted,
            Self::KycApprovalAbsent => CustomerDenial::KycRequired,
            Self::EvidenceUnavailable | Self::InvalidEvidence | Self::EligibilityNotEstablished => {
                CustomerDenial::Unavailable
            }
        }
    }
}

/// The approved rules that decide eligibility once no restriction applies.
pub trait EligibilityPolicy {
    fn establishes(&self, operation: FinancialOperation, evidence: &EligibilityEvidence) -> bool;
}

/// The only policy available: no eligibility rules are approved, so it never
/// establishes eligibility.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoApprovedEligibilityPolicy;

impl EligibilityPolicy for NoApprovedEligibilityPolicy {
    fn establishes(&self, _: FinancialOperation, _: &EligibilityEvidence) -> bool {
        false
    }
}

/// Eligibility confirmed for one customer and one operation at evaluation
/// time. Only [`evaluate`] creates it, and it is neither `Clone` nor `Copy`.
///
/// It is not an authorization: limits, ownership, transaction confirmation and
/// risk controls still apply. Evidence can change after evaluation, so a
/// financial operation must re-evaluate within the transaction that accepts it.
#[derive(Debug, PartialEq, Eq)]
pub struct EligibilityConfirmed {
    customer_id: Uuid,
    operation: FinancialOperation,
}

impl EligibilityConfirmed {
    pub const fn customer_id(&self) -> Uuid {
        self.customer_id
    }

    pub const fn operation(&self) -> FinancialOperation {
        self.operation
    }
}

/// Evaluates validated evidence for one customer's operation.
pub fn evaluate(
    customer_id: Uuid,
    operation: FinancialOperation,
    evidence: &EligibilityEvidence,
    policy: &impl EligibilityPolicy,
) -> Result<EligibilityConfirmed, Denial> {
    match evidence.account_status {
        AccountStatus::Closed => return Err(Denial::AccountClosed),
        AccountStatus::Suspended => return Err(Denial::AccountSuspended),
        AccountStatus::Restricted => return Err(Denial::AccountRestricted),
        _ => {}
    }
    if evidence.active_restrictions > 0 {
        return Err(Denial::ActiveRestriction);
    }
    let kyc_approved = evidence
        .kyc_case_status
        .is_some_and(KycCaseStatus::satisfies_kyc_approval);
    if evidence.account_status != AccountStatus::KycApproved || !kyc_approved {
        return Err(Denial::KycApprovalAbsent);
    }
    if !policy.establishes(operation, evidence) {
        return Err(Denial::EligibilityNotEstablished);
    }
    Ok(EligibilityConfirmed {
        customer_id,
        operation,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const CUSTOMER: Uuid = Uuid::from_u128(0xC1);

    /// A stand-in for a future approved policy that establishes eligibility.
    struct PermitAll;
    impl EligibilityPolicy for PermitAll {
        fn establishes(&self, _: FinancialOperation, _: &EligibilityEvidence) -> bool {
            true
        }
    }

    fn evidence(
        account_status: AccountStatus,
        active_restrictions: u64,
        kyc: Option<KycCaseStatus>,
    ) -> EligibilityEvidence {
        EligibilityEvidence {
            account_status,
            active_restrictions,
            kyc_case_status: kyc,
        }
    }

    fn all_evidence() -> impl Iterator<Item = EligibilityEvidence> {
        let kyc = std::iter::once(None).chain(KycCaseStatus::ALL.into_iter().map(Some));
        AccountStatus::ALL.into_iter().flat_map(move |status| {
            let kyc = kyc.clone();
            [0, 1, 7]
                .into_iter()
                .flat_map(move |count| kyc.clone().map(move |k| evidence(status, count, k)))
        })
    }

    #[test]
    fn account_statuses_round_trip_exactly() {
        for status in AccountStatus::ALL {
            assert_eq!(status.as_str().parse(), Ok(status));
        }
        for unknown in ["", "kyc_approved", "ACTIVE", "VERIFIED", " CLOSED"] {
            assert_eq!(unknown.parse::<AccountStatus>(), Err(UnknownAccountStatus));
        }
    }

    #[test]
    fn the_shipped_policy_denies_every_customer_and_operation() {
        for evidence in all_evidence() {
            for operation in FinancialOperation::ALL {
                let result = evaluate(CUSTOMER, operation, &evidence, &NoApprovedEligibilityPolicy);
                assert!(result.is_err(), "{evidence:?} {operation:?}");
            }
        }
    }

    #[test]
    fn account_conditions_and_restrictions_cannot_be_overridden() {
        // Even a policy that establishes everything, with full KYC approval,
        // cannot override an account condition or an active restriction.
        let approved = Some(KycCaseStatus::Approved);
        for (status, denial) in [
            (AccountStatus::Closed, Denial::AccountClosed),
            (AccountStatus::Suspended, Denial::AccountSuspended),
            (AccountStatus::Restricted, Denial::AccountRestricted),
        ] {
            for count in [0, 1] {
                assert_eq!(
                    evaluate(
                        CUSTOMER,
                        FinancialOperation::Withdrawal,
                        &evidence(status, count, approved),
                        &PermitAll
                    ),
                    Err(denial)
                );
            }
        }
        for count in [1, 2, u64::MAX] {
            assert_eq!(
                evaluate(
                    CUSTOMER,
                    FinancialOperation::InternalTransfer,
                    &evidence(AccountStatus::KycApproved, count, approved),
                    &PermitAll
                ),
                Err(Denial::ActiveRestriction)
            );
        }
    }

    #[test]
    fn kyc_status_alone_never_establishes_eligibility() {
        for evidence in all_evidence() {
            let result = evaluate(
                CUSTOMER,
                FinancialOperation::BankPayment,
                &evidence,
                &PermitAll,
            );
            let clear = evidence.active_restrictions == 0
                && evidence.account_status == AccountStatus::KycApproved
                && evidence.kyc_case_status == Some(KycCaseStatus::Approved);
            assert_eq!(result.is_ok(), clear, "{evidence:?}");
        }
        // An approved case does not help an account that is not approved, and
        // an approved account does not help a case that is not.
        for status in AccountStatus::ALL {
            if matches!(
                status,
                AccountStatus::KycApproved
                    | AccountStatus::Closed
                    | AccountStatus::Suspended
                    | AccountStatus::Restricted
            ) {
                continue;
            }
            assert_eq!(
                evaluate(
                    CUSTOMER,
                    FinancialOperation::Deposit,
                    &evidence(status, 0, Some(KycCaseStatus::Approved)),
                    &PermitAll
                ),
                Err(Denial::KycApprovalAbsent)
            );
        }
        for kyc in std::iter::once(None).chain(
            KycCaseStatus::ALL
                .into_iter()
                .filter(|s| *s != KycCaseStatus::Approved)
                .map(Some),
        ) {
            assert_eq!(
                evaluate(
                    CUSTOMER,
                    FinancialOperation::Deposit,
                    &evidence(AccountStatus::KycApproved, 0, kyc),
                    &PermitAll
                ),
                Err(Denial::KycApprovalAbsent)
            );
        }
    }

    #[test]
    fn a_confirmation_is_bound_to_its_customer_and_operation() {
        let clear = evidence(AccountStatus::KycApproved, 0, Some(KycCaseStatus::Approved));
        assert_eq!(
            evaluate(
                CUSTOMER,
                FinancialOperation::Conversion,
                &clear,
                &NoApprovedEligibilityPolicy
            ),
            Err(Denial::EligibilityNotEstablished)
        );
        let confirmed =
            evaluate(CUSTOMER, FinancialOperation::Conversion, &clear, &PermitAll).unwrap();
        assert_eq!(
            (confirmed.customer_id(), confirmed.operation()),
            (CUSTOMER, FinancialOperation::Conversion)
        );
    }

    #[test]
    fn unusable_evidence_is_rejected() {
        let record = |status: &str, count: i64, kyc: &[&str]| EligibilityRecord {
            account_status: status.to_owned(),
            active_restrictions: count,
            kyc_case_statuses: kyc.iter().map(|s| (*s).to_owned()).collect(),
        };
        assert_eq!(
            EligibilityEvidence::try_from(&record("KYC_APPROVED", 0, &["APPROVED"])),
            Ok(evidence(
                AccountStatus::KycApproved,
                0,
                Some(KycCaseStatus::Approved)
            ))
        );
        assert_eq!(
            EligibilityEvidence::try_from(&record("REGISTERED", 3, &[])),
            Ok(evidence(AccountStatus::Registered, 3, None))
        );
        for bad in [
            record("ACTIVE", 0, &["APPROVED"]),
            record("kyc_approved", 0, &["APPROVED"]),
            record("KYC_APPROVED", -1, &["APPROVED"]),
            record("KYC_APPROVED", 0, &["approved"]),
            record("KYC_APPROVED", 0, &["NOT_STARTED"]),
            record("KYC_APPROVED", 0, &["APPROVED", "APPROVED"]),
        ] {
            assert_eq!(
                EligibilityEvidence::try_from(&bad),
                Err(Denial::InvalidEvidence),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn customers_see_only_coarse_reasons() {
        use Denial::*;
        for (denial, customer) in [
            (AccountClosed, CustomerDenial::AccountRestricted),
            (AccountSuspended, CustomerDenial::AccountRestricted),
            (AccountRestricted, CustomerDenial::AccountRestricted),
            (ActiveRestriction, CustomerDenial::AccountRestricted),
            (KycApprovalAbsent, CustomerDenial::KycRequired),
            (EvidenceUnavailable, CustomerDenial::Unavailable),
            (InvalidEvidence, CustomerDenial::Unavailable),
            (EligibilityNotEstablished, CustomerDenial::Unavailable),
        ] {
            assert_eq!(denial.for_customer(), customer, "{denial:?}");
        }
        assert_eq!(
            CustomerDenial::AccountRestricted.code(),
            "ACCOUNT_RESTRICTED"
        );
        assert_eq!(CustomerDenial::KycRequired.code(), "KYC_REQUIRED");
        assert_eq!(CustomerDenial::Unavailable.code(), "SERVICE_UNAVAILABLE");
    }
}
