use bevy::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum TransportRequestKind {
    DepositToStockpile,
    DeliverToBlueprint,
    DeliverToFloorConstruction,
    DeliverToMixerSolid,
    DeliverWaterToMixer,
    GatherWaterToTank,
    ReturnBucket,
    BatchWheelbarrow,
    ConsolidateStockpile,
}
