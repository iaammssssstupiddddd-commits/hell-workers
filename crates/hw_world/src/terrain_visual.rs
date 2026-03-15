//! テレイン系ビジュアルアセットハンドルと障害物クリーンアップシステム。

use crate::map::{WorldMap, WorldMapWrite};
use crate::terrain::TerrainType;
use bevy::prelude::*;
use hw_jobs::ObstaclePosition;

/// bevy_app から注入されるテレイン系ビジュアルアセットハンドル。
#[derive(Resource)]
pub struct TerrainVisualHandles {
    pub dirt: Handle<Image>,
}

/// 障害物が削除された時に WorldMap を更新し、テレインを Dirt に戻す。
pub fn obstacle_cleanup_system(
    mut world_map: WorldMapWrite,
    handles: Res<TerrainVisualHandles>,
    mut q_sprites: Query<&mut Sprite>,
    mut removed: RemovedComponents<ObstaclePosition>,
    q_obstacles: Query<&ObstaclePosition>,
) {
    let any_removed = removed.read().next().is_some();
    let current_obstacles_count = q_obstacles.iter().count();
    let map_obstacles_count = world_map.obstacle_count();

    if !any_removed && current_obstacles_count == map_obstacles_count {
        return;
    }

    debug!(
        "OBSTACLE: Synchronizing obstacles... Map count: {}, Entity count: {}",
        map_obstacles_count, current_obstacles_count
    );

    let current_obstacles: std::collections::HashSet<(i32, i32)> =
        q_obstacles.iter().map(|pos| (pos.0, pos.1)).collect();

    if current_obstacles.is_empty() && map_obstacles_count > 0 {
        return;
    }

    let mut to_remove = Vec::new();
    for idx in world_map.obstacle_indices() {
        let pos = WorldMap::idx_to_pos(idx);
        if !current_obstacles.contains(&pos) && !world_map.has_building(pos) {
            to_remove.push(pos);
        }
    }

    for (x, y) in to_remove {
        world_map.remove_grid_obstacle((x, y));
        if let Some(idx) = world_map.pos_to_idx(x, y) {
            world_map.set_terrain_at_idx(idx, TerrainType::Dirt);

            if let Some(tile_entity) = world_map.tile_entity_at_idx(idx) {
                if let Ok(mut sprite) = q_sprites.get_mut(tile_entity) {
                    sprite.image = handles.dirt.clone();
                }
            }
        }

        info!(
            "OBSTACLE: Rock at ({}, {}) removed from map (terrain -> Dirt)",
            x, y
        );
    }
}
