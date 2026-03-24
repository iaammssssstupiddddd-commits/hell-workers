use super::WorldMap;
use crate::{
    TerrainType, find_nearest_river_grid, find_nearest_walkable_grid, grid_to_world, idx_to_pos,
    snap_to_grid_center, snap_to_grid_edge, world_to_grid,
};
use bevy::prelude::*;
use hw_core::GridPos;

impl WorldMap {
    #[inline(always)]
    pub fn pos_to_idx(&self, x: i32, y: i32) -> Option<usize> {
        use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};
        if !(0..MAP_WIDTH).contains(&x) || !(0..MAP_HEIGHT).contains(&y) {
            return None;
        }
        Some((y * MAP_WIDTH + x) as usize)
    }

    #[inline(always)]
    pub fn idx_to_pos(idx: usize) -> GridPos {
        idx_to_pos(idx)
    }

    pub fn is_walkable(&self, x: i32, y: i32) -> bool {
        use hw_core::world::DoorState;
        let idx = match self.pos_to_idx(x, y) {
            Some(i) => i,
            None => return false,
        };
        if let Some(state) = self.door_states.get(&(x, y)) {
            return *state != DoorState::Locked;
        }
        if self.obstacles[idx] {
            return false;
        }
        if self.bridged_tiles.contains(&(x, y)) {
            return true;
        }
        self.tiles[idx].is_walkable()
    }

    pub fn is_river_tile(&self, x: i32, y: i32) -> bool {
        let Some(idx) = self.pos_to_idx(x, y) else {
            return false;
        };
        self.terrain_at_idx(idx) == Some(TerrainType::River)
    }

    pub fn terrain_at_idx(&self, idx: usize) -> Option<TerrainType> {
        self.tiles.get(idx).copied()
    }

    pub fn terrain_tiles(&self) -> &[TerrainType] {
        &self.tiles
    }

    pub fn set_terrain_at_idx(&mut self, idx: usize, terrain: TerrainType) {
        if let Some(slot) = self.tiles.get_mut(idx) {
            *slot = terrain;
        }
    }

    pub fn tile_entity_at_idx(&self, idx: usize) -> Option<Entity> {
        self.tile_entities.get(idx).and_then(|entity| *entity)
    }

    pub fn set_tile_entity_at_idx(&mut self, idx: usize, entity: Entity) {
        if let Some(slot) = self.tile_entities.get_mut(idx) {
            *slot = Some(entity);
        }
    }

    pub fn world_to_grid(pos: Vec2) -> (i32, i32) {
        world_to_grid(pos)
    }

    pub fn grid_to_world(x: i32, y: i32) -> Vec2 {
        grid_to_world(x, y)
    }

    pub fn snap_to_grid_center(pos: Vec2) -> Vec2 {
        snap_to_grid_center(pos)
    }

    pub fn snap_to_grid_edge(pos: Vec2) -> Vec2 {
        snap_to_grid_edge(pos)
    }

    pub fn is_walkable_world(&self, pos: Vec2) -> bool {
        let grid = Self::world_to_grid(pos);
        self.is_walkable(grid.0, grid.1)
    }

    pub fn get_nearest_walkable_grid(&self, pos: Vec2) -> Option<(i32, i32)> {
        find_nearest_walkable_grid(self, pos, 5)
    }

    pub fn get_nearest_river_grid(&self, pos: Vec2) -> Option<(i32, i32)> {
        find_nearest_river_grid(pos, &self.tiles)
    }
}
