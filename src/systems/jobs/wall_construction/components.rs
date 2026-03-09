//! Wall construction system components

use crate::systems::command::TaskArea;
use bevy::prelude::*;
pub use hw_jobs::construction::{
    TargetWallConstructionSite, WallConstructionCancelRequested, WallConstructionPhase,
    WallTileBlueprint, WallTileState,
};

/// Wall construction site - parent entity managing a line of wall tiles
#[derive(Component, Clone, Debug)]
pub struct WallConstructionSite {
    pub phase: WallConstructionPhase,
    pub area_bounds: TaskArea,
    /// Central point where materials are delivered
    pub material_center: Vec2,
    pub tiles_total: u32,
    pub tiles_framed: u32,
    pub tiles_coated: u32,
}

impl WallConstructionSite {
    pub fn new(area_bounds: TaskArea, material_center: Vec2, tiles_total: u32) -> Self {
        Self {
            phase: WallConstructionPhase::Framing,
            area_bounds,
            material_center,
            tiles_total,
            tiles_framed: 0,
            tiles_coated: 0,
        }
    }
}
