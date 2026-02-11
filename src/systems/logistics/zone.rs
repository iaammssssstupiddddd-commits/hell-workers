use super::types::ResourceType;
use bevy::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum ZoneType {
    Stockpile,
}

#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct Stockpile {
    pub capacity: usize,
    /// 最初に格納された資源の種類。空の場合は None。
    pub resource_type: Option<ResourceType>,
}
