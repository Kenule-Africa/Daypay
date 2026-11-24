//! KYC identity verification provider interface (IMP-P4-003).
//!
//! A provider-independent port for external identity verification
//! (architecture §§14.3, 15.2; FR-KYC-005, FR-KYC-006). Concrete adapters live
//! in the infrastructure crate. This module defines no provider payloads,
//! credentials, HTTP calls or webhooks.
//!
//! A provider result is an observation, not a KYC decision. A completed
//! outcome, pass or fail, can lead only to
//! [`KycCaseCommand::RecordProviderResult`], which moves a case to manual
//! review. Nothing here approves or rejects a case, or changes account status,
//! restrictions or financial eligibility.
//!
//! Deliberately not encoded, because no approved specification defines it:
//!
//! - The identity data and documents a request carries. No field set is
//!   approved, so a request carries DEYPAY identifiers only.
//! - Provider capabilities, idempotency and webhook semantics, all unverified
//!   in the provider capability register.
//! - Provider reason codes and result summaries.
//! - Canonical errors, operation states and correlation shared by every
//!   provider, which Phase 8 defines (IMP-P8-002, -003, -006).
use deypay_domain::kyc::KycCaseCommand;
use std::{future::Future, pin::Pin};
use uuid::Uuid;

/// A request to verify one KYC case. It carries identifiers only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerificationRequest {
    pub case_id: Uuid,
    /// DEYPAY's reference for this submission. Persist it before any network
    /// call and reuse it on every retry, so that an uncertain submission can be
    /// looked up rather than repeated (database design §22.3).
    pub submission_reference: Uuid,
}

/// The provider's opaque identifier for a verification.
///
/// It is untrusted external data (SEC-PRV-005) and is never a DEYPAY identity
/// (architecture §14.4).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExternalReference(String);

impl ExternalReference {
    /// Matches `integrations.provider_operations.external_reference`.
    pub const MAX_CHARS: usize = 512;

    pub fn parse(value: &str) -> Result<Self, KycProviderError> {
        if value.trim().is_empty()
            || value.chars().count() > Self::MAX_CHARS
            || value.chars().any(char::is_control)
        {
            return Err(KycProviderError::MalformedResponse);
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// What a provider reports about a verification. Never a KYC case decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationOutcome {
    /// The provider has not finished.
    Pending,
    /// The provider reports that its checks passed.
    ProviderPassed,
    /// The provider reports that its checks failed.
    ProviderFailed,
}

impl VerificationOutcome {
    pub const fn is_complete(self) -> bool {
        !matches!(self, Self::Pending)
    }

    /// The only case command this outcome may lead to.
    ///
    /// Pass and fail alike record a provider result, which routes the case to
    /// manual review under the provisional IMP-P4-001 policy. No outcome maps
    /// to `Approve` or `Reject`.
    pub const fn case_command(self) -> Option<KycCaseCommand> {
        if self.is_complete() {
            Some(KycCaseCommand::RecordProviderResult)
        } else {
            None
        }
    }
}

/// A provider's answer about one submission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationReport {
    /// The DEYPAY submission reference the provider says it is answering.
    pub submission_reference: Uuid,
    pub external_reference: ExternalReference,
    pub outcome: VerificationOutcome,
}

impl VerificationReport {
    /// Accepts the report only if it answers `submission_reference` and, when
    /// DEYPAY already holds the provider's reference, carries that reference.
    ///
    /// A mismatch is treated as malformed, because DEYPAY then cannot tell what
    /// happened to its own submission. Passing this check does not make a
    /// report authentic; inbound webhooks also need signature verification and
    /// deduplication (architecture §23.2), which this interface does not cover.
    pub fn correlate(
        self,
        submission_reference: Uuid,
        known_external: Option<&ExternalReference>,
    ) -> Result<Self, KycProviderError> {
        let known_matches = known_external.is_none_or(|known| *known == self.external_reference);
        if self.submission_reference == submission_reference && known_matches {
            Ok(self)
        } else {
            Err(KycProviderError::MalformedResponse)
        }
    }
}

/// Provider failures. They carry no provider messages, payloads or credentials.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KycProviderError {
    /// No answer within the adapter's deadline. The provider may have acted.
    Timeout,
    /// The response failed validation or answers a different request. The
    /// provider may have acted.
    MalformedResponse,
    /// The provider was unavailable and definitely did not act. Adapters that
    /// cannot be sure must report `Timeout` instead.
    Unavailable,
    /// The provider refused the request and did not act.
    Rejected,
    /// The provider or adapter does not support this operation.
    Unsupported,
    /// The submission reference is already in use for a different case.
    ReferenceConflict,
    /// The provider holds no verification for this reference.
    NotFound,
}

impl KycProviderError {
    /// Whether the provider may have acted. If so, look the submission up or
    /// reconcile before any retry (architecture §35.5, API §11.6).
    pub const fn outcome_unknown(self) -> bool {
        matches!(self, Self::Timeout | Self::MalformedResponse)
    }
}

pub type ProviderFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, KycProviderError>> + Send + 'a>>;

/// External identity verification (architecture §15.2).
///
/// Implementations enforce their own deadline and report its expiry as
/// [`KycProviderError::Timeout`]. They must never log credentials, identity data
/// or raw provider payloads (SEC-LOG-003). Provider selection is explicit, and
/// an uncertain submission is never retried against another provider
/// (architecture §15.4).
pub trait IdentityVerificationProvider: Send + Sync {
    /// Submits a case for verification under `request.submission_reference`.
    ///
    /// Repeating a reference after an uncertain outcome is a replay. Whether a
    /// real provider deduplicates it is unverified.
    fn submit_verification(
        &self,
        request: VerificationRequest,
    ) -> ProviderFuture<'_, VerificationReport>;

