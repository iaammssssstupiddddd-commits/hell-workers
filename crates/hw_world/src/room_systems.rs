//! Room detection と validation の ECS システム関数。
//!
//! 純粋なアルゴリズムは [`crate::room_detection`] に定義されている。
//! 本モジュールはそれらを ECS クエリと接続する adapter 層。

use std::collections::{HashMap, HashSet};

use bevy::ecs::lifecycle::{Add, Remove};
use bevy::prelude::*;
use hw_core::constants::{ROOM_BORDER_COLOR, ROOM_BORDER_THICKNESS, TILE_SIZE, Z_ROOM_OVERLAY};
use hw_jobs::{Building, Door};

use crate::map::{WorldMap, WorldMapRead};
use crate::room_detection::{
    DetectedRoom, Room, RoomDetectionBuildingTile, RoomDetectionState, RoomOverlayTile,
    RoomTileLookup, RoomValidationState, build_detection_input, detect_rooms,
    room_is_valid_against_input,
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

// ---------------------------------------------------------------------------
// dirty_mark: Building / Door の変化を RoomDetectionState に伝えるシステム群
// ---------------------------------------------------------------------------

/// Building / Door の Changed イベントからダーティタイルをマークする。
/// Add/Remove は Observer (on_building_added 等) が担うため、ここでは Changed のみ処理する。
pub fn mark_room_dirty_from_building_changes_system(
    mut detection_state: ResMut<RoomDetectionState>,
    q_changed_buildings: Query<
        &Transform,
        (With<Building>, Or<(Changed<Building>, Changed<Transform>)>),
    >,
    q_changed_doors: Query<&Transform, (With<Door>, Or<(Changed<Door>, Changed<Transform>)>)>,
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

// ---------------------------------------------------------------------------
// visual: Room のオーバーレイタイルを同期するシステム
// ---------------------------------------------------------------------------

/// 壁タイル内端からの距離（壁タイルの中心に近い位置）
const LINE_OFFSET: f32 = TILE_SIZE * 0.75;
/// 隣接する壁方向へのコーナー延長量（ギャップを埋める）
const CORNER_EXT: f32 = LINE_OFFSET - TILE_SIZE * 0.5;

/// ルームの床-壁境界にボーダーラインを生成する。
/// 各フロアタイルの隣接壁タイルに対して、壁の室内側にライン（スプライト）を配置する。
/// コーナーでは隣接する2辺が接続するようにラインを延長する。
pub fn sync_room_overlay_tiles_system(
    mut commands: Commands,
    q_rooms: Query<(Entity, &Room, Option<&Children>), Or<(Added<Room>, Changed<Room>)>>,
    q_overlay_tiles: Query<(), With<RoomOverlayTile>>,
) {
    for (room_entity, room, children_opt) in q_rooms.iter() {
        if let Some(children) = children_opt {
            for child in children.iter() {
                if q_overlay_tiles.get(child).is_ok() {
                    commands.entity(child).try_despawn();
                }
            }
        }

        let wall_set: HashSet<(i32, i32)> = room.wall_tiles.iter().copied().collect();

        commands.entity(room_entity).with_children(|parent| {
            for &(fx, fy) in &room.tiles {
                let floor_pos = WorldMap::grid_to_world(fx, fy);

                let has_north = wall_set.contains(&(fx, fy + 1));
                let has_east = wall_set.contains(&(fx + 1, fy));
                let has_south = wall_set.contains(&(fx, fy - 1));
                let has_west = wall_set.contains(&(fx - 1, fy));

                if has_north {
                    let east_ext = if has_east { CORNER_EXT } else { 0.0 };
                    let west_ext = if has_west { CORNER_EXT } else { 0.0 };
                    let width = TILE_SIZE + east_ext + west_ext;
                    let center = Vec2::new(
                        floor_pos.x + (east_ext - west_ext) / 2.0,
                        floor_pos.y + LINE_OFFSET,
                    );
                    parent.spawn((
                        RoomOverlayTile {
                            grid_pos: (fx, fy + 1),
                        },
                        Sprite {
                            color: ROOM_BORDER_COLOR,
                            custom_size: Some(Vec2::new(width, ROOM_BORDER_THICKNESS)),
                            ..default()
                        },
                        Transform::from_translation(center.extend(Z_ROOM_OVERLAY)),
                        Visibility::Visible,
                        Name::new("RoomBorderLine"),
                    ));
                }

                if has_east {
                    let north_ext = if has_north { CORNER_EXT } else { 0.0 };
                    let south_ext = if has_south { CORNER_EXT } else { 0.0 };
                    let height = TILE_SIZE + north_ext + south_ext;
                    let center = Vec2::new(
                        floor_pos.x + LINE_OFFSET,
                        floor_pos.y + (north_ext - south_ext) / 2.0,
                    );
                    parent.spawn((
                        RoomOverlayTile {
                            grid_pos: (fx + 1, fy),
                        },
                        Sprite {
                            color: ROOM_BORDER_COLOR,
                            custom_size: Some(Vec2::new(ROOM_BORDER_THICKNESS, height)),
                            ..default()
                        },
                        Transform::from_translation(center.extend(Z_ROOM_OVERLAY)),
                        Visibility::Visible,
                        Name::new("RoomBorderLine"),
                    ));
                }

                if has_south {
                    let east_ext = if has_east { CORNER_EXT } else { 0.0 };
                    let west_ext = if has_west { CORNER_EXT } else { 0.0 };
                    let width = TILE_SIZE + east_ext + west_ext;
                    let center = Vec2::new(
                        floor_pos.x + (east_ext - west_ext) / 2.0,
                        floor_pos.y - LINE_OFFSET,
                    );
                    parent.spawn((
                        RoomOverlayTile {
                            grid_pos: (fx, fy - 1),
                        },
                        Sprite {
                            color: ROOM_BORDER_COLOR,
                            custom_size: Some(Vec2::new(width, ROOM_BORDER_THICKNESS)),
                            ..default()
                        },
                        Transform::from_translation(center.extend(Z_ROOM_OVERLAY)),
                        Visibility::Visible,
                        Name::new("RoomBorderLine"),
                    ));
                }

                if has_west {
                    let north_ext = if has_north { CORNER_EXT } else { 0.0 };
                    let south_ext = if has_south { CORNER_EXT } else { 0.0 };
                    let height = TILE_SIZE + north_ext + south_ext;
                    let center = Vec2::new(
                        floor_pos.x - LINE_OFFSET,
                        floor_pos.y + (north_ext - south_ext) / 2.0,
                    );
                    parent.spawn((
                        RoomOverlayTile {
                            grid_pos: (fx - 1, fy),
                        },
                        Sprite {
                            color: ROOM_BORDER_COLOR,
                            custom_size: Some(Vec2::new(ROOM_BORDER_THICKNESS, height)),
                            ..default()
                        },
                        Transform::from_translation(center.extend(Z_ROOM_OVERLAY)),
                        Visibility::Visible,
                        Name::new("RoomBorderLine"),
                    ));
                }
            }
        });
    }
}
