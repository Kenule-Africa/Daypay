//! KYC case application service (IMP-P4-002).
//!
//! Coordinates customer case operations through the IMP-P4-001 state machine
//! and the ports below. It implements case retrieval, opening a first `DRAFT`
//! case and customer submission only. Provider processing, reviewer decisions,
//! audit events, account status and financial eligibility are out of scope, so
//! no operation here can approve, reject or otherwise decide a case.
//!
//! Callers must take `customer_id` from the authenticated session, never from
//! request input.
//!
//! Provisional fail-closed boundaries, pending the open KYC policy decisions:
//!
//! - A customer may open only a first case. A second case, whether for
//!   re-verification, resubmission after rejection or in parallel, is refused.
//! - Submission requires an approved set of submission requirements. None
//!   exists yet, so [`NoApprovedRequirements`] refuses every submission.
//!
//! Not yet guaranteed, because no PostgreSQL adapter implements
//! [`KycCaseStore`]:
//!
//! - The runtime role can only read `compliance.kyc_cases`. Writes need a
//!   reviewed grant change.
//! - No database constraint limits a customer to one case. The adapter must
//!   make [`KycCaseStore::insert_first_case`] atomic per customer.
//! - Submission writes no audit event (IMP-P4-008).
use crate::repositories::{KycRepository, RepositoryError};
use deypay_domain::entities::KycCase;
use deypay_domain::kyc::{
    KycActor, KycCaseCommand, KycCaseState, KycCaseStatus, KycCaseTransition, KycTransitionError,
};
use std::future::Future;
use uuid::Uuid;

/// What a customer may see about their own case.
///
/// It deliberately carries no financial-access flag. Financial eligibility
/// belongs to the central eligibility check (architecture §25.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KycCaseView {
    pub case_id: Uuid,
    pub status: KycCaseStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KycServiceError {
    /// No case with this ID belongs to the customer.
    NotFound,
    /// The customer already has a case. Opening another depends on open policy.
    CaseAlreadyExists,
    /// The state machine refused the change because it is stale, terminal or
    /// invalid. It never carries `Unauthorized`, which is reported as `NotFound`.
    Rejected(KycTransitionError),
    /// Another change to the case was committed first.
    ConcurrentModification,
    /// The case lacks information or documents required for submission.
    SubmissionIncomplete,
    /// No approved submission requirements exist, so submission fails closed.
    SubmissionRequirementsUndefined,
    /// Stored or returned data failed validation.
    InvalidRecord,
    /// Storage was unavailable.
    Unavailable,
}

impl From<RepositoryError> for KycServiceError {
    fn from(error: RepositoryError) -> Self {
        match error {
            RepositoryError::Conflict => Self::ConcurrentModification,
            RepositoryError::StorageUnavailable => Self::Unavailable,
            RepositoryError::InvalidRecord => Self::InvalidRecord,
        }
    }
}

/// Write port for KYC cases. Every write is scoped to the case owner.
pub trait KycCaseStore: KycRepository {
    /// Inserts a case with status [`KycCaseStatus::INITIAL`] for a customer
    /// who has no case, atomically with that check. Returns `None`, and writes
    /// nothing, if the customer already has a case, including under concurrent
    /// calls.
    fn insert_first_case(
        &self,
        customer_id: Uuid,
    ) -> impl Future<Output = Result<Option<KycCase>, RepositoryError>> + Send;

