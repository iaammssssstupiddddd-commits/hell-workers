use super::WorldMap;
use bevy::prelude::*;
use hw_core::constants::DOOR_OPEN_COST;
use hw_core::world::DoorState;

impl WorldMap {
    /// Keep idle souls moving until both the doorway and the edge of its tile are clear.
    pub fn clear_idle_doorway(
        &self,
        position: Vec2,
        dest: &mut hw_core::soul::Destination,
        path: &mut hw_core::soul::Path,
    ) -> bool {
        use hw_core::constants::TILE_SIZE;
        let start = Self::world_to_grid(position);
        let neighbors = |(x, y)| [(x, y + 1), (x, y - 1), (x + 1, y), (x - 1, y)];
        let on_door = self.door_entity(start.0, start.1).is_some();
        if !on_door
            && !neighbors(start).into_iter().any(|(x, y)| {
                self.door_entity(x, y).is_some()
                    && (Self::grid_to_world(x, y) - position).abs().max_element() < TILE_SIZE * 0.85
            })
        {
            return false;
        }
        // Avoid interrupting movement through the doorway to a clear destination.
        if let Some(end) = path
            .waypoints
            .get(path.current_index..)
            .and_then(|steps| steps.last())
        {
            let (x, y) = Self::world_to_grid(*end);
            if self.door_entity(x, y).is_none()
                && self.is_walkable(x, y)
                && ((x, y) != start || *end == Self::grid_to_world(x, y))
            {
                return true;
            }
        }

        let mut exit = start;
        let mut previous = std::collections::HashMap::new();
        if on_door {
            // Adjacent doors may form a passage. Search only that connected group for an exit.
            let mut queue = std::collections::VecDeque::from([start]);
            previous.insert(start, start);
            'search: loop {
                let Some(grid) = queue.pop_front() else {
                    return false;
                };
                for next in neighbors(grid) {
                    if !self.is_walkable(next.0, next.1) || previous.contains_key(&next) {
                        continue;
                    }
                    previous.insert(next, grid);
                    if self.door_entity(next.0, next.1).is_none() {
                        exit = next;
                        break 'search;
                    }
                    queue.push_back(next);
                }
            }
        } else if !self.is_walkable(start.0, start.1) {
            return false;
        }
        dest.0 = Self::grid_to_world(exit.0, exit.1);
        path.waypoints.clear();
        path.waypoints.push(dest.0);
        let mut step = exit;
        while let Some(&parent) = previous.get(&step) {
            if parent == start {
                break;
            }
            path.waypoints.push(Self::grid_to_world(parent.0, parent.1));
            step = parent;
        }
        path.waypoints.reverse();
        path.current_index = 0;
        path.planned_destination = Some(dest.0);
        path.validated_obstacle_version = self.obstacle_version;
        true
    }

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

    /// Removes a completed door from every WorldMap layer only when all
    /// owner-bearing entries still belong to `entity`.
    ///
    /// Deconstruction uses this instead of composing `remove_door` with a
    /// separate building clear: a partially replaced door must remain wholly
    /// untouched rather than clearing another owner's occupancy.
    pub fn clear_door_if_owned(&mut self, grid: (i32, i32), entity: Entity) -> bool {
        if self.doors.get(&grid) != Some(&entity)
            || self.buildings.get(&grid) != Some(&entity)
            || !self.door_states.contains_key(&grid)
        {
            return false;
        }

        let was_walkable = self.is_walkable(grid.0, grid.1);
        self.doors.remove(&grid);
        self.door_states.remove(&grid);
        self.buildings.remove(&grid);
        self.set_obstacle_at(grid.0, grid.1, false);
        if was_walkable != self.is_walkable(grid.0, grid.1) {
            self.bump_obstacle_version();
        }
        true
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
    fn idle_soul_exits_connected_doors_and_keeps_a_path_through_them() {
        use hw_core::soul::{Destination, Path};
        let mut map = WorldMap::default();
        let pos = WorldMap::grid_to_world(12, 12);
        let exit = WorldMap::grid_to_world(14, 12);
        for (x, state) in [(12, DoorState::Open), (13, DoorState::Closed)] {
            map.register_door((x, 12), Entity::from_bits(x as u64), state);
            map.add_obstacle(x, 13);
            map.add_obstacle(x, 11);
        }
        map.add_obstacle(11, 12);
        let mut path = Path::default();
        let mut dest = Destination(pos);
        assert!(map.clear_idle_doorway(pos, &mut dest, &mut path));
        assert_eq!(dest.0, exit);
        assert_eq!(path.waypoints, [WorldMap::grid_to_world(13, 12), exit]);
        path.current_index = 1;
        assert!(map.clear_idle_doorway(WorldMap::grid_to_world(13, 12), &mut dest, &mut path));
        assert_eq!(path.current_index, 1);
        assert!(!map.clear_idle_doorway(exit, &mut dest, &mut path));
        map.set_door_state(13, 12, DoorState::Locked);
        assert!(!map.clear_idle_doorway(pos, &mut dest, &mut Path::default()));
    }

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

    #[test]
    fn owner_safe_clear_is_all_or_none_across_door_layers() {
        let mut map = WorldMap::default();
        let grid = (6, 7);
        let owner = Entity::from_bits(1);
        let replacement = Entity::from_bits(2);
        map.register_door(grid, owner, DoorState::Locked);
        map.set_building(grid, replacement);
        let mismatch_version = map.obstacle_version;

        assert!(!map.clear_door_if_owned(grid, owner));
        assert_eq!(map.door_entity(grid.0, grid.1), Some(owner));
        assert_eq!(map.building_entity(grid), Some(replacement));
        assert_eq!(map.door_state(grid.0, grid.1), Some(DoorState::Locked));
        assert!(map.has_raw_obstacle(grid.0, grid.1));
        assert!(!map.is_walkable(grid.0, grid.1));
        assert_eq!(map.obstacle_version, mismatch_version);

        map.set_building(grid, owner);
        let version = map.obstacle_version;
        assert!(map.clear_door_if_owned(grid, owner));
        assert_eq!(map.door_entity(grid.0, grid.1), None);
        assert_eq!(map.building_entity(grid), None);
        assert_eq!(map.door_state(grid.0, grid.1), None);
        assert_eq!(map.obstacle_version, version + 1);
        assert!(map.is_walkable(grid.0, grid.1));
    }

    #[test]
    fn clearing_a_closed_door_over_river_restores_blocked_terrain() {
        let mut map = WorldMap::default();
        let grid = (7, 8);
        let index = map.pos_to_idx(grid.0, grid.1).unwrap();
        map.tiles[index] = crate::TerrainType::River;
        let owner = Entity::from_bits(3);
        map.register_door(grid, owner, DoorState::Closed);
        assert!(map.is_walkable(grid.0, grid.1));

        let version = map.obstacle_version;
        assert!(map.clear_door_if_owned(grid, owner));

        assert!(!map.is_walkable(grid.0, grid.1));
        assert!(!map.has_raw_obstacle(grid.0, grid.1));
        assert_eq!(map.obstacle_version, version + 1);
    }

    #[test]
    fn clearing_a_closed_door_on_grass_keeps_topology_generation() {
        let mut map = WorldMap::default();
        let grid = (8, 9);
        let owner = Entity::from_bits(4);
        map.register_door(grid, owner, DoorState::Closed);
        let version = map.obstacle_version;

        assert!(map.clear_door_if_owned(grid, owner));

        assert!(map.is_walkable(grid.0, grid.1));
        assert_eq!(map.obstacle_version, version);
    }
}
