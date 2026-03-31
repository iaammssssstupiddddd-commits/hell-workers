//! マップスポーン

use crate::plugins::startup::Terrain3dHandles;
use bevy::prelude::*;
use hw_core::constants::{MAP_WIDTH, MAP_HEIGHT, building_3d_render_layers};
use hw_visual::SectionMaterial;
use hw_world::{TerrainType, generate_base_terrain_tiles, grid_to_world};

use super::{Tile, WorldMapWrite};

pub fn spawn_map(
    mut commands: Commands,
    terrain_handles: Res<Terrain3dHandles>,
    mut world_map: WorldMapWrite,
) {
    let terrain_tiles = generate_base_terrain_tiles(MAP_WIDTH, MAP_HEIGHT, super::SAND_WIDTH);

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
        "BEVY_STARTUP: Map spawned ({}x{} tiles, fixed river layout)",
        MAP_WIDTH, MAP_HEIGHT
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
