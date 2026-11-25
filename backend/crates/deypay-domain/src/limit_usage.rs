//! Limit-usage rules for windowed limits (IMP-P4-007, database design §10.6).
//!
//! These are the decisions a future acceptance transaction makes while it
//! holds the lock on each customer/window bucket. Nothing here persists or
//! locks anything, so no daily or monthly limit is enforced yet: enforcement
//! needs the acceptance transaction that Phase 5 and the payment phases build.
//!
//! The required transaction boundary, in one database transaction that also
//! accepts the operation:
//!
//! 1. Derive the buckets for every applicable windowed rule and lock them in
//!    [`lock_order`], the same order for withdrawals and bank payments.
//! 2. Read the bucket's recorded usage (`compliance.limit_usage`) and call
//!    [`admit`] for each bucket.
//! 3. Insert usage keyed by rule, scope, window and transaction; the existing
//!    primary key makes a retried acceptance idempotent.
//! 4. Commit together with the accepted transaction, or roll everything back.
//!
//! Window boundaries (calendar or rolling, time zone) are not approved, so the
//! caller supplies each bucket's window start.
use crate::units::MinorUnits;
use uuid::Uuid;

/// One customer/window bucket of a windowed rule. Ordered for locking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UsageBucket {
    pub rule_id: Uuid,
    pub scope_id: Uuid,
    /// Window start in microseconds since the Unix epoch.
    pub window_start_micros: i64,
}

/// Sorts and de-duplicates buckets so that every transaction locks shared
/// buckets in one order, which avoids deadlocks between concurrent operations.
pub fn lock_order(mut buckets: Vec<UsageBucket>) -> Vec<UsageBucket> {
    buckets.sort_unstable();
    buckets.dedup();
    buckets
}

