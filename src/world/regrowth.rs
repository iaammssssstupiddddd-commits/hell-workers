//! 木の再生システム
//!
//! 各森林ゾーン内で伐採された木が、ゲーム内1日ごとに1本ずつ再生する。
//! 初期配置数を超えて再生することはない。

use bevy::prelude::*;
use crate::assets::GameAssets;
use crate::constants::*;
use crate::systems::jobs::Tree;
use crate::systems::time::GameTime;
use crate::world::map::WorldMap;

/// 森林ゾーン定義
#[derive(Clone, Debug)]
pub struct ForestZone {
    /// ゾーンの左下座標
    pub min: (i32, i32),
    /// ゾーンの右上座標
    pub max: (i32, i32),
    /// このゾーン内の初期木の数
    pub initial_count: u32,
    /// このゾーン内の木の座標リスト
    pub tree_positions: Vec<(i32, i32)>,
}

impl ForestZone {
    /// 座標がこのゾーン内にあるかチェック
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.min.0 && x <= self.max.0 && y >= self.min.1 && y <= self.max.1
    }
}

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
        // map.rsのTREE_POSITIONSから各ゾーンを定義
        let zones = vec![
            // 小森林A
            ForestZone {
                min: (15, 44),
                max: (32, 60),
                initial_count: 16,
                tree_positions: vec![
                    (18, 48), (22, 53), (26, 47), (20, 56), (25, 50), (29, 55), (17, 52), (24, 58),
                    (21, 45), (27, 51), (19, 54), (23, 46), (28, 54), (16, 49), (30, 48), (22, 57),
                ],
            },
            // 小森林B
            ForestZone {
                min: (14, 20),
                max: (30, 35),
                initial_count: 16,
                tree_positions: vec![
                    (17, 24), (21, 29), (25, 23), (19, 32), (24, 26), (28, 31), (16, 28), (23, 34),
                    (20, 21), (26, 27), (18, 30), (22, 22), (27, 30), (15, 25), (29, 24), (21, 33),
                ],
            },
            // 大森林（川の向こう）- 再生対象
            ForestZone {
                min: (9, 75),
                max: (43, 96),
                initial_count: 60,
                tree_positions: vec![
                    (12, 78), (18, 83), (24, 77), (16, 88), (22, 82), (28, 87), (14, 81), (20, 94),
                    (10, 85), (26, 80), (32, 85), (15, 90), (21, 76), (27, 91), (13, 86), (19, 79),
                    (25, 93), (31, 78), (11, 82), (17, 95), (23, 84), (29, 89), (35, 80), (38, 86),
                    (40, 82), (36, 92), (33, 88), (30, 76), (37, 79), (34, 94), (39, 88), (41, 84),
                    (14, 93), (20, 87), (26, 77), (32, 91), (18, 81), (24, 95), (30, 83), (36, 77),
                    (12, 89), (22, 86), (28, 78), (34, 84), (16, 76), (38, 90), (40, 78), (42, 88),
                    (13, 84), (19, 92), (25, 79), (31, 93), (37, 83), (15, 77), (21, 89), (27, 82),
                    (33, 77), (39, 93), (35, 87), (41, 81),
                ],
            },
        ];

        Self {
            zones,
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
            // 空いている位置を探す
            for &(px, py) in &zone.tree_positions {
                if !occupied_positions.contains(&(px, py)) && world_map.is_walkable(px, py) {
                    let pos = WorldMap::grid_to_world(px, py);
                    commands.spawn((
                        Tree,
                        Sprite {
                            image: game_assets.tree.clone(),
                            custom_size: Some(Vec2::splat(TILE_SIZE * 1.5)),
                            ..default()
                        },
                        Transform::from_xyz(pos.x, pos.y, Z_ITEM_OBSTACLE),
                    ));
                    info!(
                        "REGROWTH: Tree regrown at ({}, {}) in zone ({:?}-{:?}), count: {}/{}",
                        px, py, zone.min, zone.max, current_count + 1, zone.initial_count
                    );
                    break; // 1日1本のみ再生
                }
            }
        }
    }
}
