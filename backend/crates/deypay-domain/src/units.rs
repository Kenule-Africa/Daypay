//! Exact, nonnegative integer accounting units (up to 38 decimal digits).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MinorUnits(u128);

impl MinorUnits {
    pub const fn new(value: u128) -> Option<Self> {
        if value < 10_u128.pow(38) {
            Some(Self(value))
        } else {
            None
        }
    }

    pub const fn value(self) -> u128 {
        self.0
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
    }
}
