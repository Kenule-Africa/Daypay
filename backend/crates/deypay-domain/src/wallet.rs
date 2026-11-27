//! Wallet and network rules (IMP-P6-002–007). They mirror the PostgreSQL
//! constraints in migration `202611261401_wallet_asset_management.sql`, which
//! remain authoritative; these give the application the same answers early.

/// A supported blockchain family. Network support is explicit per network;
/// a family only decides how addresses are written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChainFamily {
    Evm,
    Solana,
}

const BASE58: &[u8; 58] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

/// Number of bytes a base58 string decodes to, or `None` if it is not base58.
fn base58_byte_length(text: &str) -> Option<usize> {
    if text.is_empty() || text.len() > 64 {
        return None;
    }
    let mut bytes: Vec<u8> = Vec::new();
    let mut leading_zeros = 0;
    let mut leading = true;
    for character in text.bytes() {
        let digit = BASE58.iter().position(|&c| c == character)?;
        if leading && digit == 0 {
            leading_zeros += 1;
            continue;
        }
        leading = false;
        // bytes is a big-endian number; multiply by 58 and add the digit.
        let mut carry = digit;
        for byte in bytes.iter_mut().rev() {
            carry += usize::from(*byte) * 58;
            *byte = (carry & 0xff) as u8;
            carry >>= 8;
        }
        while carry > 0 {
            bytes.insert(0, (carry & 0xff) as u8);
            carry >>= 8;
        }
    }
    Some(leading_zeros + bytes.len())
}

impl ChainFamily {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "EVM" => Some(Self::Evm),
            "SOLANA" => Some(Self::Solana),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Evm => "EVM",
            Self::Solana => "SOLANA",
        }
    }

    /// The canonical form of an address or token contract on this family, or
    /// `None` when the text is not one. EVM addresses are lowercased (the zero
    /// address is refused); Solana addresses must decode to 32 bytes.
    pub fn canonical_address(self, text: &str) -> Option<String> {
        match self {
            Self::Evm => {
                let hex = text.strip_prefix("0x")?;
                let valid = hex.len() == 40
                    && hex.bytes().all(|b| b.is_ascii_hexdigit())
                    && hex.bytes().any(|b| b != b'0');
                valid.then(|| format!("0x{}", hex.to_ascii_lowercase()))
            }
            Self::Solana => {
                let valid = (32..=44).contains(&text.len()) && base58_byte_length(text) == Some(32);
                valid.then(|| text.to_owned())
            }
        }
    }
}

/// Network codes are lowercase, start with a letter and may contain digits and
/// hyphens, for example `base` or `polygon-pos`.
pub fn valid_network_code(code: &str) -> bool {
    let bytes = code.as_bytes();
    (2..=32).contains(&bytes.len())
        && bytes[0].is_ascii_lowercase()
        && bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
}

/// Customer wallet status (API §18.6). Database values are uppercase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalletStatus {
    Provisioning,
    Active,
    Restricted,
    Suspended,
    Closed,
}

impl WalletStatus {
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "PROVISIONING" => Self::Provisioning,
            "ACTIVE" => Self::Active,
            "RESTRICTED" => Self::Restricted,
            "SUSPENDED" => Self::Suspended,
            "CLOSED" => Self::Closed,
            _ => return None,
        })
    }

    pub const fn as_db(self) -> &'static str {
        match self {
            Self::Provisioning => "PROVISIONING",
            Self::Active => "ACTIVE",
            Self::Restricted => "RESTRICTED",
            Self::Suspended => "SUSPENDED",
            Self::Closed => "CLOSED",
        }
    }

    pub const fn public_name(self) -> &'static str {
        match self {
            Self::Provisioning => "provisioning",
            Self::Active => "active",
            Self::Restricted => "restricted",
            Self::Suspended => "suspended",
            Self::Closed => "closed",
        }
    }

    /// Allowed transitions. Provisioning ends only in active, once the ledger
    /// accounts exist; closed is final.
    pub const fn can_become(self, next: Self) -> bool {
        match (self, next) {
            (Self::Provisioning, Self::Active) => true,
            (
                Self::Active | Self::Restricted | Self::Suspended,
                Self::Active | Self::Restricted | Self::Suspended | Self::Closed,
            ) => self as u8 != next as u8,
            _ => false,
        }
    }
}

/// Reasons a wallet is limited, as reported by `wallets.restrictions`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Restriction {
    WalletProvisioning,
    WalletRestricted,
    WalletSuspended,
    WalletClosed,
    AccountRestricted,
    KycRequired,
    AssetUnavailable,
    AssetOnHold,
    FinancialActivityNotEnabled,
}

