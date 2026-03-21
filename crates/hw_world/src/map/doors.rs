use super::WorldMap;
use bevy::prelude::*;
use hw_core::constants::DOOR_OPEN_COST;
use hw_core::world::DoorState;

impl WorldMap {
    pub fn add_door(&mut self, x: i32, y: i32, door_entity: Entity, state: DoorState) {
        self.doors.insert((x, y), door_entity);
        self.door_states.insert((x, y), state);
    }

    pub fn remove_door(&mut self, x: i32, y: i32) {
        self.doors.remove(&(x, y));
        self.door_states.remove(&(x, y));
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
}
