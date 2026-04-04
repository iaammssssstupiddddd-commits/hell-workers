//! マップスポーン

use crate::plugins::startup::Terrain3dHandles;
use bevy::prelude::*;
use hw_core::constants::{MAP_WIDTH, MAP_HEIGHT, building_3d_render_layers};
use hw_visual::SectionMaterial;
use hw_world::{TerrainType, generate_world_layout, grid_to_world};

use super::{Tile, WorldMapWrite};

const WORLDGEN_SEED_ENV: &str = "HELL_WORKERS_WORLDGEN_SEED";

fn preview_worldgen_seed() -> u64 {
    match std::env::var(WORLDGEN_SEED_ENV) {
        Ok(raw) => match raw.parse::<u64>() {
            Ok(seed) => seed,
            Err(err) => {
                warn!(
                    "BEVY_STARTUP: invalid {}='{}' ({err}); falling back to random seed",
                    WORLDGEN_SEED_ENV,
                    raw
                );
                rand::random::<u64>()
            }
        },
        Err(_) => rand::random::<u64>(),
    }
}

pub fn spawn_map(
    mut commands: Commands,
    terrain_handles: Res<Terrain3dHandles>,
    mut world_map: WorldMapWrite,
) {
    // Temporary preview hook for MS-WFC-2a/2b:
    // render the map from `generate_world_layout()` so seed-based river changes
    // are visible before the full startup/resource integration in MS-WFC-4.
    let master_seed = preview_worldgen_seed();
    let layout = generate_world_layout(master_seed);
    let terrain_tiles = layout.terrain_tiles;

    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let idx = world_map
                .pos_to_idx(x, y)
                .expect("x/y within MAP_WIDTH x MAP_HEIGHT");
            let terrain = terrain_tiles[idx];
            let material = terrain_material(terrain, &terrain_handles);
            world_map.set_terrain_at_idx(idx, terrain);

            let pos2d = grid_to_world(x, y);
            let entity = commands
                .spawn((
                    Tile,
                    Mesh3d(terrain_handles.tile_mesh.clone()),
                    MeshMaterial3d(material),
                    Transform::from_xyz(pos2d.x, 0.0, -pos2d.y),
                    building_3d_render_layers(),
                ))
                .id();

            world_map.set_tile_entity_at_idx(idx, entity);
        }
    }

    info!(
        "BEVY_STARTUP: Map spawned ({}x{} tiles, preview worldgen seed={})",
        MAP_WIDTH, MAP_HEIGHT, master_seed
    );
}

pub fn terrain_material(terrain: TerrainType, handles: &Terrain3dHandles) -> Handle<SectionMaterial> {
    match terrain {
        TerrainType::River => handles.river.clone(),
        TerrainType::Sand  => handles.sand.clone(),
        TerrainType::Dirt  => handles.dirt.clone(),
        TerrainType::Grass => handles.grass.clone(),
    }
}
