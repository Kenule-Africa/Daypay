//! KYC case state machine (IMP-P4-001).
//!
//! The statuses mirror the `compliance.kyc_cases.status` CHECK constraint
//! (database design §10.1). This module only validates status changes. It does
//! not persist cases, call providers, check reviewer roles or decide financial
//! eligibility.
//!
//! Permitted transitions:
//!
//! | From                    | Command                | Actor               | To                      |
//! |-------------------------|------------------------|---------------------|-------------------------|
//! | `DRAFT`                 | `Submit`               | owning customer     | `SUBMITTED`             |
//! | `INFORMATION_REQUESTED` | `Submit`               | owning customer     | `SUBMITTED`             |
//! | `SUBMITTED`             | `DispatchToProvider`   | system              | `PENDING_PROVIDER`      |
//! | `PENDING_PROVIDER`      | `RecordProviderResult` | system              | `MANUAL_REVIEW`         |
//! | `MANUAL_REVIEW`         | `Approve`              | non-owning reviewer | `APPROVED`              |
//! | `MANUAL_REVIEW`         | `Reject`               | non-owning reviewer | `REJECTED`              |
//! | `MANUAL_REVIEW`         | `RequestInformation`   | non-owning reviewer | `INFORMATION_REQUESTED` |
//!
//! `APPROVED`, `REJECTED` and `EXPIRED` are terminal. Every other combination
//! is rejected. No transition enters `EXPIRED`, because the specifications do
//! not define what expires or when.
//!
//! A provider result never approves or rejects a case. It moves the case to
//! manual review, because DEYPAY owns KYC decisions and only a permitted
//! reviewer approves, rejects or requests information (database design §10.1,
//! API §32.2). Approval is reachable only through a reviewer decision. No
//! command represents email or contact verification.
//!
//! Sending every provider result to manual review is a **provisional,
//! conservative policy**. No approved specification requires it, or defines
//! how provider outcomes map to case decisions (DEC-006 is open). Compliance and
//! Product must approve or replace it.
//!
//! # Application-layer requirements
//!
//! [`KycActor`] and [`KycCaseState`] are claims made by the caller. They are not
//! proof of authentication, permission or stored state. Before persisting a
//! [`KycCaseTransition`], the application must:
//!
//! 1. Build `Customer` only from the authenticated session's customer ID, and
//!    load the case scoped to that customer.
//! 2. Build `Reviewer` only for an authenticated administrative identity that
//!    holds the KYC review permission (FRS §39, SEC-AZ-003, SEC-ADM-001/002).
//!    Take `reviewer_id` from that identity, never from request input.
//! 3. Build `System` only in internal orchestration. A provider result must
//!    come from a verified, deduplicated webhook or poll (BR-017).
//! 4. Take `owner_id` and the current status from the stored case, and the
//!    expected status from the reviewer's request (API §32.2).
//! 5. Update the case only where its status still equals
//!    [`KycCaseTransition::from`]. If no row changes, treat the request as
//!    stale. Commit the reviewer, reason, previous and new status, and an
//!    append-only audit event in the same database transaction (database
//!    design §10.1, test strategy §16.4).
//! 6. Return HTTP 409 for stale or terminal-case decisions (API §32.2).
//! 7. Never grant financial access from
//!    [`KycCaseStatus::satisfies_kyc_approval`] alone. Use the central
//!    eligibility check (architecture §25.1).
use crate::entities::KycCase;
use std::str::FromStr;
use uuid::Uuid;

/// Persisted status of one `compliance.kyc_cases` row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KycCaseStatus {
    Draft,
    Submitted,
    PendingProvider,
    ManualReview,
    InformationRequested,
    Approved,
    Rejected,
    Expired,
}

impl KycCaseStatus {
    /// Every status accepted by the database CHECK constraint.
    pub const ALL: [Self; 8] = [
        Self::Draft,
        Self::Submitted,
        Self::PendingProvider,
        Self::ManualReview,
        Self::InformationRequested,
        Self::Approved,
        Self::Rejected,
        Self::Expired,
    ];

