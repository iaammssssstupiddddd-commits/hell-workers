//! マップスポーン

use crate::plugins::startup::Terrain3dHandles;
use bevy::prelude::*;
use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH, building_3d_render_layers};
use hw_visual::TerrainSurfaceMaterial;
use hw_world::{GeneratedWorldLayout, generate_world_layout, grid_to_world};

use super::{Tile, WorldMapWrite};

const WORLDGEN_SEED_ENV: &str = "HELL_WORKERS_WORLDGEN_SEED";

#[derive(Resource, Clone)]
pub struct GeneratedWorldLayoutResource {
    pub master_seed: u64,
    pub layout: GeneratedWorldLayout,
}

pub fn resolve_worldgen_seed() -> u64 {
    match std::env::var(WORLDGEN_SEED_ENV) {
        Ok(raw) => match raw.parse::<u64>() {
            Ok(seed) => seed,
            Err(err) => {
                warn!(
                    "BEVY_STARTUP: invalid {}='{}' ({err}); falling back to random seed",
                    WORLDGEN_SEED_ENV, raw
                );
                rand::random::<u64>()
            }
        },
        Err(_) => rand::random::<u64>(),
    }
}

pub fn prepare_generated_world_layout_resource() -> GeneratedWorldLayoutResource {
    let master_seed = resolve_worldgen_seed();
    let layout = generate_world_layout(master_seed);
    GeneratedWorldLayoutResource {
        master_seed,
        layout,
    }
}

pub fn spawn_map(
    mut commands: Commands,
    terrain_handles: Res<Terrain3dHandles>,
    mut world_map: WorldMapWrite,
    generated_layout: Res<GeneratedWorldLayoutResource>,
) {
    let terrain_tiles = &generated_layout.layout.terrain_tiles;

    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let idx = world_map
                .pos_to_idx(x, y)
                .expect("x/y within MAP_WIDTH x MAP_HEIGHT");
            let terrain = terrain_tiles[idx];
            world_map.set_terrain_at_idx(idx, terrain);

            let pos2d = grid_to_world(x, y);
            let entity = commands
                .spawn((
                    Tile,
                    Mesh3d(terrain_handles.tile_mesh.clone()),
                    MeshMaterial3d::<TerrainSurfaceMaterial>(terrain_handles.surface.clone()),
                    Transform::from_xyz(pos2d.x, 0.0, -pos2d.y),
                    building_3d_render_layers(),
                ))
                .id();

            world_map.set_tile_entity_at_idx(idx, entity);
        }
    }

    info!(
        "BEVY_STARTUP: Map spawned ({}x{} tiles, worldgen seed={}, attempt={}, fallback={})",
        MAP_WIDTH,
        MAP_HEIGHT,
        generated_layout.master_seed,
        generated_layout.layout.generation_attempt,
        generated_layout.layout.used_fallback
    );
}
