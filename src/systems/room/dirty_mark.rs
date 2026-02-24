use super::resources::RoomDetectionState;
use crate::systems::jobs::{Building, Door};
use crate::world::map::WorldMap;
use bevy::prelude::*;

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

/// Marks dirty tiles from WorldMap building occupancy diffs.
pub fn mark_room_dirty_from_world_map_diff_system(
    world_map: Res<WorldMap>,
    mut detection_state: ResMut<RoomDetectionState>,
) {
    if !world_map.is_changed() && !detection_state.previous_world_buildings.is_empty() {
        return;
    }

    let current = &world_map.buildings;

    if detection_state.previous_world_buildings.is_empty() {
        detection_state.mark_dirty_many(current.keys().copied());
        detection_state.previous_world_buildings = current.clone();
        return;
    }

    let mut dirty_positions = Vec::new();

    for (&grid, &previous_entity) in detection_state.previous_world_buildings.iter() {
        match current.get(&grid).copied() {
            Some(entity) if entity == previous_entity => {}
            _ => dirty_positions.push(grid),
        }
    }

    for (&grid, &entity) in current {
        if detection_state.previous_world_buildings.get(&grid).copied() != Some(entity) {
            dirty_positions.push(grid);
        }
    }

    detection_state.mark_dirty_many(dirty_positions);
    detection_state.previous_world_buildings = current.clone();
}
