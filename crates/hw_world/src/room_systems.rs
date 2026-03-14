//! Room detection と validation の ECS システム関数。
//!
//! 純粋なアルゴリズムは [`crate::room_detection`] に定義されている。
//! 本モジュールはそれらを ECS クエリと接続する adapter 層。

use std::collections::HashMap;

use bevy::prelude::*;
use hw_jobs::Building;

use crate::map::{WorldMap, WorldMapRead};
use crate::room_detection::{
    DetectedRoom, Room, RoomDetectionBuildingTile, RoomDetectionState, RoomTileLookup,
    RoomValidationState, build_detection_input, detect_rooms, room_is_valid_against_input,
};

/// 建物タイルを収集し Room ECS エンティティを再構築するシステム
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

/// 既存 Room の整合性を定期検証し、無効なものを再検出キューへ送るシステム
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
            let grid = WorldMap::world_to_grid(transform.translation.truncate());
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

fn collect_building_tiles(
    q_buildings: &Query<(Entity, &Building, &Transform)>,
    world_map: &WorldMapRead,
) -> Vec<RoomDetectionBuildingTile> {
    q_buildings
        .iter()
        .map(|(_entity, building, transform)| {
            let grid = WorldMap::world_to_grid(transform.translation.truncate());
            RoomDetectionBuildingTile {
                grid,
                kind: building.kind,
                is_provisional: building.is_provisional,
                has_building_on_top: world_map.has_building(grid),
            }
        })
        .collect()
}
