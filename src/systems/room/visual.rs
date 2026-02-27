use super::components::{Room, RoomOverlayTile};
use crate::constants::{ROOM_BORDER_COLOR, ROOM_BORDER_THICKNESS, TILE_SIZE, Z_ROOM_OVERLAY};
use crate::world::map::WorldMap;
use bevy::prelude::*;
use std::collections::HashSet;

/// 壁タイル内端からの距離（壁タイルの中心に近い位置）
const LINE_OFFSET: f32 = TILE_SIZE * 0.75; // = 24.0px（壁タイル内端+8px）
/// 隣接する壁方向へのコーナー延長量（ギャップを埋める）
const CORNER_EXT: f32 = LINE_OFFSET - TILE_SIZE * 0.5; // = 8.0px

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

                // 北壁ライン（横線）: 東西の隣接壁方向へ延長してコーナーを接続する
                if has_north {
                    let east_ext = if has_east { CORNER_EXT } else { 0.0 };
                    let west_ext = if has_west { CORNER_EXT } else { 0.0 };
                    let width = TILE_SIZE + east_ext + west_ext;
                    let center = Vec2::new(
                        floor_pos.x + (east_ext - west_ext) / 2.0,
                        floor_pos.y + LINE_OFFSET,
                    );
                    parent.spawn((
                        RoomOverlayTile { grid_pos: (fx, fy + 1) },
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

                // 東壁ライン（縦線）: 南北の隣接壁方向へ延長してコーナーを接続する
                if has_east {
                    let north_ext = if has_north { CORNER_EXT } else { 0.0 };
                    let south_ext = if has_south { CORNER_EXT } else { 0.0 };
                    let height = TILE_SIZE + north_ext + south_ext;
                    let center = Vec2::new(
                        floor_pos.x + LINE_OFFSET,
                        floor_pos.y + (north_ext - south_ext) / 2.0,
                    );
                    parent.spawn((
                        RoomOverlayTile { grid_pos: (fx + 1, fy) },
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

                // 南壁ライン（横線）
                if has_south {
                    let east_ext = if has_east { CORNER_EXT } else { 0.0 };
                    let west_ext = if has_west { CORNER_EXT } else { 0.0 };
                    let width = TILE_SIZE + east_ext + west_ext;
                    let center = Vec2::new(
                        floor_pos.x + (east_ext - west_ext) / 2.0,
                        floor_pos.y - LINE_OFFSET,
                    );
                    parent.spawn((
                        RoomOverlayTile { grid_pos: (fx, fy - 1) },
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

                // 西壁ライン（縦線）
                if has_west {
                    let north_ext = if has_north { CORNER_EXT } else { 0.0 };
                    let south_ext = if has_south { CORNER_EXT } else { 0.0 };
                    let height = TILE_SIZE + north_ext + south_ext;
                    let center = Vec2::new(
                        floor_pos.x - LINE_OFFSET,
                        floor_pos.y + (north_ext - south_ext) / 2.0,
                    );
                    parent.spawn((
                        RoomOverlayTile { grid_pos: (fx - 1, fy) },
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
