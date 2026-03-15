mod facilities;
mod layout;
mod report;
mod terrain_resources;

use crate::assets::GameAssets;
use crate::world::map::WorldMapWrite;
use bevy::prelude::*;

use facilities::{spawn_site_and_yard, spawn_wheelbarrow_parking};
use layout::{compute_parking_layout, compute_site_yard_layout};
use report::InitialSpawnReport;
use terrain_resources::{spawn_initial_wood, spawn_rocks, spawn_trees};

const INITIAL_WHEELBARROW_PARKING_GRID: (i32, i32) = (58, 58);

/// 初期リソースをすべてスポーンする。スポーン順序は重要:
/// 1. 地形障害物（grid obstacle 登録を伴う）
/// 2. 拾得可能アイテム
/// 3. 施設（障害物スポーン後に grid 登録）
pub fn initial_resource_spawner(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    mut world_map: WorldMapWrite,
) {
    let trees = spawn_trees(&mut commands, &game_assets, &mut world_map);
    let rocks = spawn_rocks(&mut commands, &game_assets, &mut world_map);
    let wood = spawn_initial_wood(&mut commands, &game_assets, &world_map);

    let site_yard_spawned = match compute_site_yard_layout() {
        Ok(layout) => {
            spawn_site_and_yard(&mut commands, &layout);
            true
        }
        Err(e) => {
            warn!("INITIAL_SPAWN: skipped Site/Yard — {}", e);
            false
        }
    };

    let parking_spawned = match compute_parking_layout(INITIAL_WHEELBARROW_PARKING_GRID, &world_map)
    {
        Some(layout) => {
            spawn_wheelbarrow_parking(&mut commands, &game_assets, &mut world_map, &layout);
            true
        }
        None => {
            warn!(
                "INITIAL_SPAWN: skipped initial wheelbarrow parking at {:?} (not walkable)",
                INITIAL_WHEELBARROW_PARKING_GRID
            );
            false
        }
    };

    InitialSpawnReport {
        trees_spawned: trees,
        rocks_spawned: rocks,
        wood_spawned: wood,
        site_yard_spawned,
        parking_spawned,
        total_obstacles: world_map.obstacle_count(),
    }
    .log();
}
