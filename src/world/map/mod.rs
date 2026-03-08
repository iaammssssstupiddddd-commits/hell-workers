//! ワールドマップと座標変換API

mod access;
mod layout;
mod spawn;
pub mod terrain_border;

use crate::systems::jobs::BuildingType;
pub use access::{WorldMapRead, WorldMapWrite};
pub use layout::{
    INITIAL_WOOD_POSITIONS, RIVER_X_MAX, RIVER_X_MIN, RIVER_Y_MAX, RIVER_Y_MIN, ROCK_POSITIONS,
    SAND_WIDTH, TREE_POSITIONS,
};
pub use hw_world::generate_fixed_river_tiles;
pub use hw_world::TerrainType;
pub use spawn::spawn_map;

use hw_core::constants::*;
use hw_core::world::DoorState;
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

#[derive(Component)]
pub struct Tile;

#[derive(Resource)]
pub struct WorldMap {
    pub tiles: Vec<TerrainType>,
    pub tile_entities: Vec<Option<Entity>>,
    pub buildings: HashMap<(i32, i32), Entity>,
    pub doors: HashMap<(i32, i32), Entity>,
    pub door_states: HashMap<(i32, i32), DoorState>,
    pub stockpiles: HashMap<(i32, i32), Entity>,
    pub bridged_tiles: HashSet<(i32, i32)>,
    pub obstacles: Vec<bool>,
}

