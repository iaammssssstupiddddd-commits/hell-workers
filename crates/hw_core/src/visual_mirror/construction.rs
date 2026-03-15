use bevy::prelude::*;

use crate::logistics::ResourceType;

// ── Blueprint ────────────────────────────────────────────────

/// Mirror of `hw_jobs::Blueprint` carrying only the data `hw_visual` needs.
/// Synced by `sync_blueprint_visual_system` (Changed<Blueprint>) in `hw_jobs`.
///
/// `is_wall_or_door` serves `wall_connection.rs` which needs to know the BuildingType without
/// importing it. `occupied_grids` is needed by the same system for neighbour tile updates.
#[derive(Component, Default)]
pub struct BlueprintVisualState {
    pub progress: f32,
    /// (resource_type, delivered, required)
    pub material_counts: Vec<(ResourceType, u32, u32)>,
    /// Flexible material slot: (accepted_types, delivered_total, required_total)
    pub flexible_material: Option<(Vec<ResourceType>, u32, u32)>,
    /// True when Blueprint::kind is Wall or Door (used by wall_connection.rs is_wall check)
    pub is_wall_or_door: bool,
    /// True when Blueprint::kind is Wall specifically (not Door); used by wall_connection.rs
    pub is_plain_wall: bool,
    /// Grid cells occupied by this blueprint (used by wall_connection.rs)
    pub occupied_grids: Vec<(i32, i32)>,
}

// ── FloorTile ────────────────────────────────────────────────

/// Mirror of `hw_jobs::FloorTileState`. Includes `progress: u8` variants so
/// `floor_construction.rs` can compute colour gradients.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FloorTileStateMirror {
    #[default]
    WaitingBones,
    ReinforcingReady,
    Reinforcing {
        progress: u8,
    },
    ReinforcedComplete,
    WaitingMud,
    PouringReady,
    Pouring {
        progress: u8,
    },
    Complete,
}

/// Mirror of `hw_jobs::FloorTileBlueprint` for `hw_visual`.
/// Attached to `FloorTileBlueprint` entities at spawn; synced by
/// `sync_floor_tile_visual_system` (Changed<FloorTileBlueprint>).
#[derive(Component, Default)]
pub struct FloorTileVisualMirror {
    pub state: FloorTileStateMirror,
    pub bones_delivered: u32,
}

// ── FloorSite ────────────────────────────────────────────────

/// Mirror of `hw_jobs::FloorConstructionPhase`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FloorConstructionPhaseMirror {
    #[default]
    Reinforcing,
    Pouring,
    Curing,
}

/// Mirror of `hw_jobs::FloorConstructionSite` for `hw_visual` (progress bar, phase label).
/// Attached to `FloorConstructionSite` entities at spawn; synced by
/// `sync_floor_site_visual_system`.
#[derive(Component, Default)]
pub struct FloorSiteVisualState {
    pub phase: FloorConstructionPhaseMirror,
    pub curing_remaining_secs: f32,
    pub tiles_total: u32,
}

// ── WallTile ────────────────────────────────────────────────

/// Mirror of `hw_jobs::WallTileState`. Includes `progress: u8` variants so
/// `wall_construction.rs` can compute colour gradients.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum WallTileStateMirror {
    #[default]
    WaitingWood,
    FramingReady,
    Framing {
        progress: u8,
    },
    FramedProvisional,
    WaitingMud,
    CoatingReady,
    Coating {
        progress: u8,
    },
    Complete,
}

/// Mirror of `hw_jobs::WallTileBlueprint` for `hw_visual`.
/// Attached to `WallTileBlueprint` entities at spawn; synced by
/// `sync_wall_tile_visual_system` (Changed<WallTileBlueprint>).
#[derive(Component, Default)]
pub struct WallTileVisualMirror {
    pub state: WallTileStateMirror,
}

// ── WallSite ────────────────────────────────────────────────

/// Mirror of `hw_jobs::WallConstructionSite` for `hw_visual`.
/// Attached to `WallConstructionSite` entities at spawn; synced by
/// `sync_wall_site_visual_system`.
#[derive(Component, Default)]
pub struct WallSiteVisualState {
    pub phase_is_framing: bool,
    pub tiles_total: u32,
    pub tiles_framed: u32,
    pub tiles_coated: u32,
}
