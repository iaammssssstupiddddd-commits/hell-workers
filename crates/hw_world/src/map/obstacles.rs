use super::WorldMap;
use bevy::prelude::Entity;
use hw_core::world::DoorState;
use std::collections::{HashMap, HashSet};

impl WorldMap {
    /// Returns the raw runtime obstacle bit without terrain or bridge policy.
    pub fn has_raw_obstacle(&self, x: i32, y: i32) -> bool {
        self.pos_to_idx(x, y).is_some_and(|idx| self.obstacles[idx])
    }

    pub(crate) fn set_obstacle_at(&mut self, x: i32, y: i32, blocked: bool) -> bool {
        if let Some(idx) = self.pos_to_idx(x, y)
            && self.obstacles[idx] != blocked
        {
            self.obstacles[idx] = blocked;
            return true;
        }
        false
    }

    pub fn obstacle_count(&self) -> usize {
        self.obstacles.iter().filter(|&&blocked| blocked).count()
    }

    pub fn obstacle_indices(&self) -> impl Iterator<Item = usize> + '_ {
        self.obstacles
            .iter()
            .enumerate()
            .filter_map(|(idx, blocked)| blocked.then_some(idx))
    }

    pub fn add_obstacle(&mut self, x: i32, y: i32) {
        self.set_obstacle_with_topology(x, y, true);
    }

    pub fn remove_obstacle(&mut self, x: i32, y: i32) {
        self.set_obstacle_with_topology(x, y, false);
    }

    pub fn add_grid_obstacle(&mut self, grid: (i32, i32)) {
        self.add_obstacle(grid.0, grid.1);
    }

    pub fn remove_grid_obstacle(&mut self, grid: (i32, i32)) {
        self.remove_obstacle(grid.0, grid.1);
    }

    pub fn add_grid_obstacles<I>(&mut self, grids: I)
    where
        I: IntoIterator<Item = (i32, i32)>,
    {
        for grid in grids {
            self.add_grid_obstacle(grid);
        }
    }

    pub fn reserve_building_footprint_tiles<I>(&mut self, grids: I)
    where
        I: IntoIterator<Item = (i32, i32)>,
    {
        self.add_grid_obstacles(grids);
    }

    /// Replaces the raw obstacle cache from durable semantic blockers.
    ///
    /// Door state is the final override: an open door stores a clear raw bit,
    /// while closed and locked doors retain an obstacle bit for a later door
    /// removal. The pathfinding generation advances at most once and only when
    /// the final walkability topology differs.
    pub fn replace_obstacle_bitmap<I>(&mut self, blockers: I) -> bool
    where
        I: IntoIterator<Item = (i32, i32)>,
    {
        let previous_walkability = self.walkability_snapshot();
        self.replace_raw_obstacle_bitmap(blockers);
        self.apply_door_obstacle_bits();
        self.finish_navigation_cache_rebuild(previous_walkability)
    }

    /// Rebuilds all derived navigation caches after loading durable world state.
    ///
    /// Door and bridge caches participate in `is_walkable`, so they must be
    /// replaced in the same transaction as the raw obstacle bitmap. Comparing
    /// the topology before any cache mutation and after all cache mutations
    /// guarantees at most one generation bump for a load.
    pub fn replace_navigation_caches(
        &mut self,
        blockers: &HashSet<(i32, i32)>,
        doors: &HashMap<(i32, i32), (Entity, DoorState)>,
        bridged_tiles: &HashSet<(i32, i32)>,
    ) -> bool {
        let previous_walkability = self.walkability_snapshot();

        self.doors.clear();
        self.door_states.clear();
        for (&grid, &(entity, state)) in doors {
            self.doors.insert(grid, entity);
            self.door_states.insert(grid, state);
        }
        self.bridged_tiles.clone_from(bridged_tiles);

        self.replace_raw_obstacle_bitmap(blockers.iter().copied());
        self.apply_door_obstacle_bits();
        self.finish_navigation_cache_rebuild(previous_walkability)
    }

    fn walkability_snapshot(&self) -> Vec<bool> {
        self.obstacles
            .iter()
            .enumerate()
            .map(|(idx, _)| {
                let (x, y) = Self::idx_to_pos(idx);
                self.is_walkable(x, y)
            })
            .collect()
    }

    fn replace_raw_obstacle_bitmap<I>(&mut self, blockers: I)
    where
        I: IntoIterator<Item = (i32, i32)>,
    {
        self.obstacles.fill(false);
        for (x, y) in blockers {
            self.set_obstacle_at(x, y, true);
        }
    }

    fn apply_door_obstacle_bits(&mut self) {
        let door_states: Vec<((i32, i32), DoorState)> = self
            .door_states
            .iter()
            .map(|(&grid, &state)| (grid, state))
            .collect();
        for ((x, y), state) in door_states {
            self.set_obstacle_at(x, y, !matches!(state, DoorState::Open));
        }
    }

    fn finish_navigation_cache_rebuild(&mut self, previous_walkability: Vec<bool>) -> bool {
        let topology_changed =
            previous_walkability
                .iter()
                .enumerate()
                .any(|(idx, was_walkable)| {
                    let (x, y) = Self::idx_to_pos(idx);
                    *was_walkable != self.is_walkable(x, y)
                });
        if topology_changed {
            self.bump_obstacle_version();
        }
        topology_changed
    }

    fn set_obstacle_with_topology(&mut self, x: i32, y: i32, blocked: bool) -> bool {
        let was_walkable = self.is_walkable(x, y);
        let changed = self.set_obstacle_at(x, y, blocked);
        if changed && was_walkable != self.is_walkable(x, y) {
            self.bump_obstacle_version();
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::WorldMap;
    use hw_core::world::DoorState;

    #[test]
    fn replacement_changes_topology_once_and_applies_door_override() {
        let mut map = WorldMap::default();
        map.add_grid_obstacle((1, 1));
        map.door_states.insert((2, 2), DoorState::Open);

        let before = map.obstacle_version;
        assert!(map.replace_obstacle_bitmap([(2, 2)]));
        assert_eq!(map.obstacle_version, before + 1);
        assert!(map.is_walkable(1, 1));
        assert!(map.is_walkable(2, 2));
        assert!(!map.obstacles[map.pos_to_idx(2, 2).unwrap()]);

        assert!(!map.replace_obstacle_bitmap([(2, 2)]));
        assert_eq!(map.obstacle_version, before + 1);
    }
}
