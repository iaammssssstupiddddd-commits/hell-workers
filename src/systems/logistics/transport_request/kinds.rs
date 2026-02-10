use bevy::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum TransportRequestKind {
    DepositToStockpile,
    DeliverToBlueprint,
    DeliverToMixerSolid,
    DeliverWaterToMixer,
    ReturnBucket,
    BatchWheelbarrow,
}
