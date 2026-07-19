mod geometry;
#[cfg(test)]
mod tests;
mod validation;

pub use self::geometry::{
    bucket_storage_geometry, building_geometry, building_occupied_grids, building_size,
    building_spawn_pos, grid_is_nearby, move_anchor_grid, move_occupied_grids, move_spawn_pos,
};
pub use self::validation::{
    validate_area_size, validate_bucket_storage_placement, validate_building_placement,
    validate_floor_tile, validate_moved_bucket_storage_placement,
    validate_moved_building_placement, validate_wall_area, validate_wall_tile,
};

use bevy::prelude::*;
use bevy::time::Real;
use std::time::Duration;

pub const TANK_NEARBY_BUCKET_STORAGE_TILES: i32 = 3;
pub const RECENT_PLACEMENT_FAILURE_LIFETIME: Duration = Duration::from_secs(2);

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PlacementFeedbackSet {
    Produce,
    Present,
    Commit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementRejectReason {
    NotWalkable,
    OccupiedByBuilding,
    OccupiedByStockpile,
    OutOfBounds,
    NotRiverTile,
    NoDoorAdjacentWall,
    NotInSite,
    NotInYard,
    AlreadyHasFloorBlueprint,
    AlreadyHasCompletedFloor,
    NoCompletedFloor,
    AreaTooLarge,
    TooFarFromParent,
    NotStraightLine,
}

impl PlacementRejectReason {
    pub const ALL: [Self; 14] = [
        Self::NotWalkable,
        Self::OccupiedByBuilding,
        Self::OccupiedByStockpile,
        Self::OutOfBounds,
        Self::NotRiverTile,
        Self::NoDoorAdjacentWall,
        Self::NotInSite,
        Self::NotInYard,
        Self::AlreadyHasFloorBlueprint,
        Self::AlreadyHasCompletedFloor,
        Self::NoCompletedFloor,
        Self::AreaTooLarge,
        Self::TooFarFromParent,
        Self::NotStraightLine,
    ];

    pub fn message(&self, gx: i32, gy: i32) -> String {
        match self {
            Self::NotWalkable => format!("Tile ({},{}) is not walkable", gx, gy),
            Self::OccupiedByBuilding => {
                format!("Tile ({},{}) is already occupied by a building", gx, gy)
            }
            Self::OccupiedByStockpile => {
                format!("Tile ({},{}) is already occupied by a stockpile", gx, gy)
            }
            Self::OutOfBounds => format!("Tile ({},{}) is out of bounds", gx, gy),
            Self::NotRiverTile => format!("Tile ({},{}) is not a river tile", gx, gy),
            Self::NoDoorAdjacentWall => {
                format!("Tile ({},{}) has no adjacent wall pair for door", gx, gy)
            }
            Self::NotInSite => {
                format!("Tile ({},{}) is not inside a construction site", gx, gy)
            }
            Self::NotInYard => format!("Tile ({},{}) is not inside a yard", gx, gy),
            Self::AlreadyHasFloorBlueprint => {
                format!("Tile ({},{}) already has a floor blueprint", gx, gy)
            }
            Self::AlreadyHasCompletedFloor => {
                format!("Tile ({},{}) already has a completed floor", gx, gy)
            }
            Self::NoCompletedFloor => {
                format!("Tile ({},{}) has no completed floor", gx, gy)
            }
            Self::AreaTooLarge => {
                format!("Placement area starting at ({},{}) is too large", gx, gy)
            }
            Self::TooFarFromParent => {
                format!("Tile ({},{}) is too far from parent building", gx, gy)
            }
            Self::NotStraightLine => {
                format!(
                    "Wall must be placed as a straight 1xn line (tile {},{} is in a non-linear area)",
                    gx, gy
                )
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacementValidation {
    pub can_place: bool,
    pub reject_reason: Option<PlacementRejectReason>,
    pub reject_grid: Option<(i32, i32)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementFeedbackStatus {
    Rejected,
    Partial,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacementFeedback {
    pub status: PlacementFeedbackStatus,
    pub reason: PlacementRejectReason,
    pub target_grid: (i32, i32),
    pub valid_tile_count: usize,
    pub rejected_tile_count: usize,
}

impl PlacementFeedback {
    pub fn rejected(reason: PlacementRejectReason, target_grid: (i32, i32)) -> Self {
        Self {
            status: PlacementFeedbackStatus::Rejected,
            reason,
            target_grid,
            valid_tile_count: 0,
            rejected_tile_count: 1,
        }
    }

    pub fn header(&self) -> &'static str {
        match self.status {
            PlacementFeedbackStatus::Rejected => "Cannot place",
            PlacementFeedbackStatus::Partial => "Some tiles will be skipped",
        }
    }

    pub fn body(&self) -> String {
        let reason = self.reason.message(self.target_grid.0, self.target_grid.1);
        match self.status {
            PlacementFeedbackStatus::Rejected => reason,
            PlacementFeedbackStatus::Partial => format!(
                "{reason}. {} valid, {} skipped",
                self.valid_tile_count, self.rejected_tile_count
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacementTileRejection {
    pub grid: (i32, i32),
    pub reason: PlacementRejectReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaPlacementPlan {
    pub valid_tiles: Vec<(i32, i32)>,
    pub total_tile_count: usize,
    pub first_reject: Option<PlacementTileRejection>,
}

impl AreaPlacementPlan {
    pub fn rejected_tile_count(&self) -> usize {
        self.total_tile_count.saturating_sub(self.valid_tiles.len())
    }

    pub fn feedback(&self) -> Option<PlacementFeedback> {
        let first_reject = self.first_reject.as_ref()?;
        let status = if self.valid_tiles.is_empty() {
            PlacementFeedbackStatus::Rejected
        } else {
            PlacementFeedbackStatus::Partial
        };
        Some(PlacementFeedback {
            status,
            reason: first_reject.reason,
            target_grid: first_reject.grid,
            valid_tile_count: self.valid_tiles.len(),
            rejected_tile_count: self.rejected_tile_count(),
        })
    }
}

pub fn build_area_placement_plan(
    min_grid: (i32, i32),
    max_grid: (i32, i32),
    structural_reject: Option<PlacementRejectReason>,
    mut validate_tile: impl FnMut((i32, i32)) -> Option<PlacementRejectReason>,
) -> AreaPlacementPlan {
    let min_x = min_grid.0.min(max_grid.0);
    let max_x = min_grid.0.max(max_grid.0);
    let min_y = min_grid.1.min(max_grid.1);
    let max_y = min_grid.1.max(max_grid.1);
    let total_tile_count = ((max_x - min_x + 1) * (max_y - min_y + 1)) as usize;

    if let Some(reason) = structural_reject {
        return AreaPlacementPlan {
            valid_tiles: Vec::new(),
            total_tile_count,
            first_reject: Some(PlacementTileRejection {
                grid: (min_x, min_y),
                reason,
            }),
        };
    }

    let mut valid_tiles = Vec::with_capacity(total_tile_count);
    let mut first_reject = None;
    for gy in min_y..=max_y {
        for gx in min_x..=max_x {
            let grid = (gx, gy);
            if let Some(reason) = validate_tile(grid) {
                first_reject.get_or_insert(PlacementTileRejection { grid, reason });
            } else {
                valid_tiles.push(grid);
            }
        }
    }
    AreaPlacementPlan {
        valid_tiles,
        total_tile_count,
        first_reject,
    }
}

#[derive(Debug, Clone)]
struct RecentPlacementFailure {
    feedback: PlacementFeedback,
    expires_at: Duration,
}

#[derive(Resource, Default, Debug, Clone)]
pub struct PlacementFeedbackState {
    pub live: Option<PlacementFeedback>,
    recent_failure: Option<RecentPlacementFailure>,
    blocked_live_target: Option<(i32, i32)>,
}

impl PlacementFeedbackState {
    pub fn set_live_validation(
        &mut self,
        validation: &PlacementValidation,
        target_grid: (i32, i32),
    ) {
        self.blocked_live_target = None;
        self.apply_live_validation(validation, target_grid);
    }

    /// Updates continuous BuildingPlace feedback while honoring the successful-commit blocker.
    ///
    /// A successful placement immediately makes its own anchor occupied. Suppress only that
    /// same-anchor building/stockpile interference text until the cursor reaches another grid;
    /// validation and ghost color remain owned by the caller and are not changed here.
    pub fn set_live_building_validation(
        &mut self,
        validation: &PlacementValidation,
        target_grid: (i32, i32),
    ) {
        let is_interference = matches!(
            validation.reject_reason,
            Some(
                PlacementRejectReason::OccupiedByBuilding
                    | PlacementRejectReason::OccupiedByStockpile
            )
        );
        if self.blocked_live_target == Some(target_grid) && is_interference {
            self.live = None;
            return;
        }
        self.blocked_live_target = None;
        self.apply_live_validation(validation, target_grid);
    }

    fn apply_live_validation(&mut self, validation: &PlacementValidation, target_grid: (i32, i32)) {
        self.live = validation
            .rejection(target_grid)
            .map(|rejection| PlacementFeedback::rejected(rejection.reason, rejection.grid));
    }

    pub fn set_live_area_plan(&mut self, plan: &AreaPlacementPlan) {
        self.blocked_live_target = None;
        self.live = plan.feedback();
    }

    pub fn show_recent_failure(&mut self, feedback: PlacementFeedback, now: Duration) {
        debug_assert_eq!(feedback.status, PlacementFeedbackStatus::Rejected);
        self.blocked_live_target = None;
        self.recent_failure = Some(RecentPlacementFailure {
            feedback,
            expires_at: now + RECENT_PLACEMENT_FAILURE_LIFETIME,
        });
    }

    pub fn show_recent_rejection(
        &mut self,
        reason: PlacementRejectReason,
        target_grid: (i32, i32),
        now: Duration,
    ) {
        self.show_recent_failure(PlacementFeedback::rejected(reason, target_grid), now);
    }

    /// Suppresses automatic building/stockpile interference from the just-committed placement at
    /// the same cursor grid. Moving to another grid or recording an explicit failure releases it.
    pub fn block_live_feedback_at(&mut self, target_grid: (i32, i32)) {
        self.live = None;
        self.recent_failure = None;
        self.blocked_live_target = Some(target_grid);
    }

    pub fn clear_live_feedback_blocker(&mut self) {
        self.blocked_live_target = None;
    }

    pub fn visible(&self, now: Duration) -> Option<&PlacementFeedback> {
        self.live.as_ref().or_else(|| {
            self.recent_failure
                .as_ref()
                .filter(|recent| recent.expires_at > now)
                .map(|recent| &recent.feedback)
        })
    }

    pub fn clear(&mut self) {
        self.live = None;
        self.recent_failure = None;
        self.blocked_live_target = None;
    }

    pub fn clear_recent_failure(&mut self) {
        self.recent_failure = None;
    }

    fn begin_frame(&mut self, now: Duration) {
        self.live = None;
        if self
            .recent_failure
            .as_ref()
            .is_some_and(|recent| recent.expires_at <= now)
        {
            self.recent_failure = None;
        }
    }
}

pub fn clear_live_placement_feedback_system(
    real_time: Res<Time<Real>>,
    mut feedback: ResMut<PlacementFeedbackState>,
) {
    feedback.begin_frame(real_time.elapsed());
}

impl PlacementValidation {
    pub fn ok() -> Self {
        Self {
            can_place: true,
            reject_reason: None,
            reject_grid: None,
        }
    }

    pub fn rejected(reason: PlacementRejectReason) -> Self {
        Self {
            can_place: false,
            reject_reason: Some(reason),
            reject_grid: None,
        }
    }

    pub fn rejected_at(reason: PlacementRejectReason, grid: (i32, i32)) -> Self {
        Self {
            can_place: false,
            reject_reason: Some(reason),
            reject_grid: Some(grid),
        }
    }

    pub fn rejection(&self, fallback_grid: (i32, i32)) -> Option<PlacementTileRejection> {
        self.reject_reason.map(|reason| PlacementTileRejection {
            grid: self.reject_grid.unwrap_or(fallback_grid),
            reason,
        })
    }
}

#[derive(Debug, Clone)]
pub struct PlacementGeometry {
    pub occupied_grids: Vec<(i32, i32)>,
    pub draw_pos: Vec2,
    pub size: Vec2,
}

pub trait WorldReadApi {
    fn has_building(&self, grid: (i32, i32)) -> bool;
    fn has_stockpile(&self, grid: (i32, i32)) -> bool;
    /// Raw runtime blocker, excluding terrain walkability policy.
    fn has_raw_obstacle(&self, grid: (i32, i32)) -> bool;
    fn is_walkable(&self, gx: i32, gy: i32) -> bool;
    fn is_river_tile(&self, gx: i32, gy: i32) -> bool;
    fn building_entity(&self, grid: (i32, i32)) -> Option<Entity>;
    fn stockpile_entity(&self, grid: (i32, i32)) -> Option<Entity>;
    fn pos_to_idx(&self, gx: i32, gy: i32) -> Option<usize>;
}

pub struct BuildingPlacementContext<'a, World>
where
    World: WorldReadApi,
{
    pub world: &'a World,
    pub in_site: bool,
    pub in_yard: bool,
    pub is_wall_or_door_at: &'a dyn Fn((i32, i32)) -> bool,
    pub is_replaceable_wall_at: &'a dyn Fn((i32, i32)) -> bool,
}
