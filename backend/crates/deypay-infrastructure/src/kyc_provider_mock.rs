//! Deterministic in-memory KYC provider (IMP-P4-004).
//!
//! A controlled simulator for local development and tests (test strategy
//! §27.5). It makes no network calls and holds no credentials or identity data.
//! It touches no database, case, account, restriction, wallet or ledger state;
//! it only answers the provider interface. It can be built only for local and
//! test environments.
//!
//! Its replay and lookup behaviour models what DEYPAY needs from a provider. It
//! is not evidence that any real provider offers it.
use deypay_application::kyc_provider::{
    ExternalReference, IdentityVerificationProvider, KycProviderError, ProviderFuture,
    VerificationOutcome, VerificationReport, VerificationRequest,
};
use deypay_common::config::Environment;
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

/// How the mock answers the first submission for a case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MockScenario {
    /// Accepts and reports `Pending` until [`MockKycProvider::complete`].
    Pending,
    /// Accepts and reports a provider pass at once.
    Passes,
    /// Accepts and reports a provider failure at once.
    Fails,
    /// Answers `Unavailable` and records nothing.
    Unavailable,
    /// Answers `Rejected` and records nothing.
    Rejects,
    /// Answers `Timeout` and records nothing.
    TimesOutBeforeAccepting,
    /// Records a pending submission, then answers `Timeout`.
    TimesOutAfterAccepting,
    /// Records a pending submission, then answers `MalformedResponse`.
    MalformedAfterAccepting,
}

pub struct MockKycProvider {
    state: Mutex<State>,
    status_lookup: bool,
}

#[derive(Default)]
struct State {
    scenarios: HashMap<Uuid, MockScenario>,
    records: HashMap<Uuid, Record>,
    submit_calls: usize,
}

struct Record {
    case_id: Uuid,
    report: VerificationReport,
}

impl MockKycProvider {
    /// Builds a mock for local and test environments only (SEC-PRV-003).
    /// Every case answers [`MockScenario::Pending`] unless scripted.
    pub fn new(environment: Environment) -> Option<Self> {
        matches!(environment, Environment::Local | Environment::Test).then(|| Self {
            state: Mutex::default(),
            status_lookup: true,
        })
    }

    /// Simulates a provider that offers no status lookup.
    pub fn without_status_lookup(mut self) -> Self {
        self.status_lookup = false;
        self
    }

    /// Sets how submissions for `case_id` are answered.
    pub fn script(&self, case_id: Uuid, scenario: MockScenario) {
        if let Ok(mut state) = self.state.lock() {
            state.scenarios.insert(case_id, scenario);
        }
    }

    /// Completes a pending verification, as a provider would later. Returns
    /// `None` for an unknown reference, a verification that is not pending, or
    /// a `Pending` outcome.
    pub fn complete(
        &self,
        submission_reference: Uuid,
        outcome: VerificationOutcome,
    ) -> Option<VerificationReport> {
        let mut state = self.state.lock().ok()?;
        let record = state.records.get_mut(&submission_reference)?;
        if !outcome.is_complete() || record.report.outcome.is_complete() {
            return None;
        }
        record.report.outcome = outcome;
        Some(record.report.clone())
    }

    /// Submissions the provider recorded. Replays and refusals add none.
    pub fn recorded_submissions(&self) -> usize {
        self.state.lock().map_or(0, |state| state.records.len())
    }

    /// Submit calls received, including replays and refusals.
    pub fn submit_calls(&self) -> usize {
        self.state.lock().map_or(0, |state| state.submit_calls)
    }

