use super::WorldMap;
use bevy::prelude::*;

impl WorldMap {
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

    pub fn clear_stockpile_tile_if_owned(&mut self, grid: (i32, i32), entity: Entity) -> bool {
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
        grids
            .into_iter()
            .filter_map(|grid| self.clear_stockpile(grid))
            .collect()
    }

    pub fn stockpile_entries(&self) -> impl Iterator<Item = (&(i32, i32), &Entity)> {
        self.stockpiles.iter()
    }
}
