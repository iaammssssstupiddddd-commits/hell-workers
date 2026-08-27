use bevy::prelude::*;
use hw_core::logistics::{ResourceSourceKey, ResourceType, WheelbarrowDestination};

#[derive(Reflect, Clone, Debug, PartialEq)]
pub struct HaulWithWheelbarrowData {
    pub wheelbarrow: Entity,
    pub source_pos: Vec2,
    pub destination: WheelbarrowDestination,
    pub collect_source: Option<ResourceSourceKey>,
    pub collect_amount: u32,
    pub collect_resource_type: Option<ResourceType>,
    pub items: Vec<Entity>,
    pub phase: HaulWithWheelbarrowPhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Default)]
pub enum HaulWithWheelbarrowPhase {
    #[default]
    GoingToParking,
    PickingUpWheelbarrow,
    GoingToSource,
    Loading,
    GoingToDestination,
    Unloading,
    ReturningWheelbarrow,
}