    fn submit(&self, request: VerificationRequest) -> Result<VerificationReport, KycProviderError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| KycProviderError::Unavailable)?;
        state.submit_calls += 1;
        if let Some(record) = state.records.get(&request.submission_reference) {
            // A replay returns the recorded report and never records twice.
            return if record.case_id == request.case_id {
                Ok(record.report.clone())
            } else {
                Err(KycProviderError::ReferenceConflict)
            };
        }
        let scenario = state
            .scenarios
            .get(&request.case_id)
            .copied()
            .unwrap_or(MockScenario::Pending);
        let outcome = match scenario {
            MockScenario::Unavailable => return Err(KycProviderError::Unavailable),
            MockScenario::Rejects => return Err(KycProviderError::Rejected),
            MockScenario::TimesOutBeforeAccepting => return Err(KycProviderError::Timeout),
            MockScenario::Passes => VerificationOutcome::ProviderPassed,
            MockScenario::Fails => VerificationOutcome::ProviderFailed,
            MockScenario::Pending
            | MockScenario::TimesOutAfterAccepting
            | MockScenario::MalformedAfterAccepting => VerificationOutcome::Pending,
        };
        let report = VerificationReport {
            submission_reference: request.submission_reference,
            external_reference: ExternalReference::parse(&format!(
                "mock-kyc-{}",
                state.records.len() + 1
            ))?,
            outcome,
        };
        state.records.insert(
            request.submission_reference,
            Record {
                case_id: request.case_id,
                report: report.clone(),
            },
        );
        match scenario {
            MockScenario::TimesOutAfterAccepting => Err(KycProviderError::Timeout),
            MockScenario::MalformedAfterAccepting => Err(KycProviderError::MalformedResponse),
            _ => Ok(report),
        }
    }

    fn status(&self, submission_reference: Uuid) -> Result<VerificationReport, KycProviderError> {
        if !self.status_lookup {
            return Err(KycProviderError::Unsupported);
        }
        let state = self
            .state
            .lock()
            .map_err(|_| KycProviderError::Unavailable)?;
        state
            .records
            .get(&submission_reference)
            .map(|record| record.report.clone())
            .ok_or(KycProviderError::NotFound)
    }
}

