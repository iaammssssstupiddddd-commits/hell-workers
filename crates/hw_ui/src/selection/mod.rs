use bevy::prelude::*;

mod intent;
mod placement;

pub use intent::SelectionIntent;
pub use placement::{
    BuildingPlacementContext, PlacementGeometry, PlacementRejectReason, PlacementValidation,
    TANK_NEARBY_BUCKET_STORAGE_TILES, WorldReadApi, bucket_storage_geometry, building_geometry,
    building_occupied_grids, building_size, building_spawn_pos, can_place_moved_building,
    grid_is_nearby, move_anchor_grid, move_occupied_grids, move_spawn_pos, validate_area_size,
    validate_bucket_storage_placement, validate_building_placement, validate_floor_tile,
    validate_moved_bucket_storage_placement, validate_wall_area, validate_wall_tile,
};

#[derive(Resource, Default)]
pub struct SelectedEntity(pub Option<Entity>);

#[derive(Resource, Default)]
pub struct HoveredEntity(pub Option<Entity>);

#[derive(Component)]
pub struct SelectionIndicator;
