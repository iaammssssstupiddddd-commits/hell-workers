use bevy::prelude::*;
use hw_core::logistics::ResourceType;

pub mod demand;
pub mod helpers;
pub mod planning;
pub mod supply;

#[derive(Component, Debug, Clone, Copy)]
pub struct AutoGatherDesignation {
    pub owner: Entity,
    pub resource_type: ResourceType,
}
