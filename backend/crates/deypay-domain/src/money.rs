use crate::units::MinorUnits;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Money {
    pub asset_id: Uuid,
    pub units: MinorUnits,
}
