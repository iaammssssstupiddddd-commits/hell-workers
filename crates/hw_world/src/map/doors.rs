use super::WorldMap;
use bevy::prelude::*;
use hw_core::constants::DOOR_OPEN_COST;
use hw_core::world::DoorState;

impl WorldMap {
    pub fn add_door(&mut self, x: i32, y: i32, door_entity: Entity, state: DoorState) {
        self.doors.insert((x, y), door_entity);
        self.door_states.insert((x, y), state);
        self.bump_obstacle_version();
    }

    pub fn remove_door(&mut self, x: i32, y: i32) {
        if self.doors.remove(&(x, y)).is_some() {
            self.door_states.remove(&(x, y));
            self.bump_obstacle_version();
        }
    }

    pub fn set_door_state(&mut self, x: i32, y: i32, state: DoorState) {
        if self.doors.contains_key(&(x, y))
            && self.door_states.get(&(x, y)).copied() != Some(state)
        {
            self.door_states.insert((x, y), state);
            self.bump_obstacle_version();
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
        let obstacle_changed = match state {
            DoorState::Open => self.set_obstacle_at(grid.0, grid.1, false),
            DoorState::Closed | DoorState::Locked => self.set_obstacle_at(grid.0, grid.1, true),
        };
        let state_changed = self.doors.contains_key(&grid)
            && self.door_states.get(&grid).copied() != Some(state);
        if state_changed {
            self.door_states.insert(grid, state);
        }
        if obstacle_changed || state_changed {
            self.bump_obstacle_version();
        }
    }
}
