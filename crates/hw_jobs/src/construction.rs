use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::GridPos;
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
#[derive(Component, Clone, Debug, Reflect)]
#[reflect(Component)]
pub struct FloorTileBlueprint {
    #[entities]
    pub parent_site: Entity,
    pub grid_pos: GridPos,
    pub state: FloorTileState,
    /// Bones delivered (0-2)
    pub bones_delivered: u32,
    /// Mud delivered (0-1)
    pub mud_delivered: u32,
}

impl FloorTileBlueprint {
    pub fn new(parent_site: Entity, grid_pos: GridPos) -> Self {
        Self {
            parent_site,
            grid_pos,
            state: FloorTileState::WaitingBones,
            bones_delivered: 0,
            mud_delivered: 0,
        }
    }

    /// Returns whether this tile is a valid member of a floor site's phase
    /// transition input.
    pub fn is_reinforced_for(&self, site: Entity) -> bool {
        self.parent_site == site && self.state == FloorTileState::ReinforcedComplete
    }

    /// Applies the tile-local half of the Reinforcing -> Pouring transition.
    ///
    /// Callers must validate the complete indexed tile set before mutating any
    /// tile. Returning `false` protects direct callers from crossing site
    /// ownership or skipping the required state.
    pub fn transition_to_waiting_mud(&mut self, site: Entity) -> bool {
        if !self.is_reinforced_for(site) {
            return false;
        }
        self.state = FloorTileState::WaitingMud;
        true
    }
}

/// Marker component linking a TransportRequest to a FloorConstructionSite
#[derive(Component, Clone, Copy, Debug)]
pub struct TargetFloorConstructionSite(pub Entity);

/// Marker component requesting cancellation of an entire floor construction site.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct FloorConstructionCancelRequested;

/// Individual wall tile blueprint - child entity
#[derive(Component, Clone, Debug, Reflect)]
#[reflect(Component)]
pub struct WallTileBlueprint {
    #[entities]
    pub parent_site: Entity,
    pub grid_pos: GridPos,
    pub state: WallTileState,
    /// Wood delivered (0-1)
    pub wood_delivered: u32,
    /// Mud delivered (0-1)
    pub mud_delivered: u32,
    /// Spawned provisional/permanent wall entity after framing
    #[entities]
    pub spawned_wall: Option<Entity>,
}

impl WallTileBlueprint {
    pub fn new(parent_site: Entity, grid_pos: GridPos) -> Self {
        Self {
            parent_site,
            grid_pos,
            state: WallTileState::WaitingWood,
            wood_delivered: 0,
            mud_delivered: 0,
            spawned_wall: None,
        }
    }

    /// Returns whether this tile is a valid member of a wall site's phase
    /// transition input.
    pub fn is_framed_for(&self, site: Entity) -> bool {
        self.parent_site == site
            && self.state == WallTileState::FramedProvisional
            && self.spawned_wall.is_some()
    }

    /// Applies the tile-local half of the Framing -> Coating transition.
    ///
    /// Callers must validate the complete indexed tile set before mutating any
    /// tile.
    pub fn transition_to_waiting_mud(&mut self, site: Entity) -> bool {
        if !self.is_framed_for(site) {
            return false;
        }
        self.state = WallTileState::WaitingMud;
        true
    }
}

/// Marker component linking a TransportRequest to a WallConstructionSite
#[derive(Component, Clone, Copy, Debug)]
pub struct TargetWallConstructionSite(pub Entity);

/// Marker component requesting cancellation of an entire wall construction site.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct WallConstructionCancelRequested;

/// Floor construction site - parent entity managing an area of floor tiles
#[derive(Component, Clone, Debug, Reflect)]
#[reflect(Component)]
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

    /// Fast site-level eligibility check for Reinforcing -> Pouring.
    ///
    /// Indexed tile ownership and tile state remain authoritative and must be
    /// checked separately by the adapter that owns the index.
    pub fn can_transition_to_pouring(&self) -> bool {
        self.phase == FloorConstructionPhase::Reinforcing
            && self.tiles_total > 0
            && self.tiles_reinforced >= self.tiles_total
    }

    pub fn transition_to_pouring(&mut self) -> bool {
        if !self.can_transition_to_pouring() {
            return false;
        }
        self.phase = FloorConstructionPhase::Pouring;
        true
    }
}

/// Wall construction site - parent entity managing a line of wall tiles
#[derive(Component, Clone, Debug, Reflect)]
#[reflect(Component)]
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

    /// Fast site-level eligibility check for Framing -> Coating.
    ///
    /// Indexed tile ownership, tile state, and provisional wall presence are
    /// checked separately by the adapter that owns the index.
    pub fn can_transition_to_coating(&self) -> bool {
        self.phase == WallConstructionPhase::Framing
            && self.tiles_total > 0
            && self.tiles_framed >= self.tiles_total
    }

    pub fn transition_to_coating(&mut self) -> bool {
        if !self.can_transition_to_coating() {
            return false;
        }
        self.phase = WallConstructionPhase::Coating;
        true
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
