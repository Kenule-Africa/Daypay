//! Exact, nonnegative integer accounting units (IMP-P5-001).
//!
//! Every amount is a whole number of an asset's smallest accounting unit, in
//! the 38-digit range that PostgreSQL stores as `numeric`. Arithmetic is
//! checked: an operation that would leave the range or go below zero returns
//! `None` instead of wrapping, saturating or rounding.
use std::fmt;
use std::str::FromStr;

/// The exclusive upper bound, 10^38.
const LIMIT: u128 = 100_000_000_000_000_000_000_000_000_000_000_000_000;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MinorUnits(u128);

impl MinorUnits {
    pub const ZERO: Self = Self(0);
    pub const MAX: Self = Self(LIMIT - 1);
    /// Maximum number of decimal digits in a stored amount.
    pub const MAX_DIGITS: usize = 38;

    pub const fn new(value: u128) -> Option<Self> {
        if value < LIMIT {
            Some(Self(value))
        } else {
            None
        }
    }

    pub const fn value(self) -> u128 {
        self.0
    }

    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    pub fn checked_add(self, other: Self) -> Option<Self> {
        self.0.checked_add(other.0).and_then(Self::new)
    }

    pub fn checked_sub(self, other: Self) -> Option<Self> {
        self.0.checked_sub(other.0).map(Self)
    }

    /// Multiplies by 10^exponent, failing outside the accounting range.
    pub fn checked_scale_up(self, exponent: u32) -> Option<Self> {
        10_u128
            .checked_pow(exponent)
            .and_then(|factor| self.0.checked_mul(factor))
            .and_then(Self::new)
    }
}

impl fmt::Display for MinorUnits {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitsError {
    /// Empty text, signs, spaces, separators, exponents or other non-digits.
    Malformed,
    /// A multi-digit value starting with zero.
    LeadingZero,
    /// 10^38 or more.
    OutOfRange,
}

impl FromStr for MinorUnits {
    type Err = UnitsError;

    /// Accepts only the canonical form PostgreSQL returns for `numeric::text`
    /// of an integer: `0` or digits without a leading zero.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
            return Err(UnitsError::Malformed);
        }
        if text.len() > 1 && text.starts_with('0') {
            return Err(UnitsError::LeadingZero);
        }
        if text.len() > Self::MAX_DIGITS {
            return Err(UnitsError::OutOfRange);
        }
        text.parse::<u128>()
            .ok()
            .and_then(Self::new)
            .ok_or(UnitsError::OutOfRange)
    }
}

/// A strictly positive amount. Every posting line and reservation uses one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PostingAmount(MinorUnits);

impl PostingAmount {
    pub const fn new(units: MinorUnits) -> Option<Self> {
        if units.is_zero() {
            None
        } else {
            Some(Self(units))
        }
    }

    pub fn from_value(value: u128) -> Option<Self> {
        MinorUnits::new(value).and_then(Self::new)
    }

    pub const fn units(self) -> MinorUnits {
        self.0
    }
}

impl fmt::Display for PostingAmount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thirty_eight_digits_round_trip_without_floating_point() {
        let max = 10_u128.pow(38) - 1;
        assert_eq!(MinorUnits::new(max).unwrap().value(), max);
        assert!(MinorUnits::new(max + 1).is_none());
        assert_eq!(MinorUnits::new(0).unwrap().value(), 0);
        assert_eq!(MinorUnits::MAX.value(), max);
    }

    #[test]
    fn arithmetic_is_checked_at_both_ends() {
        let one = MinorUnits::new(1).unwrap();
        assert_eq!(MinorUnits::MAX.checked_add(one), None);
        assert_eq!(MinorUnits::ZERO.checked_sub(one), None);
        assert_eq!(one.checked_add(one).unwrap().value(), 2);
        assert_eq!(one.checked_sub(one), Some(MinorUnits::ZERO));
        assert_eq!(one.checked_scale_up(37).unwrap().value(), 10_u128.pow(37));
        assert_eq!(one.checked_scale_up(38), None);
        assert_eq!(MinorUnits::MAX.checked_scale_up(1), None);
        assert_eq!(one.checked_scale_up(200), None);
    }

    #[test]
    fn parsing_accepts_only_canonical_integers() {
        assert_eq!("0".parse::<MinorUnits>(), Ok(MinorUnits::ZERO));
        assert_eq!("1500000".parse::<MinorUnits>().unwrap().value(), 1_500_000);
        let max = "9".repeat(38);
        assert_eq!(max.parse::<MinorUnits>().unwrap(), MinorUnits::MAX);
        for (text, error) in [
            ("", UnitsError::Malformed),
            ("-1", UnitsError::Malformed),
            ("+1", UnitsError::Malformed),
            ("1.0", UnitsError::Malformed),
            ("1e6", UnitsError::Malformed),
            (" 1", UnitsError::Malformed),
            ("1_000", UnitsError::Malformed),
            ("00", UnitsError::LeadingZero),
            ("01", UnitsError::LeadingZero),
        ] {
            assert_eq!(text.parse::<MinorUnits>(), Err(error), "{text:?}");
        }
        let too_big = format!("1{}", "0".repeat(38));
        assert_eq!(too_big.parse::<MinorUnits>(), Err(UnitsError::OutOfRange));
        assert_eq!(MinorUnits::MAX.to_string(), max);
    }

    #[test]
    fn posting_amounts_are_strictly_positive() {
        assert!(PostingAmount::new(MinorUnits::ZERO).is_none());
        assert!(PostingAmount::from_value(10_u128.pow(38)).is_none());
        assert_eq!(PostingAmount::from_value(7).unwrap().units().value(), 7);
    }
}
