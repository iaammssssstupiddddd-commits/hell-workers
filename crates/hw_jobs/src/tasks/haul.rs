use bevy::prelude::*;

#[derive(Reflect, Clone, Debug, PartialEq, Eq)]
pub struct HaulData {
    pub item: Entity,
    pub stockpile: Entity,
    pub phase: HaulPhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Default)]
pub enum HaulPhase {
    #[default]
    GoingToItem,
    GoingToStockpile,
    Dropping,
}

#[derive(Reflect, Clone, Debug, PartialEq, Eq)]
pub struct HaulToBlueprintData {
    pub item: Entity,
    pub blueprint: Entity,
    pub phase: HaulToBpPhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Default)]
pub enum HaulToBpPhase {
    #[default]
    GoingToItem,
    GoingToBlueprint,
    Delivering,
}
