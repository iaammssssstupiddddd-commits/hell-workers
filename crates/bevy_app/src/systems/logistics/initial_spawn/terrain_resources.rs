use crate::assets::GameAssets;
use crate::systems::jobs::{ObstaclePosition, Rock, Tree, TreeVariant};
use crate::systems::logistics::{ResourceItem, ResourceType};
use crate::world::map::{INITIAL_WOOD_POSITIONS, ROCK_POSITIONS, TREE_POSITIONS, WorldMap};
use bevy::prelude::*;
use hw_core::constants::*;

/// 障害物スポーンの共通 helper。
/// 各位置で地形の通行可能チェック（`terrain_at_idx` ベース）を行い、
/// `make_bundle` が返す Bundle を spawn して `add_grid_obstacle` を呼ぶ。
/// 実際に spawn した個数を返す。
fn spawn_obstacle_batch<B: Bundle>(
    positions: &[(i32, i32)],
    commands: &mut Commands,
    world_map: &mut WorldMap,
    make_bundle: impl Fn(i32, i32, Vec2) -> B,
) -> usize {
    let mut count = 0;
    for &(gx, gy) in positions {
        if let Some(idx) = world_map.pos_to_idx(gx, gy)
            && world_map
                .terrain_at_idx(idx)
                .is_some_and(|terrain| terrain.is_walkable())
            {
                let pos = WorldMap::grid_to_world(gx, gy);
                commands.spawn(make_bundle(gx, gy, pos));
                world_map.add_grid_obstacle((gx, gy));
                count += 1;
            }
    }
    count
}

pub fn spawn_trees(
    commands: &mut Commands,
    game_assets: &GameAssets,
    world_map: &mut WorldMap,
) -> usize {
    spawn_obstacle_batch(TREE_POSITIONS, commands, world_map, |gx, gy, pos| {
        let variant_index = rand::random::<usize>() % game_assets.trees.len();
        (
            Tree,
            TreeVariant(variant_index),
            ObstaclePosition(gx, gy),
            Sprite {
                image: game_assets.trees[variant_index].clone(),
                custom_size: Some(Vec2::splat(TILE_SIZE * 1.5)),
                ..default()
            },
            Transform::from_xyz(pos.x, pos.y, Z_ITEM_OBSTACLE),
        )
    })
}

pub fn spawn_rocks(
    commands: &mut Commands,
    game_assets: &GameAssets,
    world_map: &mut WorldMap,
) -> usize {
    spawn_obstacle_batch(ROCK_POSITIONS, commands, world_map, |gx, gy, pos| {
        (
            Rock,
            ObstaclePosition(gx, gy),
            Sprite {
                image: game_assets.rock.clone(),
                custom_size: Some(Vec2::splat(TILE_SIZE * 1.2)),
                ..default()
            },
            Transform::from_xyz(pos.x, pos.y, Z_ITEM_OBSTACLE),
        )
    })
}

/// 木材アイテムを初期配置する。
/// 障害物にはならないため `is_walkable()` 直呼びで十分。
pub fn spawn_initial_wood(
    commands: &mut Commands,
    game_assets: &GameAssets,
    world_map: &WorldMap,
) -> usize {
    let mut count = 0;
    for &(gx, gy) in INITIAL_WOOD_POSITIONS {
        if world_map.is_walkable(gx, gy) {
            let spawn_pos = WorldMap::grid_to_world(gx, gy);
            commands.spawn((
                ResourceItem(ResourceType::Wood),
                Sprite {
                    image: game_assets.wood.clone(),
                    custom_size: Some(Vec2::splat(TILE_SIZE * 0.5)),
                    ..default()
                },
                Transform::from_xyz(spawn_pos.x, spawn_pos.y, Z_ITEM_PICKUP),
            ));
            count += 1;
        }
    }
    count
}
