use super::components::Room;
use super::resources::{RoomDetectionState, RoomTileLookup};
use crate::systems::jobs::Building;
use crate::world::map::WorldMapRead;
use bevy::prelude::*;
use hw_world::room_detection::{DetectedRoom, RoomDetectionBuildingTile, build_detection_input, detect_rooms};
use std::collections::HashMap;

pub fn detect_rooms_system(
    mut commands: Commands,
    time: Res<Time>,
    world_map: WorldMapRead,
    mut detection_state: ResMut<RoomDetectionState>,
    mut room_tile_lookup: ResMut<RoomTileLookup>,
    q_buildings: Query<(Entity, &Building, &Transform)>,
    q_rooms: Query<Entity, With<Room>>,
) {
    detection_state.cooldown.tick(time.delta());

    if detection_state.dirty_tiles.is_empty() || !detection_state.cooldown.just_finished() {
        return;
    }

    let tiles = collect_building_tiles(&q_buildings, &world_map);
    let input = build_detection_input(&tiles);
    let detected_rooms = detect_rooms(&input);

    for room_entity in q_rooms.iter() {
        commands.entity(room_entity).try_despawn();
    }

    let mut tile_to_room = HashMap::new();
    for (index, detected) in detected_rooms.into_iter().enumerate() {
        let DetectedRoom {
            tiles,
            wall_tiles,
            door_tiles,
            bounds,
        } = detected;
        let tile_count = tiles.len();
        let room_tiles_for_lookup = tiles.clone();

        let room_entity = commands
            .spawn((
                Room {
                    tiles,
                    wall_tiles,
                    door_tiles,
                    bounds,
                    tile_count,
                },
                bounds,
                Transform::default(),
                Name::new(format!("Room #{}", index + 1)),
            ))
            .id();

        for tile in room_tiles_for_lookup {
            tile_to_room.insert(tile, room_entity);
        }
    }

    room_tile_lookup.tile_to_room = tile_to_room;
    detection_state.dirty_tiles.clear();
}

/// Collects building entity data from ECS into plain tile descriptors for
/// [`build_detection_input`].
fn collect_building_tiles(
    q_buildings: &Query<(Entity, &Building, &Transform)>,
    world_map: &WorldMapRead,
) -> Vec<RoomDetectionBuildingTile> {
    q_buildings
        .iter()
        .map(|(_entity, building, transform)| {
            let grid = crate::world::map::WorldMap::world_to_grid(transform.translation.truncate());
            RoomDetectionBuildingTile {
                grid,
                kind: building.kind,
                is_provisional: building.is_provisional,
                // 完成床タイルは world_map.buildings に登録されない。
                // 別の建物（壁など）が同セルを占有する場合は floor として扱わない。
                has_building_on_top: world_map.has_building(grid),
            }
        })
        .collect()
}
