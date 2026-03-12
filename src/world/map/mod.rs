//! ワールドマップと座標変換API

mod spawn;
pub mod terrain_border;

pub use hw_world::layout::{
    INITIAL_WOOD_POSITIONS, RIVER_X_MAX, RIVER_X_MIN, RIVER_Y_MAX, RIVER_Y_MIN, ROCK_POSITIONS,
    SAND_WIDTH, TREE_POSITIONS,
};
pub use hw_world::map::WorldMap;
pub use hw_world::{TerrainType, WorldMapRead, WorldMapWrite, generate_fixed_river_tiles};
pub use spawn::spawn_map;

use bevy::prelude::*;

#[derive(Component)]
pub struct Tile;