    /// Status of a newly opened case, before the customer submits it.
    pub const INITIAL: Self = Self::Draft;

    /// The exact value stored in `compliance.kyc_cases.status`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "DRAFT",
            Self::Submitted => "SUBMITTED",
            Self::PendingProvider => "PENDING_PROVIDER",
            Self::ManualReview => "MANUAL_REVIEW",
            Self::InformationRequested => "INFORMATION_REQUESTED",
            Self::Approved => "APPROVED",
            Self::Rejected => "REJECTED",
            Self::Expired => "EXPIRED",
        }
    }

    /// A terminal case accepts no further transitions (API §32.2).
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Approved | Self::Rejected | Self::Expired)
    }

    /// Whether a reviewer may decide the case.
    pub const fn is_reviewable(self) -> bool {
        matches!(self, Self::ManualReview)
    }

    /// Only `APPROVED` satisfies KYC approval (database design §10.1).
    ///
    /// This is the KYC component of eligibility and nothing more. Financial
    /// access also depends on account status, restrictions and limits, which
    /// the central eligibility check must evaluate separately (architecture
    /// §25.1). Never use this predicate alone as a financial gate.
    pub const fn satisfies_kyc_approval(self) -> bool {
        matches!(self, Self::Approved)
    }
}

/// A stored status value that is not a KYC case status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownKycCaseStatus(pub String);

impl FromStr for KycCaseStatus {
    type Err = UnknownKycCaseStatus;

    /// Parses an exact stored value. Matching is case-sensitive and does not
    /// trim, so drifted or foreign values (including account statuses such as
    /// `KYC_APPROVED`) fail closed.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|status| status.as_str() == value)
            .ok_or_else(|| UnknownKycCaseStatus(value.to_owned()))
    }
}

/// A requested change to a KYC case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KycCaseCommand {
    /// The owning customer submits a draft, or resubmits after a request for
    /// information.
    Submit,
    /// Internal orchestration hands the submission to a verification provider.
    DispatchToProvider,
    /// Internal orchestration records a provider result, whatever its outcome,
    /// and routes the case to manual review.
    RecordProviderResult,
    /// A reviewer approves the case.
    Approve,
    /// A reviewer rejects the case.
    Reject,
    /// A reviewer asks the customer for more information.
    RequestInformation,
}

impl KycCaseCommand {
    pub const ALL: [Self; 6] = [
        Self::Submit,
        Self::DispatchToProvider,
        Self::RecordProviderResult,
        Self::Approve,
        Self::Reject,
        Self::RequestInformation,
    ];
}

/// The party requesting a transition.
///
/// The domain checks only the actor kind and its relationship to the case
/// owner. Any code can build any variant, so a value of this type is not proof
/// of identity or permission. See the module's application-layer requirements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KycActor {
    Customer { customer_id: Uuid },
    Reviewer { reviewer_id: Uuid },
    System,
}

/// The owner and status of a case at the point of a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KycCaseState {
    pub owner_id: Uuid,
    pub status: KycCaseStatus,
}

impl TryFrom<&KycCase> for KycCaseState {
    type Error = UnknownKycCaseStatus;

    fn try_from(case: &KycCase) -> Result<Self, Self::Error> {
        Ok(Self {
            owner_id: case.user_id,
            status: case.status.parse()?,
        })
    }
}

