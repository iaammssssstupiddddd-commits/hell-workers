//! テレイン系ビジュアルアセットハンドルと障害物クリーンアップシステム。

use crate::map::{WorldMap, WorldMapWrite};
use crate::terrain::TerrainType;
use bevy::prelude::*;
use hw_jobs::ObstaclePosition;

/// 障害物除去によってテレインが変化したことを通知するメッセージ。
/// `bevy_app` 側の `terrain_material_sync_system` が受信してマテリアルを差し替える。
#[derive(Message, Clone)]
pub struct TerrainChangedEvent {
    pub idx: usize,
}

/// bevy_app から注入されるテレイン系ビジュアルアセットハンドル。
/// 3D 化後はマテリアル差し替えは `TerrainChangedEvent` 経由で bevy_app 側が担う。
/// 将来的に不要になれば除去可。
#[derive(Resource)]
pub struct TerrainVisualHandles {
    pub dirt: Handle<Image>,
}

/// 障害物が削除された時に WorldMap を更新し、テレインを Dirt に戻す。
/// 視覚的なマテリアル差し替えは `TerrainChangedEvent` を発行して bevy_app 側に委ねる。
pub fn obstacle_cleanup_system(
    mut world_map: WorldMapWrite,
    mut removed: RemovedComponents<ObstaclePosition>,
    q_obstacles: Query<&ObstaclePosition>,
    mut ev_terrain_changed: MessageWriter<TerrainChangedEvent>,
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
            ev_terrain_changed.write(TerrainChangedEvent { idx });
        }

        info!(
            "OBSTACLE: Rock at ({}, {}) removed from map (terrain -> Dirt)",
            x, y
        );
    }
}
