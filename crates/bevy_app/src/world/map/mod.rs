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
use hw_ui::selection::WorldReadApi;

#[derive(Component)]
pub struct Tile;

/// Lightweight `&WorldMap` wrapper implementing [`WorldReadApi`].
///
/// Use this instead of per-file wrapper structs when passing a world reference
/// to placement validation helpers in `hw_ui`.
pub struct WorldMapRef<'a>(pub &'a WorldMap);

impl WorldReadApi for WorldMapRef<'_> {
    fn has_building(&self, grid: (i32, i32)) -> bool {
        self.0.has_building(grid)
    }
    fn has_stockpile(&self, grid: (i32, i32)) -> bool {
        self.0.has_stockpile(grid)
    }
    fn is_walkable(&self, gx: i32, gy: i32) -> bool {
        self.0.is_walkable(gx, gy)
    }
    fn is_river_tile(&self, gx: i32, gy: i32) -> bool {
        self.0.is_river_tile(gx, gy)
    }
    fn building_entity(&self, grid: (i32, i32)) -> Option<Entity> {
        self.0.building_entity(grid)
    }
    fn stockpile_entity(&self, grid: (i32, i32)) -> Option<Entity> {
        self.0.stockpile_entity(grid)
    }
    fn pos_to_idx(&self, gx: i32, gy: i32) -> Option<usize> {
        self.0.pos_to_idx(gx, gy)
    }
}
