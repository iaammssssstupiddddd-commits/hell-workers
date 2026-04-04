mod facilities;
mod layout;
mod report;
mod terrain_resources;

use crate::assets::GameAssets;
use crate::world::map::GeneratedWorldLayoutResource;
use crate::world::map::WorldMapWrite;
use bevy::prelude::*;

use facilities::{spawn_site_and_yard, spawn_wheelbarrow_parking};
use layout::{compute_parking_layout, site_yard_layout_from_anchor};
use report::InitialSpawnReport;
use terrain_resources::{spawn_initial_wood, spawn_rocks, spawn_trees};

/// 初期リソースをすべてスポーンする。スポーン順序は重要:
/// 1. 地形障害物（grid obstacle 登録を伴う）
/// 2. 拾得可能アイテム
/// 3. 施設（障害物スポーン後に grid 登録）
pub fn initial_resource_spawner(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    mut world_map: WorldMapWrite,
    generated_layout: &GeneratedWorldLayoutResource,
) {
    let layout = &generated_layout.layout;
    let trees = spawn_trees(
        &mut commands,
        &game_assets,
        &mut world_map,
        &layout.initial_tree_positions,
    );
    let rocks = spawn_rocks(
        &mut commands,
        &game_assets,
        &mut world_map,
        &layout.initial_rock_positions,
    );
    let wood = spawn_initial_wood(
        &mut commands,
        &game_assets,
        &world_map,
        &layout.anchors.initial_wood_positions,
    );

    let site_yard = site_yard_layout_from_anchor(&layout.anchors);
    spawn_site_and_yard(&mut commands, &site_yard);
    let site_yard_spawned = true;

    let parking_base = (
        layout.anchors.wheelbarrow_parking.min_x,
        layout.anchors.wheelbarrow_parking.min_y,
    );
    let parking_spawned = match compute_parking_layout(parking_base, &world_map) {
        Some(layout) => {
            spawn_wheelbarrow_parking(&mut commands, &game_assets, &mut world_map, &layout);
            true
        }
        None => {
            warn!(
                "INITIAL_SPAWN: skipped initial wheelbarrow parking at {:?} (not walkable)",
                parking_base
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
        worldgen_seed: generated_layout.master_seed,
        used_fallback: generated_layout.layout.used_fallback,
    }
    .log();
}
