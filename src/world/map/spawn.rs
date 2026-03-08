//! マップスポーン

use crate::assets::GameAssets;
use hw_core::constants::*;
use hw_world::{SAND_WIDTH, generate_fixed_river_tiles, generate_sand_tiles};
use bevy::prelude::*;

use super::{TerrainType, Tile, WorldMap};

pub fn spawn_map(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    mut world_map: ResMut<WorldMap>,
) {
    let river_tiles = generate_fixed_river_tiles();
    let sand_tiles = generate_sand_tiles(&river_tiles, MAP_HEIGHT, SAND_WIDTH);

    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let (terrain, texture) = if river_tiles.contains(&(x, y)) {
                (TerrainType::River, game_assets.river.clone())
            } else if sand_tiles.contains(&(x, y)) {
                (TerrainType::Sand, game_assets.sand.clone())
            } else if (x + y) % 30 == 0 {
                (TerrainType::Dirt, game_assets.dirt.clone())
            } else {
                (TerrainType::Grass, game_assets.grass.clone())
            };

            let idx = world_map.pos_to_idx(x, y).unwrap();
            world_map.tiles[idx] = terrain;

            let pos = WorldMap::grid_to_world(x, y);
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

            world_map.tile_entities[idx] = Some(entity);
        }
    }

    info!(
        "BEVY_STARTUP: Map spawned ({}x{} tiles, fixed river layout)",
        MAP_WIDTH, MAP_HEIGHT
    );
}
