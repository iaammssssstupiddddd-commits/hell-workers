use super::WorldMap;
use bevy::prelude::*;
use hw_core::constants::DOOR_OPEN_COST;
use hw_core::world::DoorState;

impl WorldMap {
    pub fn add_door(&mut self, x: i32, y: i32, door_entity: Entity, state: DoorState) {
        let was_walkable = self.is_walkable(x, y);
        self.doors.insert((x, y), door_entity);
        self.door_states.insert((x, y), state);
        self.set_obstacle_at(x, y, state != DoorState::Open);
        if was_walkable != self.is_walkable(x, y) {
            self.bump_obstacle_version();
        }
    }

    pub fn remove_door(&mut self, x: i32, y: i32) {
        let was_walkable = self.is_walkable(x, y);
        if self.doors.remove(&(x, y)).is_some() {
            self.door_states.remove(&(x, y));
            self.set_obstacle_at(x, y, false);
            if was_walkable != self.is_walkable(x, y) {
                self.bump_obstacle_version();
            }
        }
    }

    pub fn set_door_state(&mut self, x: i32, y: i32, state: DoorState) {
        if self.doors.contains_key(&(x, y)) {
            self.sync_door_passability((x, y), state);
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
        let was_walkable = self.is_walkable(grid.0, grid.1);
        self.set_building(grid, entity);
        self.doors.insert(grid, entity);
        self.door_states.insert(grid, state);
        self.set_obstacle_at(grid.0, grid.1, state != DoorState::Open);
        if was_walkable != self.is_walkable(grid.0, grid.1) {
            self.bump_obstacle_version();
        }
    }

    pub fn sync_door_passability(&mut self, grid: (i32, i32), state: DoorState) {
        if !self.doors.contains_key(&grid) {
            return;
        }

        let was_walkable = self.is_walkable(grid.0, grid.1);
        self.door_states.insert(grid, state);
        self.set_obstacle_at(grid.0, grid.1, state != DoorState::Open);
        if was_walkable != self.is_walkable(grid.0, grid.1) {
            self.bump_obstacle_version();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::WorldMap;
    use bevy::prelude::Entity;
    use hw_core::constants::DOOR_OPEN_COST;
    use hw_core::world::DoorState;

    #[test]
    fn open_and_closed_doors_change_cost_without_changing_topology() {
        let mut map = WorldMap::default();
        let grid = (4, 4);
        map.register_door(grid, Entity::PLACEHOLDER, DoorState::Closed);

        let version = map.obstacle_version;
        assert!(map.is_walkable(grid.0, grid.1));
        assert_eq!(map.get_door_cost(grid.0, grid.1), DOOR_OPEN_COST);

        map.sync_door_passability(grid, DoorState::Open);
        assert_eq!(map.obstacle_version, version);
        assert!(map.is_walkable(grid.0, grid.1));
        assert_eq!(map.get_door_cost(grid.0, grid.1), 0);

        map.sync_door_passability(grid, DoorState::Closed);
        map.sync_door_passability(grid, DoorState::Closed);
        assert_eq!(map.obstacle_version, version);
        assert!(map.is_walkable(grid.0, grid.1));
    }

    #[test]
    fn locked_boundary_changes_topology_once_per_transition() {
        let mut map = WorldMap::default();
        let grid = (5, 5);
        map.register_door(grid, Entity::PLACEHOLDER, DoorState::Closed);

        let version = map.obstacle_version;
        map.sync_door_passability(grid, DoorState::Locked);
        assert_eq!(map.obstacle_version, version + 1);
        assert!(!map.is_walkable(grid.0, grid.1));

        map.sync_door_passability(grid, DoorState::Locked);
        assert_eq!(map.obstacle_version, version + 1);

        map.sync_door_passability(grid, DoorState::Closed);
        assert_eq!(map.obstacle_version, version + 2);
        assert!(map.is_walkable(grid.0, grid.1));
    }
}
