use bevy::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum TransportRequestKind {
    DepositToStockpile,
    DeliverToBlueprint,
    DeliverToFloorConstruction,
    DeliverToWallConstruction,
    DeliverToProvisionalWall,
    DeliverToMixerSolid,
    DeliverWaterToMixer,
    GatherWaterToTank,
    ReturnBucket,
    ReturnWheelbarrow,
    BatchWheelbarrow,
    ConsolidateStockpile,
    DeliverToSoulSpa,
}

impl TransportRequestKind {
    pub const ALL: [Self; 13] = [
        Self::DepositToStockpile,
        Self::DeliverToBlueprint,
        Self::DeliverToFloorConstruction,
        Self::DeliverToWallConstruction,
        Self::DeliverToProvisionalWall,
        Self::DeliverToMixerSolid,
        Self::DeliverWaterToMixer,
        Self::GatherWaterToTank,
        Self::ReturnBucket,
        Self::ReturnWheelbarrow,
        Self::BatchWheelbarrow,
        Self::ConsolidateStockpile,
        Self::DeliverToSoulSpa,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DepositToStockpile => "deposit-to-stockpile",
            Self::DeliverToBlueprint => "deliver-to-blueprint",
            Self::DeliverToFloorConstruction => "deliver-to-floor-construction",
            Self::DeliverToWallConstruction => "deliver-to-wall-construction",
            Self::DeliverToProvisionalWall => "deliver-to-provisional-wall",
            Self::DeliverToMixerSolid => "deliver-to-mixer-solid",
            Self::DeliverWaterToMixer => "deliver-water-to-mixer",
            Self::GatherWaterToTank => "gather-water-to-tank",
            Self::ReturnBucket => "return-bucket",
            Self::ReturnWheelbarrow => "return-wheelbarrow",
            Self::BatchWheelbarrow => "batch-wheelbarrow",
            Self::ConsolidateStockpile => "consolidate-stockpile",
            Self::DeliverToSoulSpa => "deliver-to-soul-spa",
        }
    }

    #[cfg(feature = "profiling")]
    pub(crate) const fn metric_index(self) -> usize {
        match self {
            Self::DepositToStockpile => 0,
            Self::DeliverToBlueprint => 1,
            Self::DeliverToFloorConstruction => 2,
            Self::DeliverToWallConstruction => 3,
            Self::DeliverToProvisionalWall => 4,
            Self::DeliverToMixerSolid => 5,
            Self::DeliverWaterToMixer => 6,
            Self::GatherWaterToTank => 7,
            Self::ReturnBucket => 8,
            Self::ReturnWheelbarrow => 9,
            Self::BatchWheelbarrow => 10,
            Self::ConsolidateStockpile => 11,
            Self::DeliverToSoulSpa => 12,
        }
    }
}
