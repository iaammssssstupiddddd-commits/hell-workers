use super::components::{Room, RoomOverlayTile};
use crate::constants::{ROOM_BORDER_COLOR, ROOM_BORDER_THICKNESS, TILE_SIZE, Z_ROOM_OVERLAY};
use crate::world::map::WorldMap;
use bevy::prelude::*;
use std::collections::HashSet;

const CARDINALS: [(i32, i32); 4] = [(0, 1), (1, 0), (0, -1), (-1, 0)];

/// ルームの床-壁境界にボーダーラインを生成する。
/// 各フロアタイルの隣接壁タイルに対して、壁の室内側エッジに細い線スプライトを配置する。
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

                for (dx, dy) in CARDINALS {
                    let neighbor = (fx + dx, fy + dy);
                    if !wall_set.contains(&neighbor) {
                        continue;
                    }

                    // 壁タイルの室内側エッジ上に線を配置
                    let half_tile = TILE_SIZE / 2.0;
                    let offset = half_tile + ROOM_BORDER_THICKNESS / 2.0;
                    let line_center = Vec2::new(
                        floor_pos.x + dx as f32 * offset,
                        floor_pos.y + dy as f32 * offset,
                    );

                    // 壁の向きに応じてサイズを切り替え（東西壁は縦長、南北壁は横長）
                    let line_size = if dx != 0 {
                        Vec2::new(ROOM_BORDER_THICKNESS, TILE_SIZE)
                    } else {
                        Vec2::new(TILE_SIZE, ROOM_BORDER_THICKNESS)
                    };

                    parent.spawn((
                        RoomOverlayTile { grid_pos: neighbor },
                        Sprite {
                            color: ROOM_BORDER_COLOR,
                            custom_size: Some(line_size),
                            ..default()
                        },
                        Transform::from_translation(line_center.extend(Z_ROOM_OVERLAY)),
                        Visibility::Visible,
                        Name::new("RoomBorderLine"),
                    ));
                }
            }
        });
    }
}
