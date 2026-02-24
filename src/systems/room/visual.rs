use super::components::{Room, RoomOverlayTile};
use crate::constants::{ROOM_OVERLAY_COLOR, TILE_SIZE, Z_ROOM_OVERLAY};
use crate::world::map::WorldMap;
use bevy::prelude::*;

/// Syncs per-tile room overlays for room entities.
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

        commands.entity(room_entity).with_children(|parent| {
            for &(gx, gy) in &room.tiles {
                let world_pos = WorldMap::grid_to_world(gx, gy);
                parent.spawn((
                    RoomOverlayTile { grid_pos: (gx, gy) },
                    Sprite {
                        color: ROOM_OVERLAY_COLOR,
                        custom_size: Some(Vec2::splat(TILE_SIZE)),
                        ..default()
                    },
                    Transform::from_translation(world_pos.extend(Z_ROOM_OVERLAY)),
                    Visibility::Visible,
                    Name::new("RoomOverlayTile"),
                ));
            }
        });
    }
}
