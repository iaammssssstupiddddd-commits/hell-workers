use bevy::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum TransportRequestKind {
    DepositToStockpile,
    DeliverToBlueprint,
    DeliverToFloorConstruction,
    DeliverToProvisionalWall,
    DeliverToMixerSolid,
    DeliverWaterToMixer,
    GatherWaterToTank,
    ReturnBucket,
    ReturnWheelbarrow,
    BatchWheelbarrow,
    ConsolidateStockpile,
}
