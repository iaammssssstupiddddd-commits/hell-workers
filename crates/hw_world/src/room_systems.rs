//! Room detection と validation の ECS システム関数。
//!
//! 純粋なアルゴリズムは [`crate::room_detection`] に定義されている。
//! 本モジュールはそれらを ECS クエリと接続する adapter 層。

use std::collections::{HashMap, HashSet};

use bevy::ecs::lifecycle::{Add, Remove};
use bevy::prelude::*;
use hw_core::constants::{ROOM_BORDER_COLOR, ROOM_BORDER_THICKNESS, TILE_SIZE, Z_ROOM_OVERLAY};
use hw_jobs::{Building, Door};

use crate::map::WorldMap;
use crate::room_detection::{
    DetectedRoom, Room, RoomBoundaryLookup, RoomDetectionBuildingTile, RoomDetectionState,
    RoomOverlayTile, RoomTileLookup, RoomValidationState, build_detection_input, detect_rooms,
    room_is_valid_against_input,
};

type ChangedBuildingQuery<'w, 's> = Query<
    'w,
    's,
    &'static Transform,
    (With<Building>, Or<(Changed<Building>, Changed<Transform>)>),
>;
type ChangedDoorQuery<'w, 's> =
    Query<'w, 's, &'static Transform, (With<Door>, Or<(Changed<Door>, Changed<Transform>)>)>;
type ChangedRoomsQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static Room, Option<&'static Children>),
    Or<(Added<Room>, Changed<Room>)>,
>;
type RoomDetectionBuildingQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static Building, &'static Transform)>;
type ExistingRoomQuery<'w, 's> = Query<'w, 's, Entity, With<Room>>;

/// 建物タイルを収集し Room ECS エンティティを再構築するシステム
pub fn detect_rooms_system(
    mut commands: Commands,
    time: Res<Time>,
    mut detection_state: ResMut<RoomDetectionState>,
    mut room_tile_lookup: ResMut<RoomTileLookup>,
    mut room_boundary_lookup: ResMut<RoomBoundaryLookup>,
    q_buildings: RoomDetectionBuildingQuery,
    q_rooms: ExistingRoomQuery,
) {
    detection_state.cooldown.tick(time.delta());

    if detection_state.dirty_tiles.is_empty() || !detection_state.cooldown.just_finished() {
        return;
    }

    rebuild_rooms(
        &mut commands,
        &mut detection_state,
        &mut room_tile_lookup,
        &mut room_boundary_lookup,
        &q_buildings,
        &q_rooms,
    );
}

/// Profiling fixtures need the production Room adapter before virtual time is
/// unpaused. This bypasses only the cooldown; collection, detection, lookup
/// construction, and ECS Room spawning are shared with normal runtime.
#[cfg(feature = "profiling")]
pub fn detect_rooms_immediately_system(
    mut commands: Commands,
    mut detection_state: ResMut<RoomDetectionState>,
    mut room_tile_lookup: ResMut<RoomTileLookup>,
    mut room_boundary_lookup: ResMut<RoomBoundaryLookup>,
    q_buildings: RoomDetectionBuildingQuery,
    q_rooms: ExistingRoomQuery,
) {
    if detection_state.dirty_tiles.is_empty() {
        return;
    }
    rebuild_rooms(
        &mut commands,
        &mut detection_state,
        &mut room_tile_lookup,
        &mut room_boundary_lookup,
        &q_buildings,
        &q_rooms,
    );
}

fn rebuild_rooms(
    commands: &mut Commands,
    detection_state: &mut RoomDetectionState,
    room_tile_lookup: &mut RoomTileLookup,
    room_boundary_lookup: &mut RoomBoundaryLookup,
    q_buildings: &RoomDetectionBuildingQuery,
    q_rooms: &ExistingRoomQuery,
) {
    let tiles = collect_building_tiles(q_buildings);
    let input = build_detection_input(&tiles);
    let mut detected_rooms = detect_rooms(&input);
    detected_rooms.sort_by(|left, right| {
        (
            left.bounds.min_y,
            left.bounds.min_x,
            left.bounds.max_y,
            left.bounds.max_x,
            &left.tiles,
        )
            .cmp(&(
                right.bounds.min_y,
                right.bounds.min_x,
                right.bounds.max_y,
                right.bounds.max_x,
                &right.tiles,
            ))
    });

    for room_entity in q_rooms.iter() {
        commands.entity(room_entity).try_despawn();
    }

    let mut tile_to_room = HashMap::new();
    let mut boundary_to_rooms = HashMap::new();
    for (index, detected) in detected_rooms.into_iter().enumerate() {
        let DetectedRoom {
            tiles,
            wall_tiles,
            door_tiles,
            bounds,
        } = detected;
        let tile_count = tiles.len();
        let room_tiles_for_lookup = tiles.clone();

        let room_entity = commands.spawn_empty().id();
        insert_room_boundaries(
            &mut boundary_to_rooms,
            room_entity,
            &wall_tiles,
            &door_tiles,
        );

        commands.entity(room_entity).insert((
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
        ));

        for tile in room_tiles_for_lookup {
            tile_to_room.insert(tile, room_entity);
        }
    }

    room_tile_lookup.tile_to_room = tile_to_room;
    room_boundary_lookup.boundary_to_rooms = boundary_to_rooms;
    detection_state.dirty_tiles.clear();
}