    /// Sets the status to `transition.to()` only where the case has `case_id`,
    /// belongs to `owner_id` and still has `transition.from()`. Returns
    /// `false`, and writes nothing, when no case matches.
    fn apply_transition(
        &self,
        owner_id: Uuid,
        case_id: Uuid,
        transition: KycCaseTransition,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmissionReadiness {
    /// Everything required for submission is present.
    Ready,
    /// The customer has not yet provided everything required.
    Incomplete,
    /// No approved requirement set exists.
    Undefined,
}

/// Decides whether a case holds the information and documents required before
/// submission (FRS §9.2, API §§17.3–17.5).
///
/// Readiness is checked before the conditional write, not atomically with it.
/// Once documents can change, the persisting adapter must re-check or lock them
/// in the same transaction.
pub trait SubmissionRequirements {
    fn readiness(
        &self,
        owner_id: Uuid,
        case_id: Uuid,
    ) -> impl Future<Output = Result<SubmissionReadiness, RepositoryError>> + Send;
}

/// The only requirement source available today. The required data and
/// documents are not approved, so every submission fails closed.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoApprovedRequirements;

impl SubmissionRequirements for NoApprovedRequirements {
    fn readiness(
        &self,
        _owner_id: Uuid,
        _case_id: Uuid,
    ) -> impl Future<Output = Result<SubmissionReadiness, RepositoryError>> + Send {
        std::future::ready(Ok(SubmissionReadiness::Undefined))
    }
}

/// Returns the customer's own case. A case owned by anyone else is reported as
/// missing.
pub async fn customer_case(
    cases: &impl KycRepository,
    customer_id: Uuid,
    case_id: Uuid,
) -> Result<KycCaseView, KycServiceError> {
    let (_, view) = load_owned(cases, customer_id, case_id).await?;
    Ok(view)
}

/// Opens the customer's first case in `DRAFT` (FR-KYC-001, API §17.2).
pub async fn open_first_case(
    store: &impl KycCaseStore,
    customer_id: Uuid,
) -> Result<KycCaseView, KycServiceError> {
    let case = store
        .insert_first_case(customer_id)
        .await?
        .ok_or(KycServiceError::CaseAlreadyExists)?;
    let (_, view) = owned_view(&case, customer_id, case.id)?;
    if view.status != KycCaseStatus::INITIAL {
        return Err(KycServiceError::InvalidRecord);
    }
    Ok(view)
}

/// Submits a draft, or resubmits after a request for information.
///
/// `expected` is the status the customer acted on, when the request carries
/// one. Either way the write is conditional on the status that was loaded.
pub async fn submit_case(
    store: &impl KycCaseStore,
    requirements: &impl SubmissionRequirements,
    customer_id: Uuid,
    case_id: Uuid,
    expected: Option<KycCaseStatus>,
) -> Result<KycCaseView, KycServiceError> {
    let (state, _) = load_owned(store, customer_id, case_id).await?;
    let transition = state
        .transition(
            expected.unwrap_or(state.status),
            KycCaseCommand::Submit,
            KycActor::Customer { customer_id },
        )
        .map_err(rejected)?;
    match requirements.readiness(customer_id, case_id).await? {
        SubmissionReadiness::Ready => {}
        SubmissionReadiness::Incomplete => return Err(KycServiceError::SubmissionIncomplete),
        SubmissionReadiness::Undefined => {
            return Err(KycServiceError::SubmissionRequirementsUndefined);
        }
    }
    if !store
        .apply_transition(customer_id, case_id, transition)
        .await?
    {
        return Err(KycServiceError::ConcurrentModification);
    }
    Ok(KycCaseView {
        case_id,
        status: transition.to(),
    })
}

async fn load_owned(
    cases: &impl KycRepository,
    customer_id: Uuid,
    case_id: Uuid,
) -> Result<(KycCaseState, KycCaseView), KycServiceError> {
    let case = cases
        .kyc_case(customer_id, case_id)
        .await?
        .ok_or(KycServiceError::NotFound)?;
    owned_view(&case, customer_id, case_id)
}

/// Validates a stored case before anything about it is used or returned.
fn owned_view(
    case: &KycCase,
    customer_id: Uuid,
    case_id: Uuid,
) -> Result<(KycCaseState, KycCaseView), KycServiceError> {
    if case.user_id != customer_id || case.id != case_id {
        return Err(KycServiceError::InvalidRecord);
    }
    let state = KycCaseState::try_from(case).map_err(|_| KycServiceError::InvalidRecord)?;
    Ok((
        state,
        KycCaseView {
            case_id,
            status: state.status,
        },
    ))
}

/// The case was loaded for its owner, so `Unauthorized` should not occur. If it
/// does, report the case as missing rather than reveal anything about it.
fn rejected(error: KycTransitionError) -> KycServiceError {
    match error {
        KycTransitionError::Unauthorized { .. } => KycServiceError::NotFound,
        other => KycServiceError::Rejected(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repositories::Lookup;
    use KycCaseCommand::Submit;
    use KycCaseStatus::*;
    use KycServiceError as E;
    use std::future::ready;
    use std::pin::pin;
    use std::sync::Mutex;
    use std::task::{Context, Poll, Waker};

    const CUSTOMER: Uuid = Uuid::from_u128(0xC1);
    const OTHER: Uuid = Uuid::from_u128(0xC2);
    const CASE: Uuid = Uuid::from_u128(0xA1);
    const MISSING: Uuid = Uuid::from_u128(0xFF);

    /// The test doubles complete immediately, so one poll is enough.
    fn run<F: Future>(future: F) -> F::Output {
        match pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(output) => output,
            Poll::Pending => panic!("test doubles complete synchronously"),
        }
    }

    #[derive(Default)]
    struct FakeStore {
        cases: Mutex<Vec<KycCase>>,
        writes: Mutex<Vec<KycCaseTransition>>,
        read_error: Option<RepositoryError>,
        write_error: Option<RepositoryError>,
        /// A faulty adapter that returns this row for every lookup.
        read_override: Option<KycCase>,
        /// A faulty adapter that reports this status for an inserted case.
        inserted_status: Option<&'static str>,
        /// Another writer commits this status just before our write.
        concurrent_status: Option<&'static str>,
    }

    impl FakeStore {
        fn with(status: &str) -> Self {
            Self {
                cases: Mutex::new(vec![KycCase {
                    id: CASE,
                    user_id: CUSTOMER,
                    status: status.to_owned(),
                }]),
                ..Self::default()
            }
        }

        fn status(&self, case_id: Uuid) -> String {
            let cases = self.cases.lock().unwrap();
            cases
                .iter()
                .find(|c| c.id == case_id)
                .unwrap()
                .status
                .clone()
        }

        fn case_count(&self) -> usize {
            self.cases.lock().unwrap().len()
        }

        fn writes(&self) -> Vec<KycCaseTransition> {
            self.writes.lock().unwrap().clone()
        }
    }

    impl KycRepository for FakeStore {
        fn kyc_case(
            &self,
            customer_id: Uuid,
            case_id: Uuid,
        ) -> impl Future<Output = Lookup<KycCase>> + Send {
            let found = || {
                let cases = self.cases.lock().unwrap();
                let scoped = cases
                    .iter()
                    .find(|c| c.id == case_id && c.user_id == customer_id);
                scoped.cloned()
            };
            ready(match (self.read_error, &self.read_override) {
                (Some(error), _) => Err(error),
                (None, Some(row)) => Ok(Some(row.clone())),
                (None, None) => Ok(found()),
            })
        }
    }

    impl KycCaseStore for FakeStore {
        fn insert_first_case(
            &self,
            customer_id: Uuid,
        ) -> impl Future<Output = Result<Option<KycCase>, RepositoryError>> + Send {
            let insert = || {
                let mut cases = self.cases.lock().unwrap();
                if cases.iter().any(|c| c.user_id == customer_id) {
                    return None;
                }
                let case = KycCase {
                    id: Uuid::from_u128(0x100 + cases.len() as u128),
                    user_id: customer_id,
                    status: self.inserted_status.unwrap_or("DRAFT").to_owned(),
                };
                cases.push(case.clone());
                Some(case)
            };
            ready(match self.write_error {
                Some(error) => Err(error),
                None => Ok(insert()),
            })
        }

        fn apply_transition(
            &self,
            owner_id: Uuid,
            case_id: Uuid,
            transition: KycCaseTransition,
        ) -> impl Future<Output = Result<bool, RepositoryError>> + Send {
            let compare_and_set = || {
                let mut cases = self.cases.lock().unwrap();
                let Some(case) = cases
                    .iter_mut()
                    .find(|c| c.id == case_id && c.user_id == owner_id)
                else {
                    return false;
                };
                if let Some(status) = self.concurrent_status {
                    case.status = status.to_owned();
                }
                if case.status != transition.from().as_str() {
                    return false;
                }
                case.status = transition.to().as_str().to_owned();
                self.writes.lock().unwrap().push(transition);
                true
            };
            ready(match self.write_error {
                Some(error) => Err(error),
                None => Ok(compare_and_set()),
            })
        }
    }

    struct Requirements(Result<SubmissionReadiness, RepositoryError>);

    impl SubmissionRequirements for Requirements {
        fn readiness(
            &self,
            _owner_id: Uuid,
            _case_id: Uuid,
        ) -> impl Future<Output = Result<SubmissionReadiness, RepositoryError>> + Send {
            ready(self.0)
        }
    }

    const READY: Requirements = Requirements(Ok(SubmissionReadiness::Ready));

    fn submit(
        store: &FakeStore,
        customer_id: Uuid,
        expected: Option<KycCaseStatus>,
    ) -> Result<KycCaseView, KycServiceError> {
        run(submit_case(store, &READY, customer_id, CASE, expected))
    }

    #[test]
    fn owners_read_their_case_in_every_status() {
        for status in KycCaseStatus::ALL {
            let store = FakeStore::with(status.as_str());
            assert_eq!(
                run(customer_case(&store, CUSTOMER, CASE)),
                Ok(KycCaseView {
                    case_id: CASE,
                    status
                })
            );
        }
    }

    #[test]
    fn other_customers_and_missing_cases_are_not_found() {
        let store = FakeStore::with("MANUAL_REVIEW");
        assert_eq!(run(customer_case(&store, OTHER, CASE)), Err(E::NotFound));
        assert_eq!(
            run(customer_case(&store, CUSTOMER, MISSING)),
            Err(E::NotFound)
        );
    }

    #[test]
    fn invalid_stored_statuses_fail_closed() {
        for value in ["", "approved", " DRAFT", "NOT_STARTED", "KYC_APPROVED"] {
            let store = FakeStore::with(value);
            assert_eq!(
                run(customer_case(&store, CUSTOMER, CASE)),
                Err(E::InvalidRecord)
            );
            assert_eq!(submit(&store, CUSTOMER, None), Err(E::InvalidRecord));
            assert!(store.writes().is_empty());
        }
    }

    #[test]
    fn rows_for_another_owner_or_case_are_never_used() {
        let foreign = KycCase {
            id: CASE,
            user_id: OTHER,
            status: "DRAFT".to_owned(),
        };
        let wrong_case = KycCase {
            id: MISSING,
            user_id: CUSTOMER,
            status: "DRAFT".to_owned(),
        };
        for row in [foreign, wrong_case] {
            let store = FakeStore {
                read_override: Some(row),
                ..FakeStore::with("DRAFT")
            };
            assert_eq!(
                run(customer_case(&store, CUSTOMER, CASE)),
                Err(E::InvalidRecord)
            );
            assert_eq!(submit(&store, CUSTOMER, None), Err(E::InvalidRecord));
            assert!(store.writes().is_empty());
        }
    }

    #[test]
    fn repository_errors_map_without_partial_writes() {
        for (error, expected) in [
            (RepositoryError::StorageUnavailable, E::Unavailable),
            (RepositoryError::InvalidRecord, E::InvalidRecord),
            (RepositoryError::Conflict, E::ConcurrentModification),
        ] {
            let store = FakeStore {
                read_error: Some(error),
                ..FakeStore::with("DRAFT")
            };
            assert_eq!(run(customer_case(&store, CUSTOMER, CASE)), Err(expected));
            assert_eq!(submit(&store, CUSTOMER, None), Err(expected));

            let store = FakeStore {
                write_error: Some(error),
                ..FakeStore::with("DRAFT")
            };
            assert_eq!(submit(&store, CUSTOMER, None), Err(expected));
            assert_eq!(run(open_first_case(&store, OTHER)), Err(expected));
            assert_eq!(store.status(CASE), "DRAFT");
            assert_eq!(store.case_count(), 1);
        }
    }

    #[test]
    fn a_first_case_opens_as_an_unapproved_owned_draft() {
        let store = FakeStore::default();
        let view = run(open_first_case(&store, CUSTOMER)).unwrap();
        assert_eq!(view.status, Draft);
        assert!(!view.status.satisfies_kyc_approval());
        assert_eq!(run(customer_case(&store, CUSTOMER, view.case_id)), Ok(view));
        assert_eq!(
            run(customer_case(&store, OTHER, view.case_id)),
            Err(E::NotFound)
        );
    }

    #[test]
    fn repeated_open_requests_create_no_second_case() {
        let store = FakeStore::default();
        let first = run(open_first_case(&store, CUSTOMER)).unwrap();
        assert_eq!(
            run(open_first_case(&store, CUSTOMER)),
            Err(E::CaseAlreadyExists)
        );
        assert_eq!(store.case_count(), 1);
        let other = run(open_first_case(&store, OTHER)).unwrap();
        assert_ne!(other.case_id, first.case_id);
    }

    #[test]
    fn no_case_opens_beside_an_existing_case_in_any_status() {
        // Re-verification and resubmission after rejection remain undecided,
        // so a rejected, expired or approved customer cannot start over.
        for status in KycCaseStatus::ALL {
            let store = FakeStore::with(status.as_str());
            assert_eq!(
                run(open_first_case(&store, CUSTOMER)),
                Err(E::CaseAlreadyExists)
            );
            assert_eq!(store.case_count(), 1);
            assert_eq!(store.status(CASE), status.as_str());
        }
    }

    #[test]
    fn an_inserted_case_that_is_not_a_draft_is_rejected() {
        for status in ["APPROVED", "SUBMITTED", "KYC_APPROVED"] {
            let store = FakeStore {
                inserted_status: Some(status),
                ..FakeStore::default()
            };
            assert_eq!(
                run(open_first_case(&store, CUSTOMER)),
                Err(E::InvalidRecord)
            );
        }
    }

    #[test]
    fn drafts_and_information_requests_submit_exactly_once() {
        for from in [Draft, InformationRequested] {
            for expected in [None, Some(from)] {
                let store = FakeStore::with(from.as_str());
                assert_eq!(
                    submit(&store, CUSTOMER, expected),
                    Ok(KycCaseView {
                        case_id: CASE,
                        status: Submitted
                    })
                );
                assert_eq!(store.status(CASE), "SUBMITTED");
                let writes = store.writes();
                assert_eq!(writes.len(), 1);
                assert_eq!(
                    (writes[0].from(), writes[0].to(), writes[0].command()),
                    (from, Submitted, Submit)
                );
            }
        }
    }

    #[test]
    fn other_statuses_cannot_be_submitted() {
        for status in KycCaseStatus::ALL {
            if matches!(status, Draft | InformationRequested) {
                continue;
            }
            let store = FakeStore::with(status.as_str());
            let refusal = if status.is_terminal() {
                KycTransitionError::TerminalCase { status }
            } else {
                KycTransitionError::InvalidTransition {
                    from: status,
                    command: Submit,
                }
            };
            assert_eq!(submit(&store, CUSTOMER, None), Err(E::Rejected(refusal)));
            assert_eq!(store.status(CASE), status.as_str());
            assert!(store.writes().is_empty());
        }
    }

    #[test]
    fn customers_cannot_submit_cases_they_do_not_own() {
        let store = FakeStore::with("DRAFT");
        assert_eq!(submit(&store, OTHER, None), Err(E::NotFound));
        assert_eq!(
            run(submit_case(&store, &READY, CUSTOMER, MISSING, None)),
            Err(E::NotFound)
        );
        assert_eq!(store.status(CASE), "DRAFT");
        assert!(store.writes().is_empty());
    }

    #[test]
    fn a_stale_expected_status_is_rejected_without_writing() {
        let store = FakeStore::with("INFORMATION_REQUESTED");
        assert_eq!(
            submit(&store, CUSTOMER, Some(Draft)),
            Err(E::Rejected(KycTransitionError::StaleStatus {
                expected: Draft,
                actual: InformationRequested
            }))
        );
        assert!(store.writes().is_empty());
    }

    #[test]
    fn a_concurrent_change_is_not_overwritten() {
        // Another request commits between our read and our conditional write.
        for concurrent in ["SUBMITTED", "EXPIRED"] {
            let store = FakeStore {
                concurrent_status: Some(concurrent),
                ..FakeStore::with("DRAFT")
            };
            assert_eq!(
                submit(&store, CUSTOMER, Some(Draft)),
                Err(E::ConcurrentModification)
            );
            assert_eq!(store.status(CASE), concurrent);
            assert!(store.writes().is_empty());
        }
    }

    #[test]
    fn repeated_submissions_write_once() {
        let store = FakeStore::with("DRAFT");
        assert!(submit(&store, CUSTOMER, Some(Draft)).is_ok());
        assert_eq!(
            submit(&store, CUSTOMER, Some(Draft)),
            Err(E::Rejected(KycTransitionError::StaleStatus {
                expected: Draft,
                actual: Submitted
            }))
        );
        assert_eq!(
            submit(&store, CUSTOMER, None),
            Err(E::Rejected(KycTransitionError::InvalidTransition {
                from: Submitted,
                command: Submit
            }))
        );
        assert_eq!(store.writes().len(), 1);
    }

    #[test]
    fn submission_fails_closed_without_satisfied_requirements() {
        for (readiness, expected) in [
            (Ok(SubmissionReadiness::Incomplete), E::SubmissionIncomplete),
            (
                Ok(SubmissionReadiness::Undefined),
                E::SubmissionRequirementsUndefined,
            ),
            (Err(RepositoryError::StorageUnavailable), E::Unavailable),
        ] {
            let store = FakeStore::with("DRAFT");
            let requirements = Requirements(readiness);
            assert_eq!(
                run(submit_case(&store, &requirements, CUSTOMER, CASE, None)),
                Err(expected)
            );
            assert_eq!(store.status(CASE), "DRAFT");
            assert!(store.writes().is_empty());
        }
    }

    #[test]
    fn the_shipped_requirement_source_refuses_every_submission() {
        for status in [Draft, InformationRequested] {
            let store = FakeStore::with(status.as_str());
            assert_eq!(
                run(submit_case(
                    &store,
                    &NoApprovedRequirements,
                    CUSTOMER,
                    CASE,
                    None
                )),
                Err(E::SubmissionRequirementsUndefined)
            );
            assert_eq!(store.status(CASE), status.as_str());
            assert!(store.writes().is_empty());
        }
    }

    #[test]
    fn no_operation_decides_a_case() {
        for status in KycCaseStatus::ALL {
            let store = FakeStore::with(status.as_str());
            let _ = run(open_first_case(&store, CUSTOMER));
            let _ = run(open_first_case(&store, OTHER));
            for expected in [None, Some(status)] {
                let _ = submit(&store, CUSTOMER, expected);
            }
            for write in store.writes() {
                assert_eq!((write.command(), write.to()), (Submit, Submitted));
            }
            let after: KycCaseStatus = store.status(CASE).parse().unwrap();
            assert_eq!(after.satisfies_kyc_approval(), status == Approved);
            assert_eq!(after.is_terminal(), status.is_terminal());
            let cases = store.cases.lock().unwrap();
            assert!(
                cases
                    .iter()
                    .filter(|c| c.id != CASE)
                    .all(|c| c.status == "DRAFT")
            );
        }
    }
}