impl IdentityVerificationProvider for MockKycProvider {
    fn submit_verification(
        &self,
        request: VerificationRequest,
    ) -> ProviderFuture<'_, VerificationReport> {
        Box::pin(async move { self.submit(request) })
    }

    fn verification_status(
        &self,
        submission_reference: Uuid,
    ) -> ProviderFuture<'_, VerificationReport> {
        Box::pin(async move { self.status(submission_reference) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deypay_domain::kyc::{KycActor, KycCaseState, KycCaseStatus};
    use std::sync::Arc;

    const CASE: Uuid = Uuid::from_u128(0xCA);
    const OTHER_CASE: Uuid = Uuid::from_u128(0xCB);
    const SUBMISSION: Uuid = Uuid::from_u128(0x5B);

    const REQUEST: VerificationRequest = VerificationRequest {
        case_id: CASE,
        submission_reference: SUBMISSION,
    };

    fn mock() -> MockKycProvider {
        MockKycProvider::new(Environment::Test).unwrap()
    }

    fn scripted(scenario: MockScenario) -> MockKycProvider {
        let provider = mock();
        provider.script(CASE, scenario);
        provider
    }

    fn report(outcome: VerificationOutcome) -> VerificationReport {
        VerificationReport {
            submission_reference: SUBMISSION,
            external_reference: ExternalReference::parse("mock-kyc-1").unwrap(),
            outcome,
        }
    }

    #[test]
    fn only_local_and_test_environments_can_build_the_mock() {
        assert!(MockKycProvider::new(Environment::Local).is_some());
        assert!(MockKycProvider::new(Environment::Test).is_some());
        assert!(MockKycProvider::new(Environment::Staging).is_none());
        assert!(MockKycProvider::new(Environment::Production).is_none());
    }

    #[tokio::test]
    async fn scripted_outcomes_are_deterministic() {
        for (scenario, outcome) in [
            (MockScenario::Pending, VerificationOutcome::Pending),
            (MockScenario::Passes, VerificationOutcome::ProviderPassed),
            (MockScenario::Fails, VerificationOutcome::ProviderFailed),
        ] {
            for _ in 0..2 {
                let provider = scripted(scenario);
                let answer = provider.submit_verification(REQUEST).await;
                assert_eq!(answer, Ok(report(outcome)));
                let looked_up = provider.verification_status(SUBMISSION).await;
                assert_eq!(looked_up, Ok(report(outcome)));
            }
        }
    }

    #[tokio::test]
    async fn pending_verifications_complete_only_when_told() {
        let provider = mock();
        provider.submit_verification(REQUEST).await.unwrap();
        assert_eq!(
            provider.complete(SUBMISSION, VerificationOutcome::Pending),
            None
        );
        assert_eq!(
            provider.complete(Uuid::from_u128(0xFF), VerificationOutcome::ProviderPassed),
            None
        );
        let completed = provider.complete(SUBMISSION, VerificationOutcome::ProviderFailed);
        assert_eq!(completed, Some(report(VerificationOutcome::ProviderFailed)));
        // A completed verification does not change again.
        assert_eq!(
            provider.complete(SUBMISSION, VerificationOutcome::ProviderPassed),
            None
        );
        let looked_up = provider.verification_status(SUBMISSION).await;
        assert_eq!(looked_up, Ok(report(VerificationOutcome::ProviderFailed)));
    }

    #[tokio::test]
    async fn refusals_record_nothing() {
        for (scenario, error) in [
            (MockScenario::Unavailable, KycProviderError::Unavailable),
            (MockScenario::Rejects, KycProviderError::Rejected),
            (
                MockScenario::TimesOutBeforeAccepting,
                KycProviderError::Timeout,
            ),
        ] {
            let provider = scripted(scenario);
            assert_eq!(provider.submit_verification(REQUEST).await, Err(error));
            assert_eq!(provider.recorded_submissions(), 0);
            let looked_up = provider.verification_status(SUBMISSION).await;
            assert_eq!(looked_up, Err(KycProviderError::NotFound));
        }
    }

    #[tokio::test]
    async fn uncertain_outcomes_are_resolved_by_lookup_not_resubmission() {
        for (scenario, error) in [
            (
                MockScenario::TimesOutAfterAccepting,
                KycProviderError::Timeout,
            ),
            (
                MockScenario::MalformedAfterAccepting,
                KycProviderError::MalformedResponse,
            ),
        ] {
            let provider = scripted(scenario);
            let answer = provider.submit_verification(REQUEST).await;
            assert_eq!(answer, Err(error));
            assert!(error.outcome_unknown());
            let looked_up = provider.verification_status(SUBMISSION).await.unwrap();
            assert_eq!(
                looked_up.correlate(SUBMISSION, None),
                Ok(report(VerificationOutcome::Pending))
            );
            assert_eq!(provider.recorded_submissions(), 1);
        }
    }

    #[tokio::test]
    async fn replays_return_the_recorded_report_without_a_second_record() {
        let provider = scripted(MockScenario::TimesOutAfterAccepting);
        let first = provider.submit_verification(REQUEST).await;
        assert_eq!(first, Err(KycProviderError::Timeout));
        let replay = provider.submit_verification(REQUEST).await;
        assert_eq!(replay, Ok(report(VerificationOutcome::Pending)));
        let again = provider.submit_verification(REQUEST).await;
        assert_eq!(again, Ok(report(VerificationOutcome::Pending)));
        assert_eq!(provider.recorded_submissions(), 1);
        assert_eq!(provider.submit_calls(), 3);
    }

    #[tokio::test]
    async fn a_reference_reused_for_another_case_conflicts() {
        let provider = scripted(MockScenario::Passes);
        provider.submit_verification(REQUEST).await.unwrap();
        let reused = VerificationRequest {
            case_id: OTHER_CASE,
            ..REQUEST
        };
        let answer = provider.submit_verification(reused).await;
        assert_eq!(answer, Err(KycProviderError::ReferenceConflict));
        let looked_up = provider.verification_status(SUBMISSION).await;
        assert_eq!(looked_up, Ok(report(VerificationOutcome::ProviderPassed)));
        assert_eq!(provider.recorded_submissions(), 1);
    }

    #[tokio::test]
    async fn status_lookup_can_be_unsupported() {
        let provider = mock().without_status_lookup();
        provider.submit_verification(REQUEST).await.unwrap();
        let looked_up = provider.verification_status(SUBMISSION).await;
        assert_eq!(looked_up, Err(KycProviderError::Unsupported));
        assert!(!KycProviderError::Unsupported.outcome_unknown());
    }

    #[tokio::test]
    async fn mock_state_is_isolated_per_instance() {
        let first = scripted(MockScenario::Passes);
        let second = mock();
        first.submit_verification(REQUEST).await.unwrap();
        let unseen = second.verification_status(SUBMISSION).await;
        assert_eq!(unseen, Err(KycProviderError::NotFound));
        // The second mock does not inherit the first one's script or numbering.
        let answer = second.submit_verification(REQUEST).await;
        assert_eq!(answer, Ok(report(VerificationOutcome::Pending)));
        assert_eq!((first.submit_calls(), second.submit_calls()), (1, 1));
    }

    #[tokio::test]
    async fn provider_results_never_approve_a_case() {
        // Used through the trait object, as a business service would.
        let provider: Arc<dyn IdentityVerificationProvider> =
            Arc::new(scripted(MockScenario::Passes));
        let answer = provider.submit_verification(REQUEST).await.unwrap();
        let command = answer.outcome.case_command().unwrap();
        let mut accepted = Vec::new();
        for status in KycCaseStatus::ALL {
            let state = KycCaseState {
                owner_id: Uuid::from_u128(1),
                status,
            };
            if let Ok(transition) = state.transition(status, command, KycActor::System) {
                accepted.push((status, transition.to()));
            }
        }
        assert_eq!(
            accepted,
            [(KycCaseStatus::PendingProvider, KycCaseStatus::ManualReview)]
        );
    }
}
