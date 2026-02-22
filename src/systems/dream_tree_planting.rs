//! Dream植林システム
//!
//! DreamPoolを消費して、プレイヤー指定の矩形範囲に Tree を生成する。
//! ドラッグ確定時に `AreaEditSession.pending_dream_planting` がセットされ、本システムがそれを処理する。

use crate::assets::GameAssets;
use crate::constants::*;
use crate::entities::damned_soul::DreamPool;
use crate::systems::command::AreaEditSession;
use crate::systems::jobs::{Tree, TreeVariant};
use crate::systems::logistics::ResourceItem;
use crate::world::map::WorldMap;
use bevy::prelude::*;
use rand::seq::SliceRandom;
use rand::thread_rng;

/// Dream植林システム
/// `AreaEditSession.pending_dream_planting` を消費して Tree を生成し、DreamPool を消費する。
pub fn dream_tree_planting_system(
    mut commands: Commands,
    mut area_edit_session: ResMut<AreaEditSession>,
    mut world_map: ResMut<WorldMap>,
    mut dream_pool: ResMut<DreamPool>,
    game_assets: Res<GameAssets>,
    q_trees: Query<&Transform, With<Tree>>,
    q_items: Query<&Transform, With<ResourceItem>>,
) {
    let Some((start, end)) = area_edit_session.pending_dream_planting.take() else {
        return;
    };

    process_dream_planting(
        &start,
        &end,
        &mut commands,
        &mut world_map,
        &mut dream_pool,
        &game_assets,
        &q_trees,
        &q_items,
    );
}

fn process_dream_planting(
    start: &Vec2,
    end: &Vec2,
    commands: &mut Commands,
    world_map: &mut ResMut<WorldMap>,
    dream_pool: &mut ResMut<DreamPool>,
    game_assets: &Res<GameAssets>,
    q_trees: &Query<&Transform, With<Tree>>,
    q_items: &Query<&Transform, With<ResourceItem>>,
) {
    // グリッド座標に変換（min/max を正規化）
    let min_world = Vec2::new(start.x.min(end.x), start.y.min(end.y));
    let max_world = Vec2::new(start.x.max(end.x), start.y.max(end.y));
    let (gx_min, gy_min) = WorldMap::world_to_grid(min_world);
    let (gx_max, gy_max) = WorldMap::world_to_grid(max_world);

    // 矩形内の全タイル数（面積）
    let width = (gx_max - gx_min + 1).max(0) as u32;
    let height = (gy_max - gy_min + 1).max(0) as u32;
    let area_tiles = width * height;

    // 最低面積チェック（min_square_side = ceil(sqrt(1/rate)) = 2 → 4タイル以上必要）
    let min_side = (1.0 / DREAM_TREE_SPAWN_RATE_PER_TILE).sqrt().ceil() as u32;
    let min_area = min_side * min_side;
    if area_tiles < min_area {
        info!(
            "DREAM_PLANT: AreaTooSmall ({} tiles, need >= {}). 消費なし。",
            area_tiles, min_area
        );
        return;
    }

    // 予定生成本数
    let planned_spawn = (area_tiles as f32 * DREAM_TREE_SPAWN_RATE_PER_TILE).floor() as u32;

    // 現在の全木本数
    let current_tree_count = q_trees.iter().count() as u32;
    let cap_remaining = DREAM_TREE_GLOBAL_CAP.saturating_sub(current_tree_count);

    if cap_remaining == 0 {
        info!(
            "DREAM_PLANT: GlobalCapReached ({} / {}). 消費なし。",
            current_tree_count, DREAM_TREE_GLOBAL_CAP
        );
        return;
    }

    // Dream残高による上限
    let affordable = (dream_pool.points / DREAM_TREE_COST_PER_TREE).floor() as u32;
    if affordable == 0 {
        info!(
            "DREAM_PLANT: InsufficientDream ({:.1} points). 消費なし。",
            dream_pool.points
        );
        return;
    }

    // アイテムが存在するグリッドのセットを構築
    let mut blocked_by_item = std::collections::HashSet::new();
    for item_transform in q_items.iter() {
        let pos = item_transform.translation.truncate();
        blocked_by_item.insert(WorldMap::world_to_grid(pos));
    }

    // 候補タイル収集
    let mut candidates: Vec<(i32, i32)> = Vec::new();
    for gx in gx_min..=gx_max {
        for gy in gy_min..=gy_max {
            if !world_map.is_walkable(gx, gy) {
                continue;
            }
            if world_map.buildings.contains_key(&(gx, gy)) {
                continue;
            }
            if blocked_by_item.contains(&(gx, gy)) {
                continue;
            }
            candidates.push((gx, gy));
        }
    }

    if candidates.is_empty() {
        info!("DREAM_PLANT: NoCandidateTile in area. 消費なし。");
        return;
    }

    // 最終生成本数の決定
    let final_spawn = planned_spawn
        .min(candidates.len() as u32)
        .min(DREAM_TREE_MAX_PER_CAST)
        .min(cap_remaining)
        .min(affordable);

    if final_spawn == 0 {
        info!("DREAM_PLANT: final_spawn == 0. 消費なし。");
        return;
    }

    // ランダムに final_spawn 件を選択
    let mut rng = thread_rng();
    candidates.shuffle(&mut rng);
    let selected = &candidates[..final_spawn as usize];

    // Tree をスポーン
    for &(gx, gy) in selected {
        let pos = WorldMap::grid_to_world(gx, gy);
        let variant_index = rand::random::<usize>() % game_assets.trees.len();
        commands.spawn((
            Tree,
            TreeVariant(variant_index),
            Sprite {
                image: game_assets.trees[variant_index].clone(),
                custom_size: Some(Vec2::splat(TILE_SIZE * 1.5)),
                ..default()
            },
            Transform::from_xyz(pos.x, pos.y, Z_ITEM_OBSTACLE),
        ));
        world_map.add_obstacle(gx, gy);
    }

    let cost = final_spawn as f32 * DREAM_TREE_COST_PER_TREE;
    dream_pool.points -= cost;

    info!(
        "DREAM_PLANT: {}本生成、{:.1} Dream消費（残:{:.1}）",
        final_spawn, cost, dream_pool.points
    );

    // 消費エフェクト (-Dream) のポップアップ生成
    let popup_pos = start.extend(Z_FLOATING_TEXT) + Vec3::new(0.0, 20.0, 0.0);
    let config = crate::systems::utils::floating_text::FloatingTextConfig {
        lifetime: 1.5,
        velocity: Vec2::new(0.0, 30.0),
        initial_color: Color::srgb(1.0, 0.3, 0.3), // 赤色でマイナスを表現
        fade_out: true,
    };

    crate::systems::utils::floating_text::spawn_floating_text(
        commands,
        format!("-{:.1} Dream", cost),
        popup_pos,
        config.clone(),
        Some(16.0),
        game_assets.font_ui.clone(),
    );
}
