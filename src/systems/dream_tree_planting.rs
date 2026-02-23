//! Dream植林システム
//!
//! DreamPoolを消費して、プレイヤー指定の矩形範囲に Tree を生成する。
//! ドラッグ確定時に `AreaEditSession.pending_dream_planting` がセットされ、本システムがそれを処理する。

use crate::assets::GameAssets;
use crate::constants::*;
use crate::entities::damned_soul::DreamPool;
use crate::systems::command::AreaEditSession;
use crate::systems::jobs::{ObstaclePosition, Tree, TreeVariant};
use crate::systems::logistics::ResourceItem;
use crate::systems::visual::plant_trees::PlantTreeVisualState;
use crate::world::map::WorldMap;
use bevy::prelude::*;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;

#[derive(Debug, Clone)]
pub struct DreamTreePlantingPlan {
    pub width_tiles: u32,
    pub height_tiles: u32,
    pub min_square_side: u32,
    pub planned_spawn: u32,
    pub cap_remaining: u32,
    pub affordable: u32,
    pub candidate_count: u32,
    pub selected_tiles: Vec<(i32, i32)>,
}

impl DreamTreePlantingPlan {
    pub fn final_spawn(&self) -> u32 {
        self.selected_tiles.len() as u32
    }

    pub fn cost(&self) -> f32 {
        self.final_spawn() as f32 * DREAM_TREE_COST_PER_TREE
    }
}

pub fn build_dream_tree_planting_plan(
    start: Vec2,
    end: Vec2,
    seed: u64,
    world_map: &WorldMap,
    dream_points: f32,
    current_tree_count: u32,
    q_items: &Query<&Transform, With<ResourceItem>>,
) -> DreamTreePlantingPlan {
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
    let planned_spawn = (area_tiles as f32 * DREAM_TREE_SPAWN_RATE_PER_TILE).floor() as u32;
    let cap_remaining = DREAM_TREE_GLOBAL_CAP.saturating_sub(current_tree_count);
    let affordable = (dream_points / DREAM_TREE_COST_PER_TREE).floor() as u32;

    if width < min_side || height < min_side || cap_remaining == 0 || affordable == 0 {
        return DreamTreePlantingPlan {
            width_tiles: width,
            height_tiles: height,
            min_square_side: min_side,
            planned_spawn,
            cap_remaining,
            affordable,
            candidate_count: 0,
            selected_tiles: Vec::new(),
        };
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

    let candidate_count = candidates.len() as u32;
    let final_spawn = planned_spawn
        .min(candidate_count)
        .min(DREAM_TREE_MAX_PER_CAST)
        .min(cap_remaining)
        .min(affordable);

    if final_spawn > 0 {
        let mut rng = StdRng::seed_from_u64(seed);
        candidates.shuffle(&mut rng);
    }

    DreamTreePlantingPlan {
        width_tiles: width,
        height_tiles: height,
        min_square_side: min_side,
        planned_spawn,
        cap_remaining,
        affordable,
        candidate_count,
        selected_tiles: candidates.into_iter().take(final_spawn as usize).collect(),
    }
}

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
    let Some((start, end, seed)) = area_edit_session.pending_dream_planting.take() else {
        return;
    };

    process_dream_planting(
        &start,
        &end,
        seed,
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
    seed: u64,
    commands: &mut Commands,
    world_map: &mut ResMut<WorldMap>,
    dream_pool: &mut ResMut<DreamPool>,
    game_assets: &Res<GameAssets>,
    q_trees: &Query<&Transform, With<Tree>>,
    q_items: &Query<&Transform, With<ResourceItem>>,
) {
    let current_tree_count = q_trees.iter().count() as u32;
    let plan = build_dream_tree_planting_plan(
        *start,
        *end,
        seed,
        world_map.as_ref(),
        dream_pool.points,
        current_tree_count,
        q_items,
    );

    if plan.width_tiles < plan.min_square_side || plan.height_tiles < plan.min_square_side {
        info!(
            "DREAM_PLANT: AreaTooSmall ({}x{} tiles, need >= {}x{}). 消費なし。",
            plan.width_tiles,
            plan.height_tiles,
            plan.min_square_side,
            plan.min_square_side
        );
        return;
    }
    if plan.cap_remaining == 0 {
        info!(
            "DREAM_PLANT: GlobalCapReached ({} / {}). 消費なし。",
            current_tree_count, DREAM_TREE_GLOBAL_CAP
        );
        return;
    }
    if plan.affordable == 0 {
        info!(
            "DREAM_PLANT: InsufficientDream ({:.1} points). 消費なし。",
            dream_pool.points
        );
        return;
    }
    if plan.candidate_count == 0 {
        info!("DREAM_PLANT: NoCandidateTile in area. 消費なし。");
        return;
    }
    if plan.final_spawn() == 0 {
        info!("DREAM_PLANT: final_spawn == 0. 消費なし。");
        return;
    }

    // Tree をスポーン
    for (index, (gx, gy)) in plan.selected_tiles.iter().copied().enumerate() {
        let pos = WorldMap::grid_to_world(gx, gy);
        let variant_seed = seed.wrapping_add(index as u64 * 7_919);
        let variant_index = (variant_seed as usize) % game_assets.trees.len();
        commands.spawn((
            Tree,
            TreeVariant(variant_index),
            ObstaclePosition(gx, gy),
            PlantTreeVisualState::default(),
            Sprite {
                image: game_assets.trees[variant_index].clone(),
                custom_size: Some(Vec2::splat(TILE_SIZE * 1.5)),
                ..default()
            },
            Transform::from_xyz(pos.x, pos.y, Z_ITEM_OBSTACLE),
        ));
        world_map.add_obstacle(gx, gy);
    }

    let cost = plan.cost();
    dream_pool.points -= cost;

    info!(
        "DREAM_PLANT: {}本生成、{:.1} Dream消費（残:{:.1}）",
        plan.final_spawn(),
        cost,
        dream_pool.points
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
