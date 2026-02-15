//! Floor construction system components

use crate::systems::command::TaskArea;
use bevy::prelude::*;

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
}

impl FloorConstructionSite {
    pub fn new(area_bounds: TaskArea, tiles_total: u32) -> Self {
        let material_center = area_bounds.center();
        Self {
            phase: FloorConstructionPhase::Reinforcing,
            area_bounds,
            material_center,
            tiles_total,
            tiles_reinforced: 0,
            tiles_poured: 0,
        }
    }
}

/// Construction phase for the entire site
#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect)]
pub enum FloorConstructionPhase {
    /// Placing bones as reinforcement
    Reinforcing,
    /// Pouring mud as concrete
    Pouring,
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

/// State of individual floor tile
#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect)]
pub enum FloorTileState {
    /// Waiting for bones to be delivered
    WaitingBones,
    /// Bones delivered, ready for worker to reinforce
    ReinforcingReady,
    /// Worker is actively reinforcing
    Reinforcing { progress: u8 },
    /// Reinforcing complete, waiting for phase transition
    ReinforcedComplete,
    /// Waiting for mud to be delivered (after phase transition)
    WaitingMud,
    /// Mud delivered, ready for worker to pour
    PouringReady,
    /// Worker is actively pouring
    Pouring { progress: u8 },
    /// Construction complete
    Complete,
}
