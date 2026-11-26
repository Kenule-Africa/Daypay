//! Double-entry journal model (IMP-P5-004–006).
//!
//! A [`JournalDraft`] can only be constructed when it has 2–64 lines, each
//! account appears once, and debits equal credits separately for every asset.
//! The database enforces the same invariants again at commit.
use crate::units::{MinorUnits, PostingAmount};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

pub const MIN_JOURNAL_LINES: usize = 2;
pub const MAX_JOURNAL_LINES: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Direction {
    Debit,
    Credit,
}

impl Direction {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Debit => "DEBIT",
            Self::Credit => "CREDIT",
        }
    }

    pub const fn opposite(self) -> Self {
        match self {
            Self::Debit => Self::Credit,
            Self::Credit => Self::Debit,
        }
    }
}

/// Formal account categories. Clearing and suspense are operational
/// classifications that still declare an explicit normal balance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountType {
    Asset,
    Liability,
    Revenue,
    Expense,
    Equity,
    Clearing,
    Suspense,
}

impl AccountType {
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "ASSET" => Self::Asset,
            "LIABILITY" => Self::Liability,
            "REVENUE" => Self::Revenue,
            "EXPENSE" => Self::Expense,
            "EQUITY" => Self::Equity,
            "CLEARING" => Self::Clearing,
            "SUSPENSE" => Self::Suspense,
            _ => return None,
        })
    }

    /// The normal balance fixed by double-entry rules (ledger spec §9), or
    /// `None` when the chart of accounts must state it.
    pub const fn required_normal_balance(self) -> Option<Direction> {
        match self {
            Self::Asset | Self::Expense => Some(Direction::Debit),
            Self::Liability | Self::Revenue | Self::Equity => Some(Direction::Credit),
            Self::Clearing | Self::Suspense => None,
        }
    }
}

