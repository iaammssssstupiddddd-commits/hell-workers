//! Floor construction system components

use crate::systems::command::TaskArea;
use bevy::prelude::*;
pub use hw_jobs::construction::{
    FloorConstructionCancelRequested, FloorConstructionPhase, FloorTileBlueprint, FloorTileState,
    TargetFloorConstructionSite,
};

/// Floor construction site - parent entity managing an area of floor tiles
#[derive(Component, Clone, Debug)]
pub struct FloorConstructionSite {
    pub phase: FloorConstructionPhase,
    pub area_bounds: TaskArea,
    /// Central point where materials are delivered
    pub material_center: Vec2,
    pub tiles_total: u32,
    pub tiles_reinforced: u32,
    pub tiles_poured: u32,
    /// Remaining curing time in seconds (used while `phase == Curing`)
    pub curing_remaining_secs: f32,
}

impl FloorConstructionSite {
    pub fn new(area_bounds: TaskArea, material_center: Vec2, tiles_total: u32) -> Self {
        Self {
            phase: FloorConstructionPhase::Reinforcing,
            area_bounds,
            material_center,
            tiles_total,
            tiles_reinforced: 0,
            tiles_poured: 0,
            curing_remaining_secs: 0.0,
        }
    }
}
