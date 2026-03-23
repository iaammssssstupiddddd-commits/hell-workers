mod geometry;
#[cfg(test)]
mod tests;
mod validation;

pub use self::geometry::{
    bucket_storage_geometry, building_geometry, building_occupied_grids, building_size,
    building_spawn_pos, grid_is_nearby, move_anchor_grid, move_occupied_grids, move_spawn_pos,
};
pub use self::validation::{
    can_place_moved_building, validate_area_size, validate_bucket_storage_placement,
    validate_building_placement, validate_floor_tile, validate_moved_bucket_storage_placement,
    validate_wall_area, validate_wall_tile,
};

use bevy::prelude::*;

pub const TANK_NEARBY_BUCKET_STORAGE_TILES: i32 = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone)]
pub struct PlacementValidation {
    pub can_place: bool,
    pub reject_reason: Option<PlacementRejectReason>,
}

impl PlacementValidation {
    pub fn ok() -> Self {
        Self {
            can_place: true,
            reject_reason: None,
        }
    }

    pub fn rejected(reason: PlacementRejectReason) -> Self {
        Self {
            can_place: false,
            reject_reason: Some(reason),
        }
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