/// Whether a line raises or lowers an account's balance on its normal side.
/// A debit to a customer liability lowers what DEYPAY owes the customer.
pub const fn increases(normal_balance: Direction, line: Direction) -> bool {
    matches!(
        (normal_balance, line),
        (Direction::Debit, Direction::Debit) | (Direction::Credit, Direction::Credit)
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PostingLine {
    pub account_id: Uuid,
    pub asset_id: Uuid,
    pub direction: Direction,
    pub amount: PostingAmount,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalError {
    TooFewLines,
    TooManyLines,
    DuplicateAccount(Uuid),
    Unbalanced {
        asset_id: Uuid,
    },
    /// Per-asset totals exceed the checked arithmetic range.
    Overflow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalDraft {
    lines: Vec<PostingLine>,
}

impl JournalDraft {
    pub fn new(lines: Vec<PostingLine>) -> Result<Self, JournalError> {
        if lines.len() < MIN_JOURNAL_LINES {
            return Err(JournalError::TooFewLines);
        }
        if lines.len() > MAX_JOURNAL_LINES {
            return Err(JournalError::TooManyLines);
        }
        let mut seen = BTreeSet::new();
        let mut totals: BTreeMap<Uuid, (u128, u128)> = BTreeMap::new();
        for line in &lines {
            if !seen.insert(line.account_id) {
                return Err(JournalError::DuplicateAccount(line.account_id));
            }
            let (debits, credits) = totals.entry(line.asset_id).or_default();
            let side = match line.direction {
                Direction::Debit => debits,
                Direction::Credit => credits,
            };
            *side = side
                .checked_add(line.amount.units().value())
                .ok_or(JournalError::Overflow)?;
        }
        if let Some((asset_id, _)) = totals
            .iter()
            .find(|(_, (debits, credits))| debits != credits)
        {
            return Err(JournalError::Unbalanced {
                asset_id: *asset_id,
            });
        }
        Ok(Self { lines })
    }

    pub fn lines(&self) -> &[PostingLine] {
        &self.lines
    }

    /// The compensating journal: identical lines with opposite directions.
    pub fn reversed(&self) -> Self {
        Self {
            lines: self
                .lines
                .iter()
                .map(|line| PostingLine {
                    direction: line.direction.opposite(),
                    ..*line
                })
                .collect(),
        }
    }

    /// Debit total for one asset (equal to its credit total by construction).
    pub fn total(&self, asset_id: Uuid) -> MinorUnits {
        let sum = self
            .lines
            .iter()
            .filter(|line| line.asset_id == asset_id && line.direction == Direction::Debit)
            .map(|line| line.amount.units().value())
            .sum::<u128>();
        MinorUnits::new(sum).unwrap_or(MinorUnits::MAX)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityError {
    EventType,
    EconomicKey,
    Purpose,
}

/// The idempotency identity of an accounting event: the verified business fact
/// (`economic_key`) and the posting leg (`purpose`). The same identity always
/// yields the same journal (ledger spec §26.3, database design §13.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventIdentity {
    event_type: String,
    economic_key: String,
    purpose: String,
}

fn upper_token(value: &str, extra: &[u8], max: usize, min: usize) -> bool {
    let bytes = value.as_bytes();
    (min..=max).contains(&bytes.len())
        && bytes[0].is_ascii_uppercase()
        && bytes[1..].iter().all(|b| {
            b.is_ascii_uppercase() || b.is_ascii_digit() || *b == b'_' || extra.contains(b)
        })
}

/// 1–256 characters without control characters, as stored for keys and references.
pub fn valid_key(value: &str) -> bool {
    (1..=256).contains(&value.chars().count()) && !value.chars().any(char::is_control)
}

impl EventIdentity {
    pub fn new(event_type: &str, economic_key: &str, purpose: &str) -> Result<Self, IdentityError> {
        if !upper_token(event_type, &[], 64, 3) {
            return Err(IdentityError::EventType);
        }
        if !valid_key(economic_key) {
            return Err(IdentityError::EconomicKey);
        }
        if !upper_token(purpose, b":.-", 64, 1) {
            return Err(IdentityError::Purpose);
        }
        Ok(Self {
            event_type: event_type.to_owned(),
            economic_key: economic_key.to_owned(),
            purpose: purpose.to_owned(),
        })
    }

    pub fn event_type(&self) -> &str {
        &self.event_type
    }

    pub fn economic_key(&self) -> &str {
        &self.economic_key
    }

    pub fn purpose(&self) -> &str {
        &self.purpose
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(account: u128, asset: u128, direction: Direction, amount: u128) -> PostingLine {
        PostingLine {
            account_id: Uuid::from_u128(account),
            asset_id: Uuid::from_u128(asset),
            direction,
            amount: PostingAmount::from_value(amount).unwrap(),
        }
    }

    #[test]
    fn journals_balance_per_asset() {
        use Direction::*;
        assert!(JournalDraft::new(vec![line(1, 9, Debit, 50), line(2, 9, Credit, 50)]).is_ok());
        assert_eq!(
            JournalDraft::new(vec![line(1, 9, Debit, 50), line(2, 9, Credit, 49)]),
            Err(JournalError::Unbalanced {
                asset_id: Uuid::from_u128(9)
            })
        );
        // A USDC debit cannot be balanced by a USDT credit.
        assert!(matches!(
            JournalDraft::new(vec![line(1, 8, Debit, 50), line(2, 9, Credit, 50)]),
            Err(JournalError::Unbalanced { .. })
        ));
        // Two assets in one journal each balance on their own.
        let draft = JournalDraft::new(vec![
            line(1, 8, Debit, 50),
            line(2, 8, Credit, 50),
            line(3, 9, Debit, 4980),
            line(4, 9, Credit, 4900),
            line(5, 9, Credit, 80),
        ])
        .unwrap();
        assert_eq!(draft.total(Uuid::from_u128(9)).value(), 4980);
        assert_eq!(
            JournalDraft::new(vec![line(1, 9, Debit, 5)]),
            Err(JournalError::TooFewLines)
        );
        assert_eq!(
            JournalDraft::new(vec![line(1, 9, Debit, 5), line(1, 9, Credit, 5)]),
            Err(JournalError::DuplicateAccount(Uuid::from_u128(1)))
        );
        let many = (0..65).map(|i| line(i, 9, Debit, 1)).collect();
        assert_eq!(JournalDraft::new(many), Err(JournalError::TooManyLines));
        let max = 10_u128.pow(38) - 1;
        let overflow = (0..5)
            .map(|i| line(i, 9, Debit, max))
            .chain([line(9, 9, Credit, 1)])
            .collect();
        assert_eq!(JournalDraft::new(overflow), Err(JournalError::Overflow));
    }

    #[test]
    fn reversal_mirrors_every_line() {
        use Direction::*;
        let draft = JournalDraft::new(vec![line(1, 9, Debit, 10), line(2, 9, Credit, 10)]).unwrap();
        let reversed = draft.reversed();
        assert_eq!(reversed.lines()[0].direction, Credit);
        assert_eq!(reversed.lines()[1].direction, Debit);
        assert_eq!(
            JournalDraft::new(reversed.lines().to_vec()),
            Ok(reversed.clone())
        );
        assert_eq!(reversed.reversed(), draft);
    }

    #[test]
    fn normal_balance_decides_the_effect_of_a_line() {
        assert!(increases(Direction::Debit, Direction::Debit));
        assert!(!increases(Direction::Credit, Direction::Debit));
        assert!(increases(Direction::Credit, Direction::Credit));
        assert_eq!(
            AccountType::Liability.required_normal_balance(),
            Some(Direction::Credit)
        );
        assert_eq!(
            AccountType::Asset.required_normal_balance(),
            Some(Direction::Debit)
        );
        assert_eq!(AccountType::Clearing.required_normal_balance(), None);
        assert_eq!(AccountType::parse("SUSPENSE"), Some(AccountType::Suspense));
        assert_eq!(AccountType::parse("asset"), None);
    }

    #[test]
    fn event_identities_match_the_database_rules() {
        assert!(EventIdentity::new("DEPOSIT_CONFIRMED", "solana:tx:7", "RECOGNITION").is_ok());
        assert!(EventIdentity::new("DEPOSIT_CONFIRMED", "k", "RELEASE:1").is_ok());
        assert_eq!(
            EventIdentity::new("deposit", "k", "P"),
            Err(IdentityError::EventType)
        );
        assert_eq!(
            EventIdentity::new("AB", "k", "P"),
            Err(IdentityError::EventType)
        );
        assert_eq!(
            EventIdentity::new("DEPOSIT", "", "P"),
            Err(IdentityError::EconomicKey)
        );
        assert_eq!(
            EventIdentity::new("DEPOSIT", "a\nb", "P"),
            Err(IdentityError::EconomicKey)
        );
        assert_eq!(
            EventIdentity::new("DEPOSIT", &"k".repeat(257), "P"),
            Err(IdentityError::EconomicKey)
        );
        assert_eq!(
            EventIdentity::new("DEPOSIT", "k", "p"),
            Err(IdentityError::Purpose)
        );
        assert_eq!(
            EventIdentity::new("DEPOSIT", "k", ""),
            Err(IdentityError::Purpose)
        );
    }

    /// Randomized invariants (ledger spec §39.11): generated balanced journals
    /// always validate, any single-unit perturbation is rejected, and a
    /// reversal nets every account and asset to zero.
    #[test]
    fn randomized_journals_preserve_double_entry() {
        let mut seed = 0x9e37_79b9_7f4a_7c15_u64;
        let mut next = move |bound: u64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed % bound
        };
        for round in 0..2_000_u128 {
            let assets = 1 + next(3) as u128;
            let mut lines = Vec::new();
            let mut account = round * 1_000;
            for asset in 0..assets {
                let credits = 1 + next(4);
                let mut total = 0_u128;
                for _ in 0..credits {
                    let amount = 1 + u128::from(next(1_000_000_000));
                    total += amount;
                    account += 1;
                    lines.push(line(account, asset, Direction::Credit, amount));
                }
                account += 1;
                lines.push(line(account, asset, Direction::Debit, total));
            }
            let draft = JournalDraft::new(lines.clone()).expect("balanced journal");
            let mut net: BTreeMap<Uuid, i128> = BTreeMap::new();
            for entry in draft.lines().iter().chain(draft.reversed().lines()) {
                let value = entry.amount.units().value() as i128;
                *net.entry(entry.account_id).or_default() += if entry.direction == Direction::Debit
                {
                    value
                } else {
                    -value
                };
            }
            assert!(net.values().all(|value| *value == 0));
            let victim = next(lines.len() as u64) as usize;
            let bumped = lines[victim].amount.units().value() + 1;
            lines[victim].amount = PostingAmount::from_value(bumped).unwrap();
            assert!(matches!(
                JournalDraft::new(lines),
                Err(JournalError::Unbalanced { .. })
            ));
        }
    }
}