impl Restriction {
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "WALLET_PROVISIONING" => Self::WalletProvisioning,
            "WALLET_RESTRICTED" => Self::WalletRestricted,
            "WALLET_SUSPENDED" => Self::WalletSuspended,
            "WALLET_CLOSED" => Self::WalletClosed,
            "ACCOUNT_RESTRICTED" => Self::AccountRestricted,
            "KYC_REQUIRED" => Self::KycRequired,
            "ASSET_UNAVAILABLE" => Self::AssetUnavailable,
            "ASSET_ON_HOLD" => Self::AssetOnHold,
            "FINANCIAL_ACTIVITY_NOT_ENABLED" => Self::FinancialActivityNotEnabled,
            _ => return None,
        })
    }

    pub const fn public_name(self) -> &'static str {
        match self {
            Self::WalletProvisioning => "wallet_provisioning",
            Self::WalletRestricted => "wallet_restricted",
            Self::WalletSuspended => "wallet_suspended",
            Self::WalletClosed => "wallet_closed",
            Self::AccountRestricted => "account_restricted",
            Self::KycRequired => "kyc_required",
            Self::AssetUnavailable => "asset_unavailable",
            Self::AssetOnHold => "asset_on_hold",
            Self::FinancialActivityNotEnabled => "financial_activity_not_enabled",
        }
    }

    /// Restrictions that come from outside the wallet's own status and make an
    /// otherwise active wallet effectively restricted.
    pub const fn limits_active_wallet(self) -> bool {
        matches!(
            self,
            Self::AccountRestricted | Self::AssetUnavailable | Self::AssetOnHold
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evm_addresses_are_validated_and_lowercased() {
        let mixed = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";
        assert_eq!(
            ChainFamily::Evm.canonical_address(mixed).as_deref(),
            Some("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")
        );
        for bad in [
            "",
            "0x",
            "a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
            "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb4",
            "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb488",
            "0xg0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
            "0x0000000000000000000000000000000000000000",
            " 0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
        ] {
            assert_eq!(ChainFamily::Evm.canonical_address(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn solana_addresses_must_decode_to_32_bytes() {
        for good in [
            "11111111111111111111111111111111",
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
            "So11111111111111111111111111111111111111112",
        ] {
            assert_eq!(
                ChainFamily::Solana.canonical_address(good).as_deref(),
                Some(good)
            );
        }
        for bad in [
            "",
            "1111111111111111111111111111111",
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTD",
            "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
            "0PjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1vEPjF",
            "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
        ] {
            assert_eq!(ChainFamily::Solana.canonical_address(bad), None, "{bad:?}");
        }
        assert_eq!(base58_byte_length("z"), Some(1));
        assert_eq!(base58_byte_length("1z"), Some(2));
        assert_eq!(base58_byte_length("5Q"), Some(1), "4 × 58 + 23 = 255");
        assert_eq!(base58_byte_length("5R"), Some(2), "4 × 58 + 24 = 256");
        assert_eq!(base58_byte_length("I"), None);
    }

    #[test]
    fn network_codes_are_explicit_lowercase_identifiers() {
        for good in ["base", "polygon-pos", "ethereum", "sol2"] {
            assert!(valid_network_code(good), "{good}");
        }
        for bad in [
            "",
            "b",
            "Base",
            "2base",
            "base_main",
            "base net",
            &"a".repeat(33),
        ] {
            assert!(!valid_network_code(bad), "{bad}");
        }
    }

    #[test]
    fn wallet_status_transitions_match_the_database() {
        use WalletStatus::*;
        let all = [Provisioning, Active, Restricted, Suspended, Closed];
        for from in all {
            for to in all {
                let expected = match (from, to) {
                    (Provisioning, Active) => true,
                    (Active | Restricted | Suspended, Active | Restricted | Suspended | Closed) => {
                        from != to
                    }
                    _ => false,
                };
                assert_eq!(from.can_become(to), expected, "{from:?} → {to:?}");
            }
            assert_eq!(WalletStatus::parse(from.as_db()), Some(from));
        }
        assert_eq!(WalletStatus::parse("active"), None);
    }

    #[test]
    fn restrictions_round_trip_and_classify() {
        for code in [
            "WALLET_PROVISIONING",
            "WALLET_RESTRICTED",
            "WALLET_SUSPENDED",
            "WALLET_CLOSED",
            "ACCOUNT_RESTRICTED",
            "KYC_REQUIRED",
            "ASSET_UNAVAILABLE",
            "ASSET_ON_HOLD",
            "FINANCIAL_ACTIVITY_NOT_ENABLED",
        ] {
            let parsed = Restriction::parse(code).unwrap();
            assert_eq!(parsed.public_name(), code.to_ascii_lowercase());
        }
        assert!(Restriction::AssetOnHold.limits_active_wallet());
        assert!(!Restriction::KycRequired.limits_active_wallet());
        assert_eq!(Restriction::parse("UNKNOWN"), None);
    }
}
