use super::WorldMap;
use bevy::prelude::*;

impl WorldMap {
    pub fn add_bridged_tile(&mut self, grid: (i32, i32)) {
        let was_walkable = self.is_walkable(grid.0, grid.1);
        if self.bridged_tiles.insert(grid) && was_walkable != self.is_walkable(grid.0, grid.1) {
            self.bump_obstacle_version();
        }
    }

    pub fn register_bridge_tile(&mut self, grid: (i32, i32), entity: Entity) {
        self.add_bridged_tile(grid);
        self.set_building(grid, entity);
    }
}
