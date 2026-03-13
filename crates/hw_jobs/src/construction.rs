use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::area::TaskArea;
use hw_core::relationships::TaskWorkers;

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

/// 建設サイトの位置を抽象化するブリッジトレイト
///
/// `hw_familiar_ai` / `hw_soul_ai` の双方から利用するため、AI crate に依存しない
/// `hw_jobs::construction` が所有する。
pub trait ConstructionSitePositions {
    fn floor_site_pos(&self, site: Entity) -> Option<Vec2>;
    fn wall_site_pos(&self, site: Entity) -> Option<Vec2>;
}

/// 建設サイトへの読み取り専用アクセス
#[derive(SystemParam)]
pub struct ConstructionSiteAccess<'w, 's> {
    pub floor_sites: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static FloorConstructionSite,
            Option<&'static TaskWorkers>,
        ),
    >,
    pub wall_sites: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static WallConstructionSite,
            Option<&'static TaskWorkers>,
        ),
    >,
}

impl ConstructionSitePositions for ConstructionSiteAccess<'_, '_> {
    fn floor_site_pos(&self, site: Entity) -> Option<Vec2> {
        self.floor_sites
            .get(site)
            .ok()
            .map(|(t, _, _)| t.translation.truncate())
    }

    fn wall_site_pos(&self, site: Entity) -> Option<Vec2> {
        self.wall_sites
            .get(site)
            .ok()
            .map(|(t, _, _)| t.translation.truncate())
    }
}
