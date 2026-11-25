//! Transaction limit foundation (IMP-P4-007).
//!
//! Exact validation of a transaction amount against its asset, and of that
//! amount against limit rules (FRS §23.3, database design §10.6). Everything is
//! integer arithmetic on minor units; nothing uses floating point.
//!
//! No thresholds, customer tiers, scope vocabulary, window boundaries or
//! cross-asset valuation rule is approved (Risk owns them). Until they are, a
//! check passes only when every applicable rule can be decided exactly:
//!
//! - No applicable rule denies. The absence of a limit is not permission.
//! - A rule in another asset, or spanning assets, needs an approved valuation
//!   rule, so it denies. Converting or switching assets cannot escape a limit.
//! - A windowed rule (daily, monthly and so on) can be enforced only by
//!   consuming capacity atomically in the transaction that accepts the
//!   operation (database design §10.6). A read-only check cannot do that, so
//!   it denies. [`crate::limit_usage`] holds the decisions that transaction
//!   will make under its lock.
//! - A per-transaction rule in the amount's own asset is decided exactly. An
//!   amount equal to the limit is within it.
//!
//! Rule selection is the caller's job: it must pass only enabled rules in
//! force for the customer. Rules for other operations are ignored; rules in
//! other assets are treated as applicable, because the stored `asset_id`
//! could be the limit's unit rather than a filter.
use crate::entities::Asset;
use crate::money::Money;
use crate::operations::FinancialOperation;
use crate::units::MinorUnits;
use uuid::Uuid;

/// Minor units carry at most 38 decimal digits (`payments.minor_units`).
const MAX_DIGITS: usize = 38;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmountError {
    /// Not a plain decimal string of ASCII digits with an optional fraction.
    Malformed,
    /// More fractional digits than the asset's accounting precision.
    TooPrecise,
    /// More than 38 digits of minor units.
    Overflow,
    /// Zero. A transaction amount must be positive (`payments.transactions`).
    NotPositive,
    /// The asset is disabled or has an invalid precision.
    UnsupportedAsset,
}

/// A positive amount of an enabled asset in exact minor units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransactionAmount(Money);

impl TransactionAmount {
    /// Parses a public decimal amount (API §5.6) such as `"25.50"` into minor
    /// units of `asset`. Precision is validated, never rounded (API §5.9):
    /// `"1.0000000"` is too precise for a six-decimal asset.
    pub fn parse(text: &str, asset: &Asset) -> Result<Self, AmountError> {
        let precision = supported_precision(asset)?;
        let (whole, fraction) = match text.split_once('.') {
            Some((whole, fraction)) if !fraction.is_empty() => (whole, fraction),
            Some(_) => return Err(AmountError::Malformed),
            None => (text, ""),
        };
        let digits = |part: &str| part.bytes().all(|b| b.is_ascii_digit());
        if whole.is_empty() || !digits(whole) || !digits(fraction) {
            return Err(AmountError::Malformed);
        }
        let padding = precision
            .checked_sub(fraction.len())
            .ok_or(AmountError::TooPrecise)?;
        let mut value: u128 = 0;
        let mut significant = 0;
        for digit in whole
            .bytes()
            .chain(fraction.bytes())
            .skip_while(|b| *b == b'0')
        {
            significant += 1;
            if significant > MAX_DIGITS {
                return Err(AmountError::Overflow);
            }
            value = value * 10 + u128::from(digit - b'0');
        }
        if value == 0 {
            return Err(AmountError::NotPositive);
        }
        if significant + padding > MAX_DIGITS {
            return Err(AmountError::Overflow);
        }
        // `significant + padding <= 38`, so neither step can overflow.
        let scale = 10_u128.pow(u32::try_from(padding).map_err(|_| AmountError::Overflow)?);
        let units = MinorUnits::new(value * scale).ok_or(AmountError::Overflow)?;
        Ok(Self(Money {
            asset_id: asset.id,
            units,
        }))
    }

