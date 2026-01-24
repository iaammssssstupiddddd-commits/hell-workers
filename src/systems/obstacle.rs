//! 障害物管理システム
//!
//! Tree/RockがDespawnされた時にWorldMapから障害物を削除する

use bevy::prelude::*;
use crate::assets::GameAssets;
use crate::systems::jobs::ObstaclePosition;
use crate::world::map::WorldMap;

/// 障害物が削除された時にWorldMapを更新
pub fn obstacle_cleanup_system(
    mut world_map: ResMut<WorldMap>,
    game_assets: Res<GameAssets>,
    mut q_sprites: Query<&mut Sprite>,
    mut removed: RemovedComponents<ObstaclePosition>,
    q_obstacles: Query<&ObstaclePosition>,
) {
    // 何かが削除されたか、あるいは内部データと不一致がある場合に処理
    let any_removed = removed.read().next().is_some();
    let current_obstacles_count = q_obstacles.iter().count();
    let map_obstacles_count = world_map.obstacles.len();
    
    if !any_removed && current_obstacles_count == map_obstacles_count {
        return;
    }
    
    // 同期処理開始
    debug!("OBSTACLE: Synchronizing obstacles... Map: {}, Entities: {}", map_obstacles_count, current_obstacles_count);
    
    // 現存する障害物の座標を収集
    let current_obstacles: std::collections::HashSet<(i32, i32)> = q_obstacles
        .iter()
        .map(|pos| (pos.0, pos.1))
        .collect();
    
    // 安全チェック: 何らかの理由で既存のコンポーネントが一時的に取得できない場合の全消去を防止
    if current_obstacles.is_empty() && !world_map.obstacles.is_empty() {
        // 全部の岩が同時に消えることは通常ないため、クエリ失敗の可能性がある
        // warn!("OBSTACLE: Cleanup skipped - no ObstaclePosition found, but world_map has obstacles.");
        return;
    }
    
    // WorldMapから不要な障害物を削除
    let to_remove: Vec<(i32, i32)> = world_map
        .obstacles
        .iter()
        .filter(|pos| !current_obstacles.contains(*pos))
        .cloned()
        .collect();
    
    for (x, y) in to_remove {
        world_map.remove_obstacle(x, y);
        // 岩があった場所をDirtに変更
        world_map.tiles.insert((x, y), crate::world::map::TerrainType::Dirt);
        
        // 視覚的なタイルも更新
        if let Some(&tile_entity) = world_map.tile_entities.get(&(x, y)) {
            if let Ok(mut sprite) = q_sprites.get_mut(tile_entity) {
                sprite.image = game_assets.dirt.clone();
            }
        }
        
        info!("OBSTACLE: Rock at ({}, {}) removed from map (terrain -> Dirt)", x, y);
    }
}
