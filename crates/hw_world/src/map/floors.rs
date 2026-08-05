use super::WorldMap;
use bevy::prelude::*;

impl WorldMap {
    /// Returns the completed Floor stacked beneath ordinary building occupancy.
    pub fn floor_entity(&self, grid: (i32, i32)) -> Option<Entity> {
        self.floors.get(&grid).copied()
    }

    pub fn set_floor(&mut self, grid: (i32, i32), entity: Entity) {
        self.floors.insert(grid, entity);
    }
}