/// A status change validated by [`KycCaseState::transition`].
///
/// The fields are private, so the only way to get a value is from
/// `transition`. Persist it with a conditional update on [`Self::from`] so that
/// a concurrent change cannot be overwritten.
///
/// ```
/// use deypay_domain::kyc::{KycActor, KycCaseCommand, KycCaseState, KycCaseStatus};
/// use uuid::Uuid;
///
/// let case = KycCaseState {
///     owner_id: Uuid::from_u128(1),
///     status: KycCaseStatus::ManualReview,
/// };
/// let reviewer = KycActor::Reviewer { reviewer_id: Uuid::from_u128(2) };
/// let transition = case
///     .transition(KycCaseStatus::ManualReview, KycCaseCommand::Approve, reviewer)
///     .unwrap();
/// assert_eq!(transition.from(), KycCaseStatus::ManualReview);
/// assert_eq!(transition.to(), KycCaseStatus::Approved);
/// assert_eq!(transition.command(), KycCaseCommand::Approve);
/// ```
///
/// Code outside this module cannot forge a transition:
///
/// ```compile_fail,E0451
/// use deypay_domain::kyc::{KycCaseCommand, KycCaseStatus, KycCaseTransition};
///
/// let forged = KycCaseTransition {
///     from: KycCaseStatus::PendingProvider,
///     to: KycCaseStatus::Approved,
///     command: KycCaseCommand::RecordProviderResult,
/// };
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KycCaseTransition {
    from: KycCaseStatus,
    to: KycCaseStatus,
    command: KycCaseCommand,
}

impl KycCaseTransition {
    /// The status the case must still have when the change is persisted.
    pub const fn from(self) -> KycCaseStatus {
        self.from
    }

    /// The status the case moves to.
    pub const fn to(self) -> KycCaseStatus {
        self.to
    }

