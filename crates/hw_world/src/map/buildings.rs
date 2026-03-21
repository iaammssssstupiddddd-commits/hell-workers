use super::WorldMap;
use bevy::prelude::*;
use hw_core::world::DoorState;
use hw_jobs::BuildingType;

impl WorldMap {
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

    pub fn clear_building_occupancy_if_owned(&mut self, grid: (i32, i32), entity: Entity) -> bool {
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
}
