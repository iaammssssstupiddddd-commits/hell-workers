use bevy::prelude::*;

use crate::types::ResourceType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum ZoneType {
    Stockpile,
    Yard,
}

#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct Stockpile {
    pub capacity: usize,
    pub resource_type: Option<ResourceType>,
}