    /// The command that produced this change.
    pub const fn command(self) -> KycCaseCommand {
        self.command
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KycTransitionError {
    /// The actor may not issue this command for this case.
    Unauthorized { command: KycCaseCommand },
    /// The caller acted on a status the case no longer has.
    StaleStatus {
        expected: KycCaseStatus,
        actual: KycCaseStatus,
    },
    /// The case is terminal.
    TerminalCase { status: KycCaseStatus },
    /// The command is not permitted from the current status.
    InvalidTransition {
        from: KycCaseStatus,
        command: KycCaseCommand,
    },
}

impl KycCaseState {
    /// Validates `command` against the current status.
    ///
    /// `expected` is the status the caller observed when it chose to act
    /// (database design §10.1, API §32.2). Authorization is checked first so
    /// that an unauthorized actor learns nothing about the case's status.
    pub fn transition(
        self,
        expected: KycCaseStatus,
        command: KycCaseCommand,
        actor: KycActor,
    ) -> Result<KycCaseTransition, KycTransitionError> {
        if !self.authorizes(command, actor) {
            return Err(KycTransitionError::Unauthorized { command });
        }
        if expected != self.status {
            return Err(KycTransitionError::StaleStatus {
                expected,
                actual: self.status,
            });
        }
        if self.status.is_terminal() {
            return Err(KycTransitionError::TerminalCase {
                status: self.status,
            });
        }
        let to =
            next_status(self.status, command).ok_or(KycTransitionError::InvalidTransition {
                from: self.status,
                command,
            })?;
        Ok(KycCaseTransition {
            from: self.status,
            to,
            command,
        })
    }

    fn authorizes(self, command: KycCaseCommand, actor: KycActor) -> bool {
        use KycCaseCommand::*;
        match (command, actor) {
            (Submit, KycActor::Customer { customer_id }) => customer_id == self.owner_id,
            (DispatchToProvider | RecordProviderResult, KycActor::System) => true,
            // Separation of duties (security §3.6): nobody decides their own case.
            (Approve | Reject | RequestInformation, KycActor::Reviewer { reviewer_id }) => {
                reviewer_id != self.owner_id
            }
            _ => false,
        }
    }
}

/// The transition table. Anything absent is rejected.
const fn next_status(from: KycCaseStatus, command: KycCaseCommand) -> Option<KycCaseStatus> {
    use KycCaseCommand::*;
    use KycCaseStatus::*;
    match (from, command) {
        (Draft | InformationRequested, Submit) => Some(Submitted),
        (Submitted, DispatchToProvider) => Some(PendingProvider),
        (PendingProvider, RecordProviderResult) => Some(ManualReview),
        (ManualReview, Approve) => Some(Approved),
        (ManualReview, Reject) => Some(Rejected),
        (ManualReview, RequestInformation) => Some(InformationRequested),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::KycCaseCommand::*;
    use super::KycCaseStatus::*;
    use super::*;
    use std::collections::{BTreeSet, HashSet};

    const OWNER: Uuid = Uuid::from_u128(0x1);
    const OTHER_CUSTOMER: Uuid = Uuid::from_u128(0x2);
    const REVIEWER: Uuid = Uuid::from_u128(0x3);

    const CUSTOMER: KycActor = KycActor::Customer { customer_id: OWNER };
    const FOREIGN_CUSTOMER: KycActor = KycActor::Customer {
        customer_id: OTHER_CUSTOMER,
    };
    const REVIEWER_ACTOR: KycActor = KycActor::Reviewer {
        reviewer_id: REVIEWER,
    };
    const SELF_REVIEWER: KycActor = KycActor::Reviewer { reviewer_id: OWNER };
    const SYSTEM: KycActor = KycActor::System;

    const ALL_ACTORS: [KycActor; 5] = [
        CUSTOMER,
        FOREIGN_CUSTOMER,
        REVIEWER_ACTOR,
        SELF_REVIEWER,
        SYSTEM,
    ];

    const PERMITTED: [(KycCaseStatus, KycCaseCommand, KycCaseStatus); 7] = [
        (Draft, Submit, Submitted),
        (InformationRequested, Submit, Submitted),
        (Submitted, DispatchToProvider, PendingProvider),
        (PendingProvider, RecordProviderResult, ManualReview),
        (ManualReview, Approve, Approved),
        (ManualReview, Reject, Rejected),
        (ManualReview, RequestInformation, InformationRequested),
    ];

    fn case(status: KycCaseStatus) -> KycCaseState {
        KycCaseState {
            owner_id: OWNER,
            status,
        }
    }

    fn apply(
        status: KycCaseStatus,
        command: KycCaseCommand,
        actor: KycActor,
    ) -> Result<KycCaseTransition, KycTransitionError> {
        case(status).transition(status, command, actor)
    }

    fn authorized_actor(command: KycCaseCommand) -> KycActor {
        match command {
            Submit => CUSTOMER,
            DispatchToProvider | RecordProviderResult => SYSTEM,
            Approve | Reject | RequestInformation => REVIEWER_ACTOR,
        }
    }

    /// Every status change any actor can achieve from `status`.
    fn successors(status: KycCaseStatus, actors: &[KycActor]) -> Vec<KycCaseTransition> {
        KycCaseCommand::ALL
            .into_iter()
            .flat_map(|command| actors.iter().map(move |actor| (command, *actor)))
            .filter_map(|(command, actor)| apply(status, command, actor).ok())
            .collect()
    }

    fn reachable_from(start: KycCaseStatus, actors: &[KycActor]) -> HashSet<KycCaseStatus> {
        let mut seen = HashSet::from([start]);
        let mut queue = vec![start];
        while let Some(status) = queue.pop() {
            for transition in successors(status, actors) {
                if seen.insert(transition.to) {
                    queue.push(transition.to);
                }
            }
        }
        seen
    }

    #[test]
    fn statuses_round_trip_through_their_stored_values() {
        for status in KycCaseStatus::ALL {
            assert_eq!(status.as_str().parse::<KycCaseStatus>(), Ok(status));
        }
        let distinct: HashSet<_> = KycCaseStatus::ALL.iter().map(|s| s.as_str()).collect();
        assert_eq!(distinct.len(), KycCaseStatus::ALL.len());
    }

    #[test]
    fn unknown_and_drifted_status_values_fail_closed() {
        for value in [
            "",
            "approved",
            "Approved",
            " APPROVED",
            "APPROVED ",
            "NOT_STARTED",
            "PENDING",
            "KYC_APPROVED",
            "KYC_PENDING",
            "CONTACT_VERIFIED",
        ] {
            assert_eq!(
                value.parse::<KycCaseStatus>(),
                Err(UnknownKycCaseStatus(value.to_owned())),
                "{value:?} must not parse as a KYC case status"
            );
        }
    }

    #[test]
    fn statuses_match_the_kyc_cases_check_constraint() {
        let migration = include_str!("../../../migrations/202611180003_kyc.sql");
        let table = migration
            .split_once("CREATE TABLE compliance.kyc_cases")
            .expect("kyc_cases table")
            .1;
        let allowed = table
            .split_once("status IN (")
            .and_then(|(_, rest)| rest.split_once(')'))
            .expect("status CHECK list")
            .0;
        let schema: BTreeSet<&str> = allowed
            .split(',')
            .map(|value| value.trim().trim_matches('\''))
            .collect();
        let domain: BTreeSet<&str> = KycCaseStatus::ALL.iter().map(|s| s.as_str()).collect();
        assert_eq!(domain, schema);
    }

    #[test]
    fn new_cases_start_as_unapproved_drafts() {
        assert_eq!(KycCaseStatus::INITIAL, Draft);
        assert!(!KycCaseStatus::INITIAL.satisfies_kyc_approval());
        assert!(!KycCaseStatus::INITIAL.is_terminal());
    }

    #[test]
    fn transition_table_is_exactly_the_documented_set() {
        for status in KycCaseStatus::ALL {
            for command in KycCaseCommand::ALL {
                let expected = PERMITTED
                    .iter()
                    .find(|(from, cmd, _)| *from == status && *cmd == command)
                    .map(|(_, _, to)| *to);
                let result = apply(status, command, authorized_actor(command));
                match expected {
                    Some(to) => assert_eq!(
                        result,
                        Ok(KycCaseTransition {
                            from: status,
                            to,
                            command
                        })
                    ),
                    None => assert!(result.is_err(), "{status:?} + {command:?} must be rejected"),
                }
            }
        }
    }

    #[test]
    fn invalid_transitions_from_open_cases_name_the_status_and_command() {
        for (status, command) in [
            (Draft, Approve),
            (Draft, DispatchToProvider),
            (Submitted, Submit),
            (Submitted, Approve),
            (PendingProvider, Approve),
            (PendingProvider, Reject),
            (PendingProvider, DispatchToProvider),
            (ManualReview, Submit),
            (ManualReview, RecordProviderResult),
            (InformationRequested, Approve),
            (InformationRequested, DispatchToProvider),
        ] {
            assert_eq!(
                apply(status, command, authorized_actor(command)),
                Err(KycTransitionError::InvalidTransition {
                    from: status,
                    command
                })
            );
        }
    }

    #[test]
    fn terminal_cases_reject_every_command() {
        let terminal: Vec<_> = KycCaseStatus::ALL
            .into_iter()
            .filter(|s| s.is_terminal())
            .collect();
        assert_eq!(terminal, [Approved, Rejected, Expired]);
        for status in terminal {
            for command in KycCaseCommand::ALL {
                assert_eq!(
                    apply(status, command, authorized_actor(command)),
                    Err(KycTransitionError::TerminalCase { status })
                );
            }
        }
    }

    #[test]
    fn approval_requires_a_non_owner_reviewer_deciding_a_manual_review_case() {
        let mut routes = Vec::new();
        for status in KycCaseStatus::ALL {
            for command in KycCaseCommand::ALL {
                for actor in ALL_ACTORS {
                    if let Ok(transition) = apply(status, command, actor)
                        && transition.to.satisfies_kyc_approval()
                    {
                        routes.push((status, command, actor));
                    }
                }
            }
        }
        assert_eq!(routes, [(ManualReview, Approve, REVIEWER_ACTOR)]);
    }

    #[test]
    fn customer_and_system_actions_alone_never_reach_a_kyc_decision() {
        // Contact verification is a customer action with no KYC command. Even
        // with every customer and system command, a case cannot be decided.
        let reachable = reachable_from(Draft, &[CUSTOMER, FOREIGN_CUSTOMER, SYSTEM]);
        assert_eq!(
            reachable,
            HashSet::from([Draft, Submitted, PendingProvider, ManualReview])
        );
        assert!(!reachable.iter().any(|s| s.satisfies_kyc_approval()));
    }

    #[test]
    fn provider_results_route_to_manual_review_and_never_decide() {
        let transition = apply(PendingProvider, RecordProviderResult, SYSTEM).unwrap();
        assert_eq!(transition.to, ManualReview);
        assert!(!transition.to.satisfies_kyc_approval());
        for status in KycCaseStatus::ALL {
            for transition in successors(status, &[SYSTEM]) {
                assert!(
                    !transition.to.is_terminal(),
                    "system reached {transition:?}"
                );
            }
        }
    }

    #[test]
    fn only_manual_review_is_reviewable() {
        let reviewable: Vec<_> = KycCaseStatus::ALL
            .into_iter()
            .filter(|s| s.is_reviewable())
            .collect();
        assert_eq!(reviewable, [ManualReview]);
        for status in KycCaseStatus::ALL
            .into_iter()
            .filter(|s| *s != ManualReview)
        {
            assert!(successors(status, &[REVIEWER_ACTOR]).is_empty());
        }
    }

    #[test]
    fn only_approved_satisfies_kyc_approval() {
        for status in KycCaseStatus::ALL {
            assert_eq!(status.satisfies_kyc_approval(), status == Approved);
        }
    }

    #[test]
    fn information_requested_permits_resubmission_and_stays_unapproved() {
        let requested = apply(ManualReview, RequestInformation, REVIEWER_ACTOR).unwrap();
        assert_eq!(requested.to, InformationRequested);
        assert!(!requested.to.satisfies_kyc_approval());
        assert!(!requested.to.is_terminal());
        let resubmitted = apply(InformationRequested, Submit, CUSTOMER).unwrap();
        assert_eq!(resubmitted.to, Submitted);
        // Test strategy §16.4: resubmission alone cannot enable access.
        assert!(!resubmitted.to.satisfies_kyc_approval());
        let reachable = reachable_from(InformationRequested, &[CUSTOMER, SYSTEM]);
        assert!(!reachable.iter().any(|s| s.satisfies_kyc_approval()));
    }

    #[test]
    fn late_or_duplicate_provider_results_cannot_change_a_case() {
        // Test strategy §16.3: a repeated or delayed callback must not move a
        // case that has already been routed or decided.
        for status in [ManualReview, InformationRequested, Submitted, Draft] {
            assert_eq!(
                apply(status, RecordProviderResult, SYSTEM),
                Err(KycTransitionError::InvalidTransition {
                    from: status,
                    command: RecordProviderResult
                })
            );
        }
        for status in [Approved, Rejected, Expired] {
            assert_eq!(
                apply(status, RecordProviderResult, SYSTEM),
                Err(KycTransitionError::TerminalCase { status })
            );
        }
    }

    #[test]
    fn rejection_is_terminal_for_the_case() {
        let rejected = apply(ManualReview, Reject, REVIEWER_ACTOR).unwrap();
        assert_eq!(rejected.to, Rejected);
        assert_eq!(
            apply(Rejected, Submit, CUSTOMER),
            Err(KycTransitionError::TerminalCase { status: Rejected })
        );
    }

    #[test]
    fn no_transition_enters_expired() {
        for status in KycCaseStatus::ALL {
            let entries = successors(status, &ALL_ACTORS);
            assert!(entries.iter().all(|t| t.to != Expired), "{entries:?}");
        }
    }

    #[test]
    fn each_command_requires_its_actor() {
        for status in KycCaseStatus::ALL {
            for command in KycCaseCommand::ALL {
                for actor in ALL_ACTORS {
                    if actor == authorized_actor(command) {
                        continue;
                    }
                    assert_eq!(
                        apply(status, command, actor),
                        Err(KycTransitionError::Unauthorized { command }),
                        "{actor:?} issued {command:?} from {status:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn customers_cannot_act_on_another_customers_case() {
        assert_eq!(
            apply(Draft, Submit, FOREIGN_CUSTOMER),
            Err(KycTransitionError::Unauthorized { command: Submit })
        );
    }

    #[test]
    fn reviewers_cannot_decide_their_own_case() {
        for command in [Approve, Reject, RequestInformation] {
            assert_eq!(
                apply(ManualReview, command, SELF_REVIEWER),
                Err(KycTransitionError::Unauthorized { command })
            );
        }
    }

    #[test]
    fn customers_cannot_decide_their_own_case() {
        for command in [Approve, Reject, RequestInformation] {
            assert_eq!(
                apply(ManualReview, command, CUSTOMER),
                Err(KycTransitionError::Unauthorized { command })
            );
        }
    }

    #[test]
    fn stale_expected_status_is_rejected() {
        let state = case(Approved);
        assert_eq!(
            state.transition(ManualReview, Approve, REVIEWER_ACTOR),
            Err(KycTransitionError::StaleStatus {
                expected: ManualReview,
                actual: Approved
            })
        );
        let state = case(Submitted);
        assert_eq!(
            state.transition(Draft, Submit, CUSTOMER),
            Err(KycTransitionError::StaleStatus {
                expected: Draft,
                actual: Submitted
            })
        );
    }

    #[test]
    fn unauthorized_actors_learn_nothing_about_case_status() {
        // A stale or terminal case reports the same error as a valid one.
        for (status, expected) in [(ManualReview, ManualReview), (Approved, ManualReview)] {
            assert_eq!(
                case(status).transition(expected, Approve, FOREIGN_CUSTOMER),
                Err(KycTransitionError::Unauthorized { command: Approve })
            );
        }
    }

    #[test]
    fn full_review_cycle_reaches_approval_only_through_a_reviewer() {
        let mut state = case(KycCaseStatus::INITIAL);
        for (command, actor) in [
            (Submit, CUSTOMER),
            (DispatchToProvider, SYSTEM),
            (RecordProviderResult, SYSTEM),
            (RequestInformation, REVIEWER_ACTOR),
            (Submit, CUSTOMER),
            (DispatchToProvider, SYSTEM),
            (RecordProviderResult, SYSTEM),
        ] {
            assert!(!state.status.satisfies_kyc_approval());
            state.status = state.transition(state.status, command, actor).unwrap().to;
        }
        assert_eq!(state.status, ManualReview);
        state.status = state
            .transition(ManualReview, Approve, REVIEWER_ACTOR)
            .unwrap()
            .to;
        assert!(state.status.satisfies_kyc_approval());
        assert!(state.status.is_terminal());
    }

    #[test]
    fn persisted_cases_convert_only_with_a_known_status() {
        let stored = KycCase {
            id: Uuid::from_u128(0x10),
            user_id: OWNER,
            status: "MANUAL_REVIEW".to_owned(),
        };
        assert_eq!(
            KycCaseState::try_from(&stored),
            Ok(KycCaseState {
                owner_id: OWNER,
                status: ManualReview
            })
        );
        let drifted = KycCase {
            status: "KYC_APPROVED".to_owned(),
            ..stored
        };
        assert_eq!(
            KycCaseState::try_from(&drifted),
            Err(UnknownKycCaseStatus("KYC_APPROVED".to_owned()))
        );
    }
}
