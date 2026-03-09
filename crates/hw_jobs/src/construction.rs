use bevy::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect)]
pub enum FloorConstructionPhase {
    /// Placing bones as reinforcement
    Reinforcing,
    /// Pouring mud as concrete
    Pouring,
    /// Waiting for poured tiles to cure while area is blocked
    Curing,
}

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect)]
pub enum WallConstructionPhase {
    /// Build provisional wall frame using wood
    Framing,
    /// Coat provisional wall with stasis mud to finalize
    Coating,
}

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

/// Marker component linking a TransportRequest to a WallConstructionSite
#[derive(Component, Clone, Copy, Debug)]
pub struct TargetWallConstructionSite(pub Entity);

/// Marker component requesting cancellation of an entire wall construction site.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct WallConstructionCancelRequested;