impl Default for WorldMap {
    fn default() -> Self {
        let size = (MAP_WIDTH * MAP_HEIGHT) as usize;
        Self {
            tiles: vec![TerrainType::Grass; size],
            tile_entities: vec![None; size],
            buildings: HashMap::new(),
            doors: HashMap::new(),
            door_states: HashMap::new(),
            stockpiles: HashMap::new(),
            bridged_tiles: HashSet::new(),
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

    pub fn obstacle_count(&self) -> usize {
        self.obstacles.iter().filter(|&&blocked| blocked).count()
    }

    pub fn obstacle_indices(&self) -> impl Iterator<Item = usize> + '_ {
        self.obstacles
            .iter()
            .enumerate()
            .filter_map(|(idx, blocked)| blocked.then_some(idx))
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

    pub fn add_grid_obstacle(&mut self, grid: (i32, i32)) {
        self.add_obstacle(grid.0, grid.1);
    }

    pub fn remove_grid_obstacle(&mut self, grid: (i32, i32)) {
        self.remove_obstacle(grid.0, grid.1);
    }

    pub fn add_grid_obstacles<I>(&mut self, grids: I)
    where
        I: IntoIterator<Item = (i32, i32)>,
    {
        for grid in grids {
            self.add_grid_obstacle(grid);
        }
    }

    pub fn reserve_building_footprint_tiles<I>(&mut self, grids: I)
    where
        I: IntoIterator<Item = (i32, i32)>,
    {
        self.add_grid_obstacles(grids);
    }

    pub fn add_door(&mut self, x: i32, y: i32, door_entity: Entity, state: DoorState) {
        self.doors.insert((x, y), door_entity);
        self.door_states.insert((x, y), state);
    }

    pub fn remove_door(&mut self, x: i32, y: i32) {
        self.doors.remove(&(x, y));
        self.door_states.remove(&(x, y));
    }

    pub fn building_entity(&self, grid: (i32, i32)) -> Option<Entity> {
        self.buildings.get(&grid).copied()
    }

    pub fn has_building(&self, grid: (i32, i32)) -> bool {
        self.buildings.contains_key(&grid)
    }

    pub fn set_building(&mut self, grid: (i32, i32), entity: Entity) {
        self.buildings.insert(grid, entity);
    }

    pub fn clear_building(&mut self, grid: (i32, i32)) -> Option<Entity> {
        self.buildings.remove(&grid)
    }

    pub fn set_building_occupancy(&mut self, grid: (i32, i32), entity: Entity) {
        self.set_building(grid, entity);
        self.add_obstacle(grid.0, grid.1);
    }

    pub fn set_building_occupancies<I>(&mut self, entity: Entity, grids: I)
    where
        I: IntoIterator<Item = (i32, i32)>,
    {
        for grid in grids {
            self.set_building_occupancy(grid, entity);
        }
    }

    pub fn clear_building_occupancy(&mut self, grid: (i32, i32)) -> Option<Entity> {
        let entity = self.clear_building(grid);
        self.remove_obstacle(grid.0, grid.1);
        entity
    }

    pub fn clear_building_footprint<I>(&mut self, grids: I)
    where
        I: IntoIterator<Item = (i32, i32)>,
    {
        for grid in grids {
            self.clear_building_occupancy(grid);
        }
    }

    pub fn clear_building_occupancy_if_owned(
        &mut self,
        grid: (i32, i32),
        entity: Entity,
    ) -> bool {
        if self.building_entity(grid) != Some(entity) {
            return false;
        }
        self.clear_building_occupancy(grid);
        true
    }

    pub fn release_building_grid_if_owned(&mut self, grid: (i32, i32), entity: Entity) -> bool {
        if self.clear_building_occupancy_if_owned(grid, entity) {
            return true;
        }
        self.remove_grid_obstacle(grid);
        false
    }

    pub fn release_building_grids_if_owned<I>(&mut self, entity: Entity, grids: I)
    where
        I: IntoIterator<Item = (i32, i32)>,
    {
        for grid in grids {
            self.release_building_grid_if_owned(grid, entity);
        }
    }

    pub fn release_building_footprint_if_owned<I>(&mut self, entity: Entity, grids: I)
    where
        I: IntoIterator<Item = (i32, i32)>,
    {
        self.release_building_grids_if_owned(entity, grids);
    }

    pub fn release_building_footprint_if_matches<I>(&mut self, entity: Entity, grids: I)
    where
        I: IntoIterator<Item = ((i32, i32), Option<Entity>)>,
    {
        for (grid, alternate) in grids {
            self.release_building_grid_if_matches(grid, entity, alternate);
        }
    }

    pub fn release_building_grid_if_matches(
        &mut self,
        grid: (i32, i32),
        entity: Entity,
        alternate: Option<Entity>,
    ) -> bool {
        if self
            .building_entity(grid)
            .is_some_and(|current| current == entity || Some(current) == alternate)
        {
            self.clear_building_occupancy(grid);
            return true;
        }
        self.remove_grid_obstacle(grid);
        false
    }

    pub fn building_entries(&self) -> impl Iterator<Item = (&(i32, i32), &Entity)> {
        self.buildings.iter()
    }

    pub fn stockpile_entity(&self, grid: (i32, i32)) -> Option<Entity> {
        self.stockpiles.get(&grid).copied()
    }

    pub fn has_stockpile(&self, grid: (i32, i32)) -> bool {
        self.stockpiles.contains_key(&grid)
    }

    pub fn set_stockpile(&mut self, grid: (i32, i32), entity: Entity) {
        self.stockpiles.insert(grid, entity);
    }

    pub fn clear_stockpile(&mut self, grid: (i32, i32)) -> Option<Entity> {
        self.stockpiles.remove(&grid)
    }

    pub fn register_stockpile_tile(&mut self, grid: (i32, i32), entity: Entity) {
        self.set_stockpile(grid, entity);
    }

    pub fn move_stockpile_tile(
        &mut self,
        entity: Entity,
        old_grid: (i32, i32),
        new_grid: (i32, i32),
    ) {
        if self.stockpile_entity(old_grid) == Some(entity) {
            self.clear_stockpile(old_grid);
        }
        self.set_stockpile(new_grid, entity);
    }

    pub fn clear_stockpile_tile_if_owned(
        &mut self,
        grid: (i32, i32),
        entity: Entity,
    ) -> bool {
        if self.stockpile_entity(grid) != Some(entity) {
            return false;
        }
        self.clear_stockpile(grid);
        true
    }

    pub fn take_stockpile_tiles<I>(&mut self, grids: I) -> Vec<Entity>
    where
        I: IntoIterator<Item = (i32, i32)>,
    {
        grids.into_iter()
            .filter_map(|grid| self.clear_stockpile(grid))
            .collect()
    }

    pub fn stockpile_entries(&self) -> impl Iterator<Item = (&(i32, i32), &Entity)> {
        self.stockpiles.iter()
    }

    pub fn add_bridged_tile(&mut self, grid: (i32, i32)) {
        self.bridged_tiles.insert(grid);
    }

    pub fn register_bridge_tile(&mut self, grid: (i32, i32), entity: Entity) {
        self.add_bridged_tile(grid);
        self.set_building(grid, entity);
    }

    pub fn reserve_building_footprint<I>(
        &mut self,
        building_type: BuildingType,
        entity: Entity,
        grids: I,
    ) where
        I: IntoIterator<Item = (i32, i32)>,
    {
        match building_type {
            BuildingType::Bridge => {
                for grid in grids {
                    self.set_building(grid, entity);
                }
            }
            _ => self.set_building_occupancies(entity, grids),
        }
    }

    pub fn register_completed_building_footprint<I>(
        &mut self,
        building_type: BuildingType,
        entity: Entity,
        grids: I,
    ) where
        I: IntoIterator<Item = (i32, i32)>,
    {
        match building_type {
            BuildingType::Bridge => {
                for grid in grids {
                    self.register_bridge_tile(grid, entity);
                }
            }
            BuildingType::Door => {
                for grid in grids {
                    self.register_door(grid, entity, DoorState::Closed);
                }
            }
            _ => self.set_building_occupancies(entity, grids),
        }
    }

    pub fn set_door_state(&mut self, x: i32, y: i32, state: DoorState) {
        if self.doors.contains_key(&(x, y)) {
            self.door_states.insert((x, y), state);
        }
    }

    pub fn door_entity(&self, x: i32, y: i32) -> Option<Entity> {
        self.doors.get(&(x, y)).copied()
    }

    pub fn door_state(&self, x: i32, y: i32) -> Option<DoorState> {
        self.door_states.get(&(x, y)).copied()
    }

    pub fn get_door_cost(&self, x: i32, y: i32) -> i32 {
        match self.door_states.get(&(x, y)).copied() {
            Some(DoorState::Closed) => DOOR_OPEN_COST,
            _ => 0,
        }
    }

    pub fn register_door(&mut self, grid: (i32, i32), entity: Entity, state: DoorState) {
        self.set_building_occupancy(grid, entity);
        self.add_door(grid.0, grid.1, entity, state);
    }

    pub fn sync_door_passability(&mut self, grid: (i32, i32), state: DoorState) {
        match state {
            DoorState::Open => self.remove_obstacle(grid.0, grid.1),
            DoorState::Closed | DoorState::Locked => self.add_obstacle(grid.0, grid.1),
        }
        self.set_door_state(grid.0, grid.1, state);
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
