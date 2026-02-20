//! Wall construction system components

use crate::systems::command::TaskArea;
use bevy::prelude::*;

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

/// Construction phase for the entire wall site
#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect)]
pub enum WallConstructionPhase {
    /// Build provisional wall frame using wood
    Framing,
    /// Coat provisional wall with stasis mud to finalize
    Coating,
}

/// Individual wall tile blueprint - child entity
#[derive(Component, Clone, Debug)]
pub struct WallTileBlueprint {
    pub parent_site: Entity,
    pub grid_pos: (i32, i32),
    pub state: WallTileState,
    /// Wood delivered (0-1)
    pub wood_delivered: u32,
    /// Mud delivered (0-1)
    pub mud_delivered: u32,
    /// Spawned provisional/permanent wall entity after framing
    pub spawned_wall: Option<Entity>,
}

impl WallTileBlueprint {
    pub fn new(parent_site: Entity, grid_pos: (i32, i32)) -> Self {
        Self {
            parent_site,
            grid_pos,
            state: WallTileState::WaitingWood,
            wood_delivered: 0,
            mud_delivered: 0,
            spawned_wall: None,
        }
    }
}

/// State of individual wall tile
#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect)]
pub enum WallTileState {
    /// Waiting for wood to be delivered
    WaitingWood,
    /// Wood delivered, ready for worker to frame
    FramingReady,
    /// Worker is actively framing
    Framing { progress: u8 },
    /// Framing complete and provisional wall is spawned
    FramedProvisional,
    /// Waiting for mud to be delivered (after phase transition)
    WaitingMud,
    /// Mud delivered, ready for worker to coat
    CoatingReady,
    /// Worker is actively coating
    Coating { progress: u8 },
    /// Construction complete
    Complete,
}

/// Marker component linking a TransportRequest to a WallConstructionSite
#[derive(Component, Clone, Copy, Debug)]
pub struct TargetWallConstructionSite(pub Entity);

/// Marker component requesting cancellation of an entire wall construction site.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct WallConstructionCancelRequested;
