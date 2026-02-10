//! マップスポーン

use crate::assets::GameAssets;
use crate::constants::*;
use bevy::prelude::*;
use std::collections::HashSet;

use super::layout::{RIVER_X_MIN, RIVER_X_MAX, RIVER_Y_MIN, RIVER_Y_MAX, SAND_WIDTH};
use super::{TerrainType, Tile, WorldMap};

/// 固定配置の川タイルを生成
pub fn generate_fixed_river_tiles() -> HashSet<(i32, i32)> {
    let mut river_tiles = HashSet::new();
    for y in RIVER_Y_MIN..=RIVER_Y_MAX {
        for x in RIVER_X_MIN..=RIVER_X_MAX {
            river_tiles.insert((x, y));
        }
    }
    river_tiles
}

pub fn spawn_map(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    mut world_map: ResMut<WorldMap>,
) {
    use crate::world::river::generate_sand_tiles;

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
