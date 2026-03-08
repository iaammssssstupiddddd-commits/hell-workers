//! 木の再生システム
//!
//! 各森林ゾーン内で伐採された木が、ゲーム内1日ごとに1本ずつ再生する。
//! 初期配置数を超えて再生することはない。

use crate::assets::GameAssets;
use crate::systems::jobs::Tree;
use crate::systems::time::GameTime;
use crate::world::map::WorldMap;
use bevy::prelude::*;
use hw_core::constants::*;
use hw_world::{ForestZone, default_forest_zones, find_regrowth_position};

/// 再生管理リソース
#[derive(Resource)]
pub struct RegrowthManager {
    /// 森林ゾーンのリスト
    pub zones: Vec<ForestZone>,
    /// 最後に再生を実行した日
    pub last_regrowth_day: u32,
}

impl Default for RegrowthManager {
    fn default() -> Self {
        Self {
            zones: default_forest_zones(),
            last_regrowth_day: 0,
        }
    }
}

/// 木の再生システム
/// ゲーム内1日ごとに、各ゾーンで木が初期数未満なら1本再生
pub fn tree_regrowth_system(
    mut commands: Commands,
    game_time: Res<GameTime>,
    mut regrowth: ResMut<RegrowthManager>,
    game_assets: Res<GameAssets>,
    world_map: Res<WorldMap>,
    q_trees: Query<&Transform, With<Tree>>,
) {
    // 全体上限チェック
    let total_tree_count = q_trees.iter().count() as u32;
    if total_tree_count >= DREAM_TREE_GLOBAL_CAP {
        return;
    }

    // 日が変わったかチェック
    if game_time.day <= regrowth.last_regrowth_day {
        return;
    }

    regrowth.last_regrowth_day = game_time.day;

    // 各ゾーンで再生処理
    for zone in &regrowth.zones {
        // 現在のゾーン内の木の数をカウント
        let mut current_count = 0u32;
        let mut occupied_positions = std::collections::HashSet::new();

        for tree_transform in q_trees.iter() {
            let (gx, gy) = WorldMap::world_to_grid(tree_transform.translation.truncate());
            if zone.contains(gx, gy) {
                current_count += 1;
                occupied_positions.insert((gx, gy));
            }
        }

        // 初期数未満なら1本再生
        if current_count < zone.initial_count {
            let Some((px, py)) =
                find_regrowth_position(zone, &occupied_positions, |x, y| world_map.is_walkable(x, y))
            else {
                continue;
            };

            let pos = WorldMap::grid_to_world(px, py);
            let variant_index = rand::random::<usize>() % game_assets.trees.len();
            commands.spawn((
                Tree,
                crate::systems::jobs::TreeVariant(variant_index),
                Sprite {
                    image: game_assets.trees[variant_index].clone(),
                    custom_size: Some(Vec2::splat(TILE_SIZE * 1.5)),
                    ..default()
                },
                Transform::from_xyz(pos.x, pos.y, Z_ITEM_OBSTACLE),
            ));
            info!(
                "REGROWTH: Tree regrown at ({}, {}) in zone ({:?}-{:?}), count: {}/{}",
                px,
                py,
                zone.min,
                zone.max,
                current_count + 1,
                zone.initial_count
            );
        }
    }
}
