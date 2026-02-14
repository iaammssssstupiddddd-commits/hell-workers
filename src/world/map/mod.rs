//! ワールドマップと座標変換API

mod layout;
mod spawn;
pub mod terrain_border;

pub use layout::{
    INITIAL_WOOD_POSITIONS, RIVER_X_MAX, RIVER_X_MIN, RIVER_Y_MAX, RIVER_Y_MIN, ROCK_POSITIONS,
    SAND_WIDTH, TREE_POSITIONS,
};
pub use spawn::{generate_fixed_river_tiles, spawn_map};

use crate::constants::*;
use bevy::prelude::*;
use std::collections::HashMap;

#[derive(Component)]
pub struct Tile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainType {
    Grass,
    Dirt,
    River,
    Sand,
}

impl TerrainType {
    pub fn is_walkable(&self) -> bool {
        match self {
            TerrainType::Grass | TerrainType::Dirt | TerrainType::Sand => true,
            TerrainType::River => false,
        }
    }

    pub fn z_layer(&self) -> f32 {
        match self {
            TerrainType::River => Z_MAP,
            TerrainType::Sand => Z_MAP_SAND,
            TerrainType::Dirt => Z_MAP_DIRT,
            TerrainType::Grass => Z_MAP_GRASS,
        }
    }

    pub fn priority(&self) -> u8 {
        match self {
            TerrainType::River => 0,
            TerrainType::Sand => 1,
            TerrainType::Dirt => 2,
            TerrainType::Grass => 3,
        }
    }
}

#[derive(Resource)]
pub struct WorldMap {
    pub tiles: Vec<TerrainType>,
    pub tile_entities: Vec<Option<Entity>>,
    pub buildings: HashMap<(i32, i32), Entity>,
    pub stockpiles: HashMap<(i32, i32), Entity>,
    pub obstacles: Vec<bool>,
}

impl Default for WorldMap {
    fn default() -> Self {
        let size = (MAP_WIDTH * MAP_HEIGHT) as usize;
        Self {
            tiles: vec![TerrainType::Grass; size],
            tile_entities: vec![None; size],
            buildings: HashMap::new(),
            stockpiles: HashMap::new(),
            obstacles: vec![false; size],
        }
    }
}

impl WorldMap {
    #[inline(always)]
    pub fn pos_to_idx(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || x >= MAP_WIDTH || y < 0 || y >= MAP_HEIGHT {
            return None;
        }
        Some((y * MAP_WIDTH + x) as usize)
    }

    #[inline(always)]
    pub fn idx_to_pos(idx: usize) -> (i32, i32) {
        let x = idx as i32 % MAP_WIDTH;
        let y = idx as i32 / MAP_WIDTH;
        (x, y)
    }

    pub fn is_walkable(&self, x: i32, y: i32) -> bool {
        let idx = match self.pos_to_idx(x, y) {
            Some(i) => i,
            None => return false,
        };
        if self.obstacles[idx] {
            return false;
        }
        self.tiles[idx].is_walkable()
    }

    pub fn add_obstacle(&mut self, x: i32, y: i32) {
        if let Some(idx) = self.pos_to_idx(x, y) {
            self.obstacles[idx] = true;
        }
    }

    pub fn remove_obstacle(&mut self, x: i32, y: i32) {
        if let Some(idx) = self.pos_to_idx(x, y) {
            self.obstacles[idx] = false;
        }
    }

    pub fn world_to_grid(pos: Vec2) -> (i32, i32) {
        let x = (pos.x / TILE_SIZE + (MAP_WIDTH as f32 - 1.0) / 2.0 + 0.5).floor() as i32;
        let y = (pos.y / TILE_SIZE + (MAP_HEIGHT as f32 - 1.0) / 2.0 + 0.5).floor() as i32;
        (x, y)
    }

    pub fn grid_to_world(x: i32, y: i32) -> Vec2 {
        Vec2::new(
            (x as f32 - (MAP_WIDTH as f32 - 1.0) / 2.0) * TILE_SIZE,
            (y as f32 - (MAP_HEIGHT as f32 - 1.0) / 2.0) * TILE_SIZE,
        )
    }

    pub fn snap_to_grid_center(pos: Vec2) -> Vec2 {
        let (x, y) = Self::world_to_grid(pos);
        Self::grid_to_world(x, y)
    }

    pub fn snap_to_grid_edge(pos: Vec2) -> Vec2 {
        let map_offset_x = (MAP_WIDTH as f32 * TILE_SIZE) / 2.0;
        let map_offset_y = (MAP_HEIGHT as f32 * TILE_SIZE) / 2.0;
        let local_x = pos.x + map_offset_x;
        let local_y = pos.y + map_offset_y;
        let snapped_local_x = (local_x / TILE_SIZE).round() * TILE_SIZE;
        let snapped_local_y = (local_y / TILE_SIZE).round() * TILE_SIZE;
        Vec2::new(
            snapped_local_x - map_offset_x,
            snapped_local_y - map_offset_y,
        )
    }

    pub fn is_walkable_world(&self, pos: Vec2) -> bool {
        let grid = Self::world_to_grid(pos);
        self.is_walkable(grid.0, grid.1)
    }

    pub fn get_nearest_walkable_grid(&self, pos: Vec2) -> Option<(i32, i32)> {
        let grid = Self::world_to_grid(pos);
        if self.is_walkable(grid.0, grid.1) {
            return Some(grid);
        }
        for r in 1..=5 {
            for dx in -r..=r {
                for dy in -r..=r {
                    let test = (grid.0 + dx, grid.1 + dy);
                    if self.is_walkable(test.0, test.1) {
                        return Some(test);
                    }
                }
            }
        }
        None
    }

    pub fn get_nearest_river_grid(&self, pos: Vec2) -> Option<(i32, i32)> {
        let from = Self::world_to_grid(pos);
        let mut nearest: Option<(i32, i32)> = None;
        let mut nearest_dist_sq = i64::MAX;

        for (idx, terrain) in self.tiles.iter().enumerate() {
            if *terrain != TerrainType::River {
                continue;
            }

            let (x, y) = Self::idx_to_pos(idx);
            let dx = (x - from.0) as i64;
            let dy = (y - from.1) as i64;
            let dist_sq = dx * dx + dy * dy;
            if dist_sq < nearest_dist_sq {
                nearest_dist_sq = dist_sq;
                nearest = Some((x, y));
            }
        }

        nearest
    }
}
