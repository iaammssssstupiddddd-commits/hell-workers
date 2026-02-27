use super::resources::RoomDetectionState;
use crate::systems::jobs::{Building, Door};
use crate::world::map::WorldMap;
use bevy::prelude::*;
use bevy::ecs::lifecycle::{Add, Remove};

/// Marks dirty tiles from Building / Door changes.
pub fn mark_room_dirty_from_building_changes_system(
    mut detection_state: ResMut<RoomDetectionState>,
    q_changed_buildings: Query<
        &Transform,
        (
            With<Building>,
            Or<(Added<Building>, Changed<Building>, Changed<Transform>)>,
        ),
    >,
    q_changed_doors: Query<
        &Transform,
        (
            With<Door>,
            Or<(Added<Door>, Changed<Door>, Changed<Transform>)>,
        ),
    >,
) {
    for transform in q_changed_buildings.iter() {
        let grid = WorldMap::world_to_grid(transform.translation.truncate());
        detection_state.mark_dirty(grid);
    }

    for transform in q_changed_doors.iter() {
        let grid = WorldMap::world_to_grid(transform.translation.truncate());
        detection_state.mark_dirty(grid);
    }
}

pub fn on_building_added(
    on: On<Add, Building>,
    q_transform: Query<&Transform>,
    mut detection_state: ResMut<RoomDetectionState>,
) {
    if let Ok(transform) = q_transform.get(on.entity) {
        let grid = WorldMap::world_to_grid(transform.translation.truncate());
        detection_state.mark_dirty(grid);
    }
}

pub fn on_building_removed(
    on: On<Remove, Building>,
    q_transform: Query<&Transform>,
    mut detection_state: ResMut<RoomDetectionState>,
) {
    if let Ok(transform) = q_transform.get(on.entity) {
        let grid = WorldMap::world_to_grid(transform.translation.truncate());
        detection_state.mark_dirty(grid);
    }
}

pub fn on_door_added(
    on: On<Add, Door>,
    q_transform: Query<&Transform>,
    mut detection_state: ResMut<RoomDetectionState>,
) {
    if let Ok(transform) = q_transform.get(on.entity) {
        let grid = WorldMap::world_to_grid(transform.translation.truncate());
        detection_state.mark_dirty(grid);
    }
}

pub fn on_door_removed(
    on: On<Remove, Door>,
    q_transform: Query<&Transform>,
    mut detection_state: ResMut<RoomDetectionState>,
) {
    if let Ok(transform) = q_transform.get(on.entity) {
        let grid = WorldMap::world_to_grid(transform.translation.truncate());
        detection_state.mark_dirty(grid);
    }
}
