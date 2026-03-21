mod access;
mod bridges;
mod buildings;
mod doors;
mod obstacles;
mod stockpiles;
mod tiles;

pub use access::{WorldMapRead, WorldMapWrite};

use crate::pathfinding::PathWorld;
use crate::TerrainType;
use bevy::prelude::*;
use hw_core::world::DoorState;
use std::collections::{HashMap, HashSet};

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
        use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};
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

impl PathWorld for WorldMap {
    fn pos_to_idx(&self, x: i32, y: i32) -> Option<usize> {
        WorldMap::pos_to_idx(self, x, y)
    }

    fn idx_to_pos(&self, idx: usize) -> (i32, i32) {
        WorldMap::idx_to_pos(idx)
    }

    fn is_walkable(&self, x: i32, y: i32) -> bool {
        WorldMap::is_walkable(self, x, y)
    }

    fn get_door_cost(&self, x: i32, y: i32) -> i32 {
        WorldMap::get_door_cost(self, x, y)
    }
}
