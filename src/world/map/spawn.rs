//! マップスポーン

use crate::assets::GameAssets;
use bevy::prelude::*;
use hw_core::constants::*;
use hw_world::{generate_base_terrain_tiles, grid_to_world};

use super::{TerrainType, Tile, WorldMapWrite};

pub fn spawn_map(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    mut world_map: WorldMapWrite,
) {
    let terrain_tiles = generate_base_terrain_tiles(MAP_WIDTH, MAP_HEIGHT, super::SAND_WIDTH);

    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let idx = world_map.pos_to_idx(x, y).unwrap();
            let terrain = terrain_tiles[idx];
            let texture = terrain_texture(terrain, &game_assets);
            world_map.set_terrain_at_idx(idx, terrain);

            let pos = grid_to_world(x, y);
            let entity = commands
                .spawn((
                    Tile,
                    Sprite {
                        image: texture,
                        custom_size: Some(Vec2::splat(TILE_SIZE)),
                        ..default()
                    },
                    Transform::from_xyz(pos.x, pos.y, Z_MAP),
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

fn terrain_texture(terrain: TerrainType, assets: &GameAssets) -> Handle<Image> {
    match terrain {
        TerrainType::River => assets.river.clone(),
        TerrainType::Sand => assets.sand.clone(),
        TerrainType::Dirt => assets.dirt.clone(),
        TerrainType::Grass => assets.grass.clone(),
    }
}