use bevy::ecs::system::SystemParam;

#[derive(SystemParam)]
pub struct ValidateRoomsParams<'w, 's> {
    commands: Commands<'w, 's>,
    time: Res<'w, Time>,
    validation_state: ResMut<'w, RoomValidationState>,
    detection_state: ResMut<'w, RoomDetectionState>,
    room_tile_lookup: ResMut<'w, RoomTileLookup>,
    room_boundary_lookup: ResMut<'w, RoomBoundaryLookup>,
    q_rooms: Query<'w, 's, (Entity, &'static Room)>,
    q_buildings: Query<'w, 's, (Entity, &'static Building, &'static Transform)>,
}

/// 既存 Room の整合性を定期検証し、無効なものを再検出キューへ送るシステム
pub fn validate_rooms_system(mut p: ValidateRoomsParams) {
    p.validation_state.timer.tick(p.time.delta());
    if !p.validation_state.timer.just_finished() {
        return;
    }

    let tiles: Vec<RoomDetectionBuildingTile> = p
        .q_buildings
        .iter()
        .map(|(_entity, building, transform)| {
            let grid = WorldMap::world_to_grid(transform.translation.truncate());
            RoomDetectionBuildingTile {
                grid,
                role: building.kind.room_detection_role(building.is_provisional),
            }
        })
        .collect();

    let input = build_detection_input(&tiles);
    let mut tile_to_room = HashMap::new();
    let mut boundary_to_rooms = HashMap::new();

    for (room_entity, room) in p.q_rooms.iter() {
        if room_is_valid_against_input(&room.tiles, &input) {
            for &tile in &room.tiles {
                tile_to_room.insert(tile, room_entity);
            }
            insert_room_boundaries(
                &mut boundary_to_rooms,
                room_entity,
                &room.wall_tiles,
                &room.door_tiles,
            );
            continue;
        }

        p.detection_state
            .mark_dirty_many(room.tiles.iter().copied());
        p.detection_state
            .mark_dirty_many(room.wall_tiles.iter().copied());
        p.detection_state
            .mark_dirty_many(room.door_tiles.iter().copied());
        p.commands.entity(room_entity).try_despawn();
    }

    p.room_tile_lookup.tile_to_room = tile_to_room;
    p.room_boundary_lookup.boundary_to_rooms = boundary_to_rooms;
}

fn collect_building_tiles(
    q_buildings: &Query<(Entity, &Building, &Transform)>,
) -> Vec<RoomDetectionBuildingTile> {
    q_buildings
        .iter()
        .map(|(_entity, building, transform)| {
            let grid = WorldMap::world_to_grid(transform.translation.truncate());
            RoomDetectionBuildingTile {
                grid,
                role: building.kind.room_detection_role(building.is_provisional),
            }
        })
        .collect()
}

fn insert_room_boundaries(
    lookup: &mut HashMap<(i32, i32), Vec<Entity>>,
    room_entity: Entity,
    wall_tiles: &[(i32, i32)],
    door_tiles: &[(i32, i32)],
) {
    for grid in wall_tiles.iter().chain(door_tiles) {
        let rooms = lookup.entry(*grid).or_default();
        rooms.push(room_entity);
        rooms.sort_unstable_by_key(|entity| entity.to_bits());
        rooms.dedup();
    }
}

// ---------------------------------------------------------------------------
// dirty_mark: Building / Door の変化を RoomDetectionState に伝えるシステム群
// ---------------------------------------------------------------------------

/// Building / Door の Changed イベントからダーティタイルをマークする。
/// Add/Remove は Observer (on_building_added 等) が担うため、ここでは Changed のみ処理する。
pub fn mark_room_dirty_from_building_changes_system(
    mut detection_state: ResMut<RoomDetectionState>,
    q_changed_buildings: ChangedBuildingQuery,
    q_changed_doors: ChangedDoorQuery,
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
    q_rooms: ChangedRoomsQuery,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_boundary_lookup_keeps_both_rooms_once() {
        let mut world = World::new();
        let first = world.spawn_empty().id();
        let second = world.spawn_empty().id();
        let mut lookup = HashMap::new();

        insert_room_boundaries(&mut lookup, second, &[(5, 5)], &[]);
        insert_room_boundaries(&mut lookup, first, &[(5, 5)], &[]);
        insert_room_boundaries(&mut lookup, first, &[(5, 5)], &[]);

        let mut expected = vec![first, second];
        expected.sort_unstable_by_key(|entity| entity.to_bits());
        assert_eq!(lookup.get(&(5, 5)), Some(&expected));
    }
}