    /// Validates an amount already held in minor units of `asset`.
    pub fn from_units(units: MinorUnits, asset: &Asset) -> Result<Self, AmountError> {
        supported_precision(asset)?;
        if units.value() == 0 {
            return Err(AmountError::NotPositive);
        }
        Ok(Self(Money {
            asset_id: asset.id,
            units,
        }))
    }

    pub const fn money(self) -> Money {
        self.0
    }
}

fn supported_precision(asset: &Asset) -> Result<usize, AmountError> {
    if !asset.enabled {
        return Err(AmountError::UnsupportedAsset);
    }
    usize::try_from(asset.accounting_precision).map_err(|_| AmountError::UnsupportedAsset)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitPeriod {
    /// Each transaction on its own.
    PerTransaction,
    /// Usage accumulated over a time window. No window boundaries are approved.
    Windowed,
}

/// A limit rule in force for the customer (`compliance.transaction_limits`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LimitRule {
    pub id: Uuid,
    pub operation: FinancialOperation,
    /// `None` spans assets, which needs an approved valuation rule.
    pub asset_id: Option<Uuid>,
    pub period: LimitPeriod,
    /// Minor units of `asset_id`.
    pub limit: MinorUnits,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitDenial {
    /// No approved limit configuration exists.
    LimitsUndefined,
    /// Limit configuration could not be loaded.
    LimitsUnavailable,
    /// No rule applies to the operation.
    NoApplicableLimits,
    /// The amount exceeds a per-transaction limit.
    Exceeded { rule_id: Uuid },
    /// The rule is in another asset or spans assets, and no valuation rule is
    /// approved.
    ValuationUndefined { rule_id: Uuid },
    /// The rule accumulates usage over a window, which only atomic consumption
    /// in the accepting transaction can enforce.
    RequiresAtomicConsumption { rule_id: Uuid },
}

/// The amount is within every applicable per-transaction limit.
///
/// It is neither an authorization nor a consumption of limit capacity:
/// eligibility, ownership, confirmation and risk controls still apply. Only
/// [`check`] creates it, and it is neither `Clone` nor `Copy`.
#[derive(Debug, PartialEq, Eq)]
pub struct WithinLimits {
    operation: FinancialOperation,
    amount: Money,
}

impl WithinLimits {
    pub const fn operation(&self) -> FinancialOperation {
        self.operation
    }

    pub const fn amount(&self) -> Money {
        self.amount
    }
}

/// Checks `amount` for `operation` against `rules`. An exceeded limit is
/// reported ahead of any rule that cannot be decided.
pub fn check(
    operation: FinancialOperation,
    amount: TransactionAmount,
    rules: &[LimitRule],
) -> Result<WithinLimits, LimitDenial> {
    let money = amount.money();
    let mut applicable = false;
    let mut undecidable = None;
    for rule in rules.iter().filter(|rule| rule.operation == operation) {
        applicable = true;
        let rule_id = rule.id;
        let denial = match (rule.asset_id, rule.period) {
            (Some(asset), _) if asset != money.asset_id => {
                LimitDenial::ValuationUndefined { rule_id }
            }
            (None, _) => LimitDenial::ValuationUndefined { rule_id },
            (Some(_), LimitPeriod::Windowed) => LimitDenial::RequiresAtomicConsumption { rule_id },
            (Some(_), LimitPeriod::PerTransaction) if money.units.value() > rule.limit.value() => {
                return Err(LimitDenial::Exceeded { rule_id });
            }
            (Some(_), LimitPeriod::PerTransaction) => continue,
        };
        undecidable.get_or_insert(denial);
    }
    if !applicable {
        return Err(LimitDenial::NoApplicableLimits);
    }
    if let Some(denial) = undecidable {
        return Err(denial);
    }
    Ok(WithinLimits {
        operation,
        amount: money,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use FinancialOperation::*;

    const USDC: Uuid = Uuid::from_u128(0xA1);
    const USDT: Uuid = Uuid::from_u128(0xA2);
    const MAX: u128 = 10_u128.pow(38) - 1;

    fn asset(id: Uuid, precision: i16, enabled: bool) -> Asset {
        Asset {
            id,
            symbol: "TEST".to_owned(),
            accounting_precision: precision,
            enabled,
        }
    }

    fn usdc() -> Asset {
        asset(USDC, 6, true)
    }

    fn units(value: u128) -> MinorUnits {
        MinorUnits::new(value).unwrap()
    }

    fn amount(value: u128) -> TransactionAmount {
        TransactionAmount::from_units(units(value), &usdc()).unwrap()
    }

    fn rule(
        id: u128,
        operation: FinancialOperation,
        asset_id: Option<Uuid>,
        period: LimitPeriod,
        limit: u128,
    ) -> LimitRule {
        LimitRule {
            id: Uuid::from_u128(id),
            operation,
            asset_id,
            period,
            limit: units(limit),
        }
    }

    #[test]
    fn decimal_amounts_convert_exactly() {
        for (text, precision, expected) in [
            ("25.50", 6, 25_500_000),
            ("25.5", 6, 25_500_000),
            ("0.000001", 6, 1),
            ("1", 0, 1),
            ("007.10", 2, 710),
            ("1", 6, 1_000_000),
            ("99999999999999999999999999999999.999999", 6, MAX),
            ("99999999999999999999999999999999999999", 0, MAX),
            ("0.99999999999999999999999999999999999999", 38, MAX),
            ("0.1", 38, 10_u128.pow(37)),
        ] {
            let parsed = TransactionAmount::parse(text, &asset(USDC, precision, true));
            assert_eq!(
                parsed.map(|a| a.money().units.value()),
                Ok(expected),
                "{text}"
            );
        }
    }

    #[test]
    fn malformed_amounts_are_rejected() {
        for text in [
            "", " ", "1 ", " 1", "+1", "-1", "1.", ".5", "1..0", "1.2.3", "1e6", "0x10", "1,000",
            "١", "NaN", "inf", "1_000", "Ⅰ",
        ] {
            assert_eq!(
                TransactionAmount::parse(text, &usdc()),
                Err(AmountError::Malformed),
                "{text:?}"
            );
        }
    }

    #[test]
    fn precision_is_validated_never_rounded() {
        for (text, precision) in [("0.0000001", 6), ("1.0000000", 6), ("0.5", 0), ("1.10", 1)] {
            assert_eq!(
                TransactionAmount::parse(text, &asset(USDC, precision, true)),
                Err(AmountError::TooPrecise),
                "{text}"
            );
        }
        // Negative precisions are invalid; the database forbids them too.
        assert_eq!(
            TransactionAmount::parse("1", &asset(USDC, -1, true)),
            Err(AmountError::UnsupportedAsset)
        );
    }

    #[test]
    fn zero_and_overflowing_amounts_are_rejected() {
        for text in ["0", "0.0", "000.000000"] {
            assert_eq!(
                TransactionAmount::parse(text, &usdc()),
                Err(AmountError::NotPositive),
                "{text}"
            );
        }
        for (text, precision) in [
            ("100000000000000000000000000000000", 6),
            ("100000000000000000000000000000000000000", 0),
            ("1", 38),
            ("0.1", 39),
            ("1", i16::MAX),
            (
                "999999999999999999999999999999999999999999999999999999999999999999",
                0,
            ),
        ] {
            assert_eq!(
                TransactionAmount::parse(text, &asset(USDC, precision, true)),
                Err(AmountError::Overflow),
                "{text} at {precision}"
            );
        }
        assert_eq!(
            TransactionAmount::from_units(units(0), &usdc()),
            Err(AmountError::NotPositive)
        );
    }

    #[test]
    fn unsupported_assets_are_rejected_before_the_amount() {
        let disabled = asset(USDC, 6, false);
        for text in ["1", "0", "x", "1.0000001"] {
            assert_eq!(
                TransactionAmount::parse(text, &disabled),
                Err(AmountError::UnsupportedAsset)
            );
        }
        assert_eq!(
            TransactionAmount::from_units(units(1), &disabled),
            Err(AmountError::UnsupportedAsset)
        );
    }

    #[test]
    fn per_transaction_boundaries_are_exact() {
        let limit = rule(
            1,
            Withdrawal,
            Some(USDC),
            LimitPeriod::PerTransaction,
            1_000_000,
        );
        for (value, within) in [
            (1, true),
            (999_999, true),
            (1_000_000, true),
            (1_000_001, false),
            (MAX, false),
        ] {
            let result = check(Withdrawal, amount(value), &[limit]);
            assert_eq!(result.is_ok(), within, "{value}");
            if !within {
                assert_eq!(result, Err(LimitDenial::Exceeded { rule_id: limit.id }));
            }
        }
        let largest = rule(2, Withdrawal, Some(USDC), LimitPeriod::PerTransaction, MAX);
        let within = check(Withdrawal, amount(MAX), &[largest]).unwrap();
        assert_eq!(
            (within.operation(), within.amount().units.value()),
            (Withdrawal, MAX)
        );
        let zero = rule(3, Withdrawal, Some(USDC), LimitPeriod::PerTransaction, 0);
        assert_eq!(
            check(Withdrawal, amount(1), &[zero]),
            Err(LimitDenial::Exceeded { rule_id: zero.id })
        );
    }

    #[test]
    fn missing_limits_deny() {
        assert_eq!(
            check(Withdrawal, amount(1), &[]),
            Err(LimitDenial::NoApplicableLimits)
        );
        // Rules for other operations do not apply.
        let deposit = rule(1, Deposit, Some(USDC), LimitPeriod::PerTransaction, MAX);
        assert_eq!(
            check(Withdrawal, amount(1), &[deposit]),
            Err(LimitDenial::NoApplicableLimits)
        );
    }

    #[test]
    fn rules_that_cannot_be_decided_deny() {
        let other_asset = rule(1, Conversion, Some(USDT), LimitPeriod::PerTransaction, MAX);
        let any_asset = rule(2, Conversion, None, LimitPeriod::PerTransaction, MAX);
        let daily = rule(3, Conversion, Some(USDC), LimitPeriod::Windowed, MAX);
        let fits = rule(4, Conversion, Some(USDC), LimitPeriod::PerTransaction, MAX);
        assert_eq!(
            check(Conversion, amount(1), &[fits, other_asset]),
            Err(LimitDenial::ValuationUndefined {
                rule_id: other_asset.id
            })
        );
        assert_eq!(
            check(Conversion, amount(1), &[any_asset, fits]),
            Err(LimitDenial::ValuationUndefined {
                rule_id: any_asset.id
            })
        );
        // A windowed rule needs atomic consumption, so a read-only check never passes it.
        assert_eq!(
            check(Conversion, amount(1), &[fits, daily]),
            Err(LimitDenial::RequiresAtomicConsumption { rule_id: daily.id })
        );
        // An exceeded limit is reported first, whatever the order.
        let tight = rule(5, Conversion, Some(USDC), LimitPeriod::PerTransaction, 0);
        for rules in [[daily, other_asset, tight], [tight, daily, other_asset]] {
            assert_eq!(
                check(Conversion, amount(1), &rules),
                Err(LimitDenial::Exceeded { rule_id: tight.id })
            );
        }
    }

    #[test]
    fn every_applicable_rule_must_pass() {
        let loose = rule(1, BankPayment, Some(USDC), LimitPeriod::PerTransaction, 10);
        let strict = rule(2, BankPayment, Some(USDC), LimitPeriod::PerTransaction, 5);
        assert!(check(BankPayment, amount(5), &[loose, strict]).is_ok());
        assert_eq!(
            check(BankPayment, amount(6), &[loose, strict]),
            Err(LimitDenial::Exceeded { rule_id: strict.id })
        );
    }
}
