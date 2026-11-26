//! Asset-tagged money, accounting precision and token-unit conversion
//! (IMP-P5-001/002). Nothing here uses floating point, and nothing rounds:
//! a value that cannot be represented exactly is rejected.
use crate::units::MinorUnits;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Money {
    pub asset_id: Uuid,
    pub units: MinorUnits,
}

impl Money {
    /// Adds two amounts of the same asset. Different assets never combine
    /// without an explicit conversion.
    pub fn checked_add(self, other: Self) -> Result<Self, MoneyError> {
        if self.asset_id != other.asset_id {
            return Err(MoneyError::AssetMismatch);
        }
        let units = self
            .units
            .checked_add(other.units)
            .ok_or(MoneyError::OutOfRange)?;
        Ok(Self {
            asset_id: self.asset_id,
            units,
        })
    }

    pub fn checked_sub(self, other: Self) -> Result<Self, MoneyError> {
        if self.asset_id != other.asset_id {
            return Err(MoneyError::AssetMismatch);
        }
        let units = self
            .units
            .checked_sub(other.units)
            .ok_or(MoneyError::Negative)?;
        Ok(Self {
            asset_id: self.asset_id,
            units,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoneyError {
    /// Not a plain decimal: signs, exponents, spaces, separators or empty parts.
    Malformed,
    /// More fractional digits than the asset's accounting precision.
    TooManyDecimals { allowed: u8 },
    /// Outside the 38-digit accounting range.
    OutOfRange,
    /// The result would be below zero.
    Negative,
    /// The two amounts are different assets.
    AssetMismatch,
    /// A token amount with a fraction smaller than one accounting unit, or an
    /// accounting amount smaller than one token base unit.
    UnsupportedFraction,
}

/// Number of decimal places in one whole unit of an asset's accounting amount.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Precision(u8);

impl Precision {
    /// Matches the `approved_accounting_precision` database constraint.
    pub const MAX: u8 = 18;

    pub const fn new(decimals: u8) -> Option<Self> {
        if decimals <= Self::MAX {
            Some(Self(decimals))
        } else {
            None
        }
    }

    pub const fn decimals(self) -> u8 {
        self.0
    }
}

/// Owner-approved accounting precision (2026-11-26): six decimal places for
/// USDC and USDT, two for NGN. Other assets need an explicit decision.
pub fn approved_precision(symbol: &str) -> Option<Precision> {
    match symbol {
        "USDC" | "USDT" => Precision::new(6),
        "NGN" => Precision::new(2),
        _ => None,
    }
}

/// Parses a plain decimal such as `25`, `25.5` or `0.000001` into accounting
/// units. Rejects signs, exponents, separators, whitespace, leading zeros in the
/// integer part, an empty integer or fraction, and any fraction longer than the
/// precision (even trailing zeros), so no input is ever rounded.
pub fn parse_decimal(text: &str, precision: Precision) -> Result<MinorUnits, MoneyError> {
    let (whole, fraction) = match text.split_once('.') {
        Some((whole, fraction)) => (whole, Some(fraction)),
        None => (text, None),
    };
    let digits = |part: &str| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit());
    if !digits(whole) || (whole.len() > 1 && whole.starts_with('0')) {
        return Err(MoneyError::Malformed);
    }
    let fraction = fraction.unwrap_or("");
    if text.contains('.') && !digits(fraction) {
        return Err(MoneyError::Malformed);
    }
    let allowed = precision.decimals();
    if fraction.len() > usize::from(allowed) {
        return Err(MoneyError::TooManyDecimals { allowed });
    }
    if whole.len() > MinorUnits::MAX_DIGITS {
        return Err(MoneyError::OutOfRange);
    }
    let whole_units = whole
        .parse::<u128>()
        .ok()
        .and_then(MinorUnits::new)
        .and_then(|units| units.checked_scale_up(u32::from(allowed)))
        .ok_or(MoneyError::OutOfRange)?;
    let padded = format!("{fraction:0<width$}", width = usize::from(allowed));
    let fraction_units = if padded.is_empty() {
        MinorUnits::ZERO
    } else {
        padded
            .parse::<u128>()
            .ok()
            .and_then(MinorUnits::new)
            .ok_or(MoneyError::Malformed)?
    };
    whole_units
        .checked_add(fraction_units)
        .ok_or(MoneyError::OutOfRange)
}

/// Formats accounting units with exactly `precision` fractional digits.
pub fn format_decimal(units: MinorUnits, precision: Precision) -> String {
    let digits = units.value().to_string();
    let places = usize::from(precision.decimals());
    if places == 0 {
        return digits;
    }
    let padded = format!("{digits:0>width$}", width = places + 1);
    let (whole, fraction) = padded.split_at(padded.len() - places);
    format!("{whole}.{fraction}")
}

/// Converts between a token's on-chain base units and DEYPAY accounting units
/// using verified contract decimals. Blockchain precision and accounting
/// precision are independent (ledger spec §6.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenScale {
    token_decimals: u8,
    accounting: Precision,
}

impl TokenScale {
    /// Contract decimals above 36 are rejected as unverifiable for this range.
    pub const MAX_TOKEN_DECIMALS: u8 = 36;

    pub const fn new(token_decimals: u8, accounting: Precision) -> Option<Self> {
        if token_decimals <= Self::MAX_TOKEN_DECIMALS {
            Some(Self {
                token_decimals,
                accounting,
            })
        } else {
            None
        }
    }

    fn difference(self) -> (bool, u32) {
        let token = u32::from(self.token_decimals);
        let accounting = u32::from(self.accounting.decimals());
        (token >= accounting, token.abs_diff(accounting))
    }

    /// Token base units to accounting units. A remainder finer than one
    /// accounting unit is rejected for explicit handling, never dropped.
    pub fn to_accounting(self, base_units: u128) -> Result<MinorUnits, MoneyError> {
        let (token_finer, exponent) = self.difference();
        let factor = 10_u128
            .checked_pow(exponent)
            .ok_or(MoneyError::OutOfRange)?;
        if token_finer {
            if !base_units.is_multiple_of(factor) {
                return Err(MoneyError::UnsupportedFraction);
            }
            MinorUnits::new(base_units / factor).ok_or(MoneyError::OutOfRange)
        } else {
            base_units
                .checked_mul(factor)
                .and_then(MinorUnits::new)
                .ok_or(MoneyError::OutOfRange)
        }
    }

    /// Accounting units to token base units, for submitting a withdrawal.
    pub fn to_token(self, units: MinorUnits) -> Result<u128, MoneyError> {
        let (token_finer, exponent) = self.difference();
        let factor = 10_u128
            .checked_pow(exponent)
            .ok_or(MoneyError::OutOfRange)?;
        if token_finer {
            units
                .value()
                .checked_mul(factor)
                .ok_or(MoneyError::OutOfRange)
        } else if !units.value().is_multiple_of(factor) {
            Err(MoneyError::UnsupportedFraction)
        } else {
            Ok(units.value() / factor)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(decimals: u8) -> Precision {
        Precision::new(decimals).unwrap()
    }

    #[test]
    fn decimals_parse_exactly_or_not_at_all() {
        let six = p(6);
        assert_eq!(parse_decimal("25", six).unwrap().value(), 25_000_000);
        assert_eq!(parse_decimal("25.5", six).unwrap().value(), 25_500_000);
        assert_eq!(parse_decimal("0.000001", six).unwrap().value(), 1);
        assert_eq!(parse_decimal("0", six).unwrap(), MinorUnits::ZERO);
        assert_eq!(parse_decimal("15000.75", p(2)).unwrap().value(), 1_500_075);
        assert_eq!(
            parse_decimal("0.0000001", six),
            Err(MoneyError::TooManyDecimals { allowed: 6 })
        );
        assert_eq!(
            parse_decimal("1.0000000", six),
            Err(MoneyError::TooManyDecimals { allowed: 6 })
        );
        for bad in [
            "", ".5", "5.", "-1", "+1", "1e3", "1,000", " 1", "01", "1.2.3", "NaN", "Infinity",
        ] {
            assert_eq!(
                parse_decimal(bad, six),
                Err(MoneyError::Malformed),
                "{bad:?}"
            );
        }
        assert_eq!(
            parse_decimal(&"9".repeat(32), six).unwrap().value(),
            10_u128.pow(38) - 1_000_000
        );
        assert_eq!(
            parse_decimal(&"9".repeat(33), six),
            Err(MoneyError::OutOfRange)
        );
        assert_eq!(
            parse_decimal(&"9".repeat(40), p(0)),
            Err(MoneyError::OutOfRange)
        );
    }

    #[test]
    fn formatting_round_trips() {
        let six = p(6);
        for (units, text) in [(0, "0.000000"), (1, "0.000001"), (25_500_000, "25.500000")] {
            let value = MinorUnits::new(units).unwrap();
            assert_eq!(format_decimal(value, six), text);
            assert_eq!(parse_decimal(text, six).unwrap(), value);
        }
        assert_eq!(format_decimal(MinorUnits::new(42).unwrap(), p(0)), "42");
        assert_eq!(format_decimal(MinorUnits::new(5).unwrap(), p(2)), "0.05");
    }

    #[test]
    fn approved_precision_matches_owner_decision() {
        assert_eq!(approved_precision("USDC"), Some(p(6)));
        assert_eq!(approved_precision("USDT"), Some(p(6)));
        assert_eq!(approved_precision("NGN"), Some(p(2)));
        assert_eq!(approved_precision("ETH"), None);
        assert!(Precision::new(19).is_none());
    }

    #[test]
    fn token_units_convert_without_silent_rounding() {
        let six = p(6);
        let same = TokenScale::new(6, six).unwrap();
        assert_eq!(same.to_accounting(1_234_567).unwrap().value(), 1_234_567);
        assert_eq!(same.to_token(MinorUnits::new(9).unwrap()), Ok(9));

        // An 18-decimal token: whole accounting units only.
        let finer = TokenScale::new(18, six).unwrap();
        assert_eq!(finer.to_accounting(1_000_000_000_000).unwrap().value(), 1);
        assert_eq!(
            finer.to_accounting(1_000_000_000_001),
            Err(MoneyError::UnsupportedFraction)
        );
        assert_eq!(
            finer.to_token(MinorUnits::new(2).unwrap()),
            Ok(2_000_000_000_000)
        );

        // A coarser token scales up exactly and refuses sub-token amounts.
        let coarser = TokenScale::new(2, six).unwrap();
        assert_eq!(coarser.to_accounting(150).unwrap().value(), 1_500_000);
        assert_eq!(
            coarser.to_token(MinorUnits::new(1_500_000).unwrap()),
            Ok(150)
        );
        assert_eq!(
            coarser.to_token(MinorUnits::new(1).unwrap()),
            Err(MoneyError::UnsupportedFraction)
        );
        assert_eq!(
            coarser.to_accounting(u128::MAX),
            Err(MoneyError::OutOfRange)
        );
        assert!(TokenScale::new(37, six).is_none());
    }

    #[test]
    fn money_never_combines_different_assets() {
        let usdc = Uuid::from_u128(1);
        let usdt = Uuid::from_u128(2);
        let one = MinorUnits::new(1).unwrap();
        let a = Money {
            asset_id: usdc,
            units: one,
        };
        let b = Money {
            asset_id: usdt,
            units: one,
        };
        assert_eq!(a.checked_add(b), Err(MoneyError::AssetMismatch));
        assert_eq!(a.checked_add(a).unwrap().units.value(), 2);
        assert_eq!(
            a.checked_sub(a.checked_add(a).unwrap()),
            Err(MoneyError::Negative)
        );
        let max = Money {
            asset_id: usdc,
            units: MinorUnits::MAX,
        };
        assert_eq!(max.checked_add(a), Err(MoneyError::OutOfRange));
    }
}