/// Usage already recorded in a bucket for one transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordedUsage {
    pub transaction_id: Uuid,
    /// Minor units of the rule's asset.
    pub amount: MinorUnits,
    /// Released after definitive non-execution.
    pub released: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admission {
    /// Capacity is available: record the usage.
    Admit,
    /// This transaction already holds the same usage; a retry must not count
    /// it twice.
    AlreadyRecorded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageDenial {
    /// Recorded usage plus this amount would exceed the limit.
    Exceeded,
    /// The transaction already holds different or released usage in this
    /// bucket, so the request does not match the original acceptance.
    Conflict,
    /// The amount is zero.
    NotPositive,
}

/// Decides whether `amount` for `transaction_id` fits `limit`, given the
/// bucket's recorded usage. Pending, uncertain and completed usage all count;
/// only released usage does not. An amount equal to the remaining capacity
/// fits. Arithmetic is exact and checked.
pub fn admit(
    limit: MinorUnits,
    recorded: &[RecordedUsage],
    transaction_id: Uuid,
    amount: MinorUnits,
) -> Result<Admission, UsageDenial> {
    if amount.value() == 0 {
        return Err(UsageDenial::NotPositive);
    }
    if let Some(existing) = recorded.iter().find(|u| u.transaction_id == transaction_id) {
        return if !existing.released && existing.amount == amount {
            Ok(Admission::AlreadyRecorded)
        } else {
            Err(UsageDenial::Conflict)
        };
    }
    let mut used: u128 = 0;
    for usage in recorded.iter().filter(|u| !u.released) {
        used = used
            .checked_add(usage.amount.value())
            .ok_or(UsageDenial::Exceeded)?;
    }
    match used.checked_add(amount.value()) {
        Some(total) if total <= limit.value() => Ok(Admission::Admit),
        _ => Err(UsageDenial::Exceeded),
    }
}

/// What finally happened to an operation that consumed limit capacity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionOutcome {
    Pending,
    /// The provider outcome is unknown; the charge stays until reconciled.
    Uncertain,
    Completed,
    /// Definitively not executed: the only outcome that frees capacity.
    NotExecuted,
    /// Executed, then reversed or refunded: the original period keeps the
    /// usage.
    ReversedAfterExecution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Release {
    Released,
    Kept,
    AlreadyReleased,
}

/// Applies an outcome to recorded usage. Capacity is released at most once,
/// and only on definitive non-execution.
pub fn release(usage: &mut RecordedUsage, outcome: ExecutionOutcome) -> Release {
    if usage.released {
        return Release::AlreadyReleased;
    }
    if outcome == ExecutionOutcome::NotExecuted {
        usage.released = true;
        Release::Released
    } else {
        Release::Kept
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIMIT: u128 = 1_000;

    fn units(value: u128) -> MinorUnits {
        MinorUnits::new(value).unwrap()
    }

    fn usage(id: u128, amount: u128, released: bool) -> RecordedUsage {
        RecordedUsage {
            transaction_id: Uuid::from_u128(id),
            amount: units(amount),
            released,
        }
    }

    fn check(
        limit: u128,
        recorded: &[RecordedUsage],
        id: u128,
        amount: u128,
    ) -> Result<Admission, UsageDenial> {
        admit(units(limit), recorded, tx(id), units(amount))
    }

    fn tx(id: u128) -> Uuid {
        Uuid::from_u128(id)
    }

    #[test]
    fn capacity_boundaries_are_exact() {
        let recorded = [usage(1, 400, false), usage(2, 100, false)];
        assert_eq!(check(LIMIT, &recorded, 9, 500), Ok(Admission::Admit));
        assert_eq!(check(LIMIT, &recorded, 9, 501), Err(UsageDenial::Exceeded));
        assert_eq!(check(LIMIT, &[], 9, LIMIT), Ok(Admission::Admit));
        assert_eq!(check(0, &[], 9, 1), Err(UsageDenial::Exceeded));
        assert_eq!(check(LIMIT, &[], 9, 0), Err(UsageDenial::NotPositive));
    }

    #[test]
    fn pending_and_uncertain_usage_counts_until_released() {
        // Released usage frees capacity; everything else still counts.
        let recorded = [usage(1, 600, false), usage(2, 400, true)];
        assert_eq!(check(LIMIT, &recorded, 9, 400), Ok(Admission::Admit));
        assert_eq!(check(LIMIT, &recorded, 9, 401), Err(UsageDenial::Exceeded));
    }

    #[test]
    fn retries_are_idempotent_and_mismatches_conflict() {
        let recorded = [usage(1, 600, false), usage(2, 300, true)];
        assert_eq!(
            check(LIMIT, &recorded, 1, 600),
            Ok(Admission::AlreadyRecorded)
        );
        assert_eq!(check(LIMIT, &recorded, 1, 601), Err(UsageDenial::Conflict));
        // A transaction whose usage was released cannot consume again.
        assert_eq!(check(LIMIT, &recorded, 2, 300), Err(UsageDenial::Conflict));
    }

    #[test]
    fn overflowing_usage_is_denied_not_wrapped() {
        // Four maximal usages exceed u128 itself; the sum must not wrap.
        let max = 10_u128.pow(38) - 1;
        let recorded = [
            usage(1, max, false),
            usage(2, max, false),
            usage(3, max, false),
            usage(4, max, false),
        ];
        assert_eq!(check(max, &recorded, 9, 1), Err(UsageDenial::Exceeded));
        assert_eq!(
            check(max, &[usage(1, max, false)], 9, 1),
            Err(UsageDenial::Exceeded)
        );
        assert_eq!(
            check(max, &[usage(1, max - 1, false)], 9, 1),
            Ok(Admission::Admit)
        );
    }

    #[test]
    fn only_definitive_non_execution_releases_and_only_once() {
        for outcome in [
            ExecutionOutcome::Pending,
            ExecutionOutcome::Uncertain,
            ExecutionOutcome::Completed,
            ExecutionOutcome::ReversedAfterExecution,
        ] {
            let mut recorded = usage(1, 10, false);
            assert_eq!(
                release(&mut recorded, outcome),
                Release::Kept,
                "{outcome:?}"
            );
            assert!(!recorded.released);
        }
        let mut recorded = usage(1, 10, false);
        assert_eq!(
            release(&mut recorded, ExecutionOutcome::NotExecuted),
            Release::Released
        );
        assert_eq!(
            release(&mut recorded, ExecutionOutcome::NotExecuted),
            Release::AlreadyReleased
        );
        assert_eq!(
            release(&mut recorded, ExecutionOutcome::Completed),
            Release::AlreadyReleased
        );
    }

    #[test]
    fn buckets_lock_in_one_order_whatever_the_request() {
        let bucket = |rule: u128, scope: u128, start: i64| UsageBucket {
            rule_id: Uuid::from_u128(rule),
            scope_id: Uuid::from_u128(scope),
            window_start_micros: start,
        };
        let withdrawal = vec![
            bucket(2, 1, 0),
            bucket(1, 1, 86_400),
            bucket(1, 1, 0),
            bucket(2, 1, 0),
        ];
        let bank_payment = vec![bucket(1, 1, 0), bucket(2, 1, 0), bucket(1, 1, 86_400)];
        let ordered = lock_order(withdrawal);
        assert_eq!(
            ordered,
            [bucket(1, 1, 0), bucket(1, 1, 86_400), bucket(2, 1, 0)]
        );
        assert_eq!(lock_order(bank_payment), ordered);
    }
}