    /// Returns the provider's current report for a submission reference.
    /// Providers without lookup return [`KycProviderError::Unsupported`].
    fn verification_status(
        &self,
        submission_reference: Uuid,
    ) -> ProviderFuture<'_, VerificationReport>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use deypay_domain::kyc::{KycActor, KycCaseState, KycCaseStatus};
    use std::task::{Context, Poll, Waker};

    const SUBMISSION: Uuid = Uuid::from_u128(0x5B);
    const OUTCOMES: [VerificationOutcome; 3] = [
        VerificationOutcome::Pending,
        VerificationOutcome::ProviderPassed,
        VerificationOutcome::ProviderFailed,
    ];

    fn run<F: Future>(future: F) -> F::Output {
        match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(output) => output,
            Poll::Pending => panic!("test doubles complete synchronously"),
        }
    }

    fn external(value: &str) -> ExternalReference {
        ExternalReference::parse(value).unwrap()
    }

    fn report(outcome: VerificationOutcome) -> VerificationReport {
        VerificationReport {
            submission_reference: SUBMISSION,
            external_reference: external("provider-1"),
            outcome,
        }
    }

    #[test]
    fn external_references_are_validated_as_untrusted_data() {
        let longest = "a".repeat(ExternalReference::MAX_CHARS);
        for valid in ["provider-1", "ref with space", "réf-雪", longest.as_str()] {
            assert_eq!(external(valid).as_str(), valid);
        }
        let too_long = "a".repeat(ExternalReference::MAX_CHARS + 1);
        for invalid in ["", "   ", "a\nb", "a\0b", "\u{7f}", too_long.as_str()] {
            assert_eq!(
                ExternalReference::parse(invalid),
                Err(KycProviderError::MalformedResponse),
                "{invalid:?}"
            );
        }
    }

    #[test]
    fn outcomes_lead_at_most_to_recording_a_provider_result() {
        assert_eq!(VerificationOutcome::Pending.case_command(), None);
        for outcome in [
            VerificationOutcome::ProviderPassed,
            VerificationOutcome::ProviderFailed,
        ] {
            assert!(outcome.is_complete());
            assert_eq!(
                outcome.case_command(),
                Some(KycCaseCommand::RecordProviderResult)
            );
        }
    }

    #[test]
    fn a_provider_pass_never_approves_a_case_from_any_status() {
        let owner = Uuid::from_u128(1);
        for outcome in OUTCOMES {
            let Some(command) = outcome.case_command() else {
                continue;
            };
            for status in KycCaseStatus::ALL {
                let state = KycCaseState {
                    owner_id: owner,
                    status,
                };
                let Ok(transition) = state.transition(status, command, KycActor::System) else {
                    continue;
                };
                assert_eq!(
                    (status, transition.to()),
                    (KycCaseStatus::PendingProvider, KycCaseStatus::ManualReview)
                );
                assert!(!transition.to().satisfies_kyc_approval());
            }
        }
    }

    #[test]
    fn only_uncertain_errors_require_reconciliation() {
        use KycProviderError::*;
        for error in [
            Timeout,
            MalformedResponse,
            Unavailable,
            Rejected,
            Unsupported,
            ReferenceConflict,
            NotFound,
        ] {
            assert_eq!(
                error.outcome_unknown(),
                matches!(error, Timeout | MalformedResponse),
                "{error:?}"
            );
        }
    }

    #[test]
    fn reports_must_answer_the_request_that_was_made() {
        let answer = report(VerificationOutcome::ProviderPassed);
        assert_eq!(
            answer.clone().correlate(SUBMISSION, None),
            Ok(answer.clone())
        );
        assert_eq!(
            answer
                .clone()
                .correlate(SUBMISSION, Some(&external("provider-1"))),
            Ok(answer.clone())
        );
        assert_eq!(
            answer.clone().correlate(Uuid::from_u128(0x5C), None),
            Err(KycProviderError::MalformedResponse)
        );
        assert_eq!(
            answer.correlate(SUBMISSION, Some(&external("provider-2"))),
            Err(KycProviderError::MalformedResponse)
        );
    }

    /// A provider that supports nothing, used through the trait object.
    struct NoCapabilities;

    impl IdentityVerificationProvider for NoCapabilities {
        fn submit_verification(
            &self,
            _request: VerificationRequest,
        ) -> ProviderFuture<'_, VerificationReport> {
            Box::pin(async { Err(KycProviderError::Unsupported) })
        }

        fn verification_status(
            &self,
            _submission_reference: Uuid,
        ) -> ProviderFuture<'_, VerificationReport> {
            Box::pin(async { Err(KycProviderError::Unsupported) })
        }
    }

    #[test]
    fn unsupported_operations_are_reported_through_the_trait_object() {
        let provider: &dyn IdentityVerificationProvider = &NoCapabilities;
        let request = VerificationRequest {
            case_id: Uuid::from_u128(0xCA),
            submission_reference: SUBMISSION,
        };
        let submitted = run(provider.submit_verification(request));
        assert_eq!(submitted, Err(KycProviderError::Unsupported));
        let looked_up = run(provider.verification_status(SUBMISSION));
        assert_eq!(looked_up, Err(KycProviderError::Unsupported));
        assert!(!KycProviderError::Unsupported.outcome_unknown());
    }
}
