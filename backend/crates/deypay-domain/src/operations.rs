//! Financial operation kinds shared by eligibility and limit checks.
use std::{fmt, str::FromStr};

/// The transaction types in the `payments.transactions.transaction_type`
/// constraint. Parsing is exact; any other value is unknown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FinancialOperation {
    Deposit,
    Withdrawal,
    InternalTransfer,
    BankPayment,
    BankWithdrawal,
    FundingOrder,
    Conversion,
    Adjustment,
    Reversal,
}

impl FinancialOperation {
    pub const ALL: [Self; 9] = [
        Self::Deposit,
        Self::Withdrawal,
        Self::InternalTransfer,
        Self::BankPayment,
        Self::BankWithdrawal,
        Self::FundingOrder,
        Self::Conversion,
        Self::Adjustment,
        Self::Reversal,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Deposit => "DEPOSIT",
            Self::Withdrawal => "WITHDRAWAL",
            Self::InternalTransfer => "INTERNAL_TRANSFER",
            Self::BankPayment => "BANK_PAYMENT",
            Self::BankWithdrawal => "BANK_WITHDRAWAL",
            Self::FundingOrder => "FUNDING_ORDER",
            Self::Conversion => "CONVERSION",
            Self::Adjustment => "ADJUSTMENT",
            Self::Reversal => "REVERSAL",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnknownFinancialOperation;

impl FromStr for FinancialOperation {
    type Err = UnknownFinancialOperation;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|operation| operation.as_str() == value)
            .ok_or(UnknownFinancialOperation)
    }
}

impl fmt::Display for FinancialOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operations_round_trip_exactly() {
        for operation in FinancialOperation::ALL {
            assert_eq!(operation.as_str().parse(), Ok(operation));
        }
        for unknown in ["", "deposit", " DEPOSIT", "PAYOUT", "WITHDRAWAL "] {
            assert_eq!(
                unknown.parse::<FinancialOperation>(),
                Err(UnknownFinancialOperation)
            );
        }
    }
}
