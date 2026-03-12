use super::components::Room;
use super::resources::{RoomDetectionState, RoomTileLookup, RoomValidationState};
use crate::systems::jobs::Building;
use crate::world::map::WorldMapRead;
use bevy::prelude::*;
use hw_world::room_detection::{RoomDetectionBuildingTile, build_detection_input, room_is_valid_against_input};
use std::collections::HashMap;

/// Periodically validates existing room entities and repairs stale state.
pub fn validate_rooms_system(
    mut commands: Commands,
    time: Res<Time>,
    mut validation_state: ResMut<RoomValidationState>,
    mut detection_state: ResMut<RoomDetectionState>,
    mut room_tile_lookup: ResMut<RoomTileLookup>,
    q_rooms: Query<(Entity, &Room)>,
    q_buildings: Query<(Entity, &Building, &Transform)>,
    world_map: WorldMapRead,
) {
    validation_state.timer.tick(time.delta());
    if !validation_state.timer.just_finished() {
        return;
    }

    let tiles: Vec<RoomDetectionBuildingTile> = q_buildings
        .iter()
        .map(|(_entity, building, transform)| {
            let grid = crate::world::map::WorldMap::world_to_grid(transform.translation.truncate());
            RoomDetectionBuildingTile {
                grid,
                kind: building.kind,
                is_provisional: building.is_provisional,
                has_building_on_top: world_map.has_building(grid),
            }
        })
        .collect();

    let input = build_detection_input(&tiles);
    let mut tile_to_room = HashMap::new();

    for (room_entity, room) in q_rooms.iter() {
        if room_is_valid_against_input(&room.tiles, &input) {
            for &tile in &room.tiles {
                tile_to_room.insert(tile, room_entity);
            }
            continue;
        }

        detection_state.mark_dirty_many(room.tiles.iter().copied());
        detection_state.mark_dirty_many(room.wall_tiles.iter().copied());
        detection_state.mark_dirty_many(room.door_tiles.iter().copied());
        commands.entity(room_entity).try_despawn();
    }

    room_tile_lookup.tile_to_room = tile_to_room;
}
