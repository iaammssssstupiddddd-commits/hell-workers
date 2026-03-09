//! Floor construction system components

use crate::systems::command::TaskArea;
use bevy::prelude::*;
pub use hw_jobs::construction::{FloorConstructionPhase, FloorTileState};

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

/// Individual floor tile blueprint - child entity
#[derive(Component, Clone, Debug)]
pub struct FloorTileBlueprint {
    pub parent_site: Entity,
    pub grid_pos: (i32, i32),
    pub state: FloorTileState,
    /// Bones delivered (0-2)
    pub bones_delivered: u32,
    /// Mud delivered (0-1)
    pub mud_delivered: u32,
}

impl FloorTileBlueprint {
    pub fn new(parent_site: Entity, grid_pos: (i32, i32)) -> Self {
        Self {
            parent_site,
            grid_pos,
            state: FloorTileState::WaitingBones,
            bones_delivered: 0,
            mud_delivered: 0,
        }
    }
}

/// Marker component linking a TransportRequest to a FloorConstructionSite
#[derive(Component, Clone, Copy, Debug)]
pub struct TargetFloorConstructionSite(pub Entity);

/// Marker component requesting cancellation of an entire floor construction site.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct FloorConstructionCancelRequested;
