use super::types::{ResourceItem, ResourceType};
use crate::assets::GameAssets;
use crate::constants::*;
use crate::systems::jobs::{Rock, Tree};
use crate::world::map::{INITIAL_WOOD_POSITIONS, ROCK_POSITIONS, TREE_POSITIONS, WorldMap};
use bevy::prelude::*;

pub fn initial_resource_spawner(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    mut world_map: ResMut<WorldMap>,
) {
    // 木のスポーン（障害物として登録）
    for &(gx, gy) in TREE_POSITIONS {
        // 地形が通行可能な場合のみスポーン（障害物チェックなし）
        if let Some(idx) = world_map.pos_to_idx(gx, gy) {
            if world_map.tiles[idx].is_walkable() {
                let pos = WorldMap::grid_to_world(gx, gy);
                commands.spawn((
                    Tree,
                    crate::systems::jobs::ObstaclePosition(gx, gy),
                    Sprite {
                        image: game_assets.tree.clone(),
                        custom_size: Some(Vec2::splat(TILE_SIZE * 1.5)),
                        ..default()
                    },
                    Transform::from_xyz(pos.x, pos.y, Z_ITEM_OBSTACLE),
                ));
                world_map.add_obstacle(gx, gy);
            }
        }
    }

    // 岩のスポーン（障害物として登録）
    for &(gx, gy) in ROCK_POSITIONS {
        if let Some(idx) = world_map.pos_to_idx(gx, gy) {
            if world_map.tiles[idx].is_walkable() {
                let pos = WorldMap::grid_to_world(gx, gy);
                commands.spawn((
                    Rock,
                    crate::systems::jobs::ObstaclePosition(gx, gy),
                    Sprite {
                        image: game_assets.rock.clone(),
                        custom_size: Some(Vec2::splat(TILE_SIZE * 1.2)),
                        ..default()
                    },
                    Transform::from_xyz(pos.x, pos.y, Z_ITEM_OBSTACLE),
                ));
                world_map.add_obstacle(gx, gy);
            }
        }
    }

    // 既存の資材（木材）も中央付近に少し撒く
    for &(gx, gy) in INITIAL_WOOD_POSITIONS {
        if world_map.is_walkable(gx, gy) {
            let spawn_pos = WorldMap::grid_to_world(gx, gy);
            commands.spawn((
                ResourceItem(ResourceType::Wood),
                Sprite {
                    image: game_assets.wood.clone(),
                    custom_size: Some(Vec2::splat(TILE_SIZE * 0.5)),
                    color: Color::srgb(0.5, 0.35, 0.05),
                    ..default()
                },
                Transform::from_xyz(spawn_pos.x, spawn_pos.y, Z_ITEM_PICKUP),
            ));
        }
    }

    let rock_count = ROCK_POSITIONS.len();
    let obstacle_count = world_map.obstacles.iter().filter(|&&b| b).count();
    info!(
        "SPAWNER: Fixed Trees ({}), Rocks ({}) spawned. WorldMap active obstacles: {}",
        TREE_POSITIONS.len(),
        rock_count,
        obstacle_count
    );
}
