use super::WorldMap;
use bevy::prelude::*;

impl WorldMap {
    pub fn add_bridged_tile(&mut self, grid: (i32, i32)) {
        self.bridged_tiles.insert(grid);
    }

    pub fn register_bridge_tile(&mut self, grid: (i32, i32), entity: Entity) {
        self.add_bridged_tile(grid);
        self.set_building(grid, entity);
    }
}
