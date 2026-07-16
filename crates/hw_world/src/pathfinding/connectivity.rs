//! Versioned reachability cache for boolean pathfinding queries.
//!
//! This cache deliberately answers only whether a route exists. Waypoint
//! generation continues to use A* so route cost and door penalties remain
//! outside this component-level view of walkability.

use super::core::{PATHFINDING_DIRECTIONS, can_cross_diagonal_move};
use crate::map::WorldMap;
use bevy::prelude::Resource;
use hw_core::GridPos;

const UNASSIGNED_COMPONENT: u32 = u32::MAX;

/// Dense connected-component IDs for the current [`WorldMap`] topology.
///
/// `WorldMap::obstacle_version` advances only when the final `is_walkable`
/// topology changes. Therefore an Open/Closed door cost change can reuse this
/// cache, while a Locked transition rebuilds it once before the next boolean
/// reachability query.
#[derive(Resource, Default)]
pub struct WalkabilityConnectivityCache {
    obstacle_version: Option<u64>,
    component_ids: Vec<u32>,
    flood_fill_queue: Vec<usize>,
}

impl WalkabilityConnectivityCache {
    /// Returns whether `start` can reach `target` under the existing boolean
    /// pathfinding contract.
    ///
    /// A walkable target must be in the same connected component as `start`.
    /// For a blocked target, reaching any valid adjacent goal is sufficient,
    /// which preserves `find_path_to_adjacent` semantics for task targets.
    pub fn can_reach_target(
        &mut self,
        world_map: &WorldMap,
        start: GridPos,
        target: GridPos,
        target_walkable: bool,
    ) -> bool {
        if world_map.pos_to_idx(start.0, start.1).is_none()
            || world_map.pos_to_idx(target.0, target.1).is_none()
        {
            return false;
        }
        // `can_reach_target` historically treats an already-occupied goal as
        // reachable, even when its tile is blocked. Keep that zero-step case
        // before looking up a walkable component.
        if start == target {
            return true;
        }

        self.ensure_current(world_map);
        let Some(start_component) = self.component_at(world_map, start) else {
            return false;
        };

        if target_walkable && world_map.is_walkable(target.0, target.1) {
            return self.component_at(world_map, target) == Some(start_component);
        }

        self.has_reachable_adjacent_goal(world_map, start_component, target)
    }

    fn ensure_current(&mut self, world_map: &WorldMap) {
        if self.obstacle_version == Some(world_map.obstacle_version)
            && self.component_ids.len() == world_map.tiles.len()
        {
            return;
        }

        self.rebuild(world_map);
    }

    fn rebuild(&mut self, world_map: &WorldMap) {
        self.component_ids
            .resize(world_map.tiles.len(), UNASSIGNED_COMPONENT);
        self.component_ids.fill(UNASSIGNED_COMPONENT);
        self.flood_fill_queue.clear();

        let mut next_component = 0;
        for start_idx in 0..self.component_ids.len() {
            if self.component_ids[start_idx] != UNASSIGNED_COMPONENT {
                continue;
            }

            let start = WorldMap::idx_to_pos(start_idx);
            if !world_map.is_walkable(start.0, start.1) {
                continue;
            }

            self.component_ids[start_idx] = next_component;
            self.flood_fill_queue.push(start_idx);

            let mut queue_head = 0;
            while let Some(&current_idx) = self.flood_fill_queue.get(queue_head) {
                queue_head += 1;
                let current = WorldMap::idx_to_pos(current_idx);

                for (dx, dy) in PATHFINDING_DIRECTIONS {
                    let next = (current.0 + dx, current.1 + dy);
                    let Some(next_idx) = world_map.pos_to_idx(next.0, next.1) else {
                        continue;
                    };
                    if self.component_ids[next_idx] != UNASSIGNED_COMPONENT
                        || !world_map.is_walkable(next.0, next.1)
                        || !can_cross_diagonal_move(world_map, current, next)
                    {
                        continue;
                    }

                    self.component_ids[next_idx] = next_component;
                    self.flood_fill_queue.push(next_idx);
                }
            }

            self.flood_fill_queue.clear();
            next_component += 1;
        }

        self.obstacle_version = Some(world_map.obstacle_version);
    }

    fn component_at(&self, world_map: &WorldMap, pos: GridPos) -> Option<u32> {
        let idx = world_map.pos_to_idx(pos.0, pos.1)?;
        let component = *self.component_ids.get(idx)?;
        (component != UNASSIGNED_COMPONENT).then_some(component)
    }

    fn has_reachable_adjacent_goal(
        &self,
        world_map: &WorldMap,
        start_component: u32,
        target: GridPos,
    ) -> bool {
        for (dx, dy) in PATHFINDING_DIRECTIONS {
            let adjacent = (target.0 + dx, target.1 + dy);
            if !world_map.is_walkable(adjacent.0, adjacent.1)
                || !can_cross_diagonal_move(world_map, target, adjacent)
            {
                continue;
            }

            if self.component_at(world_map, adjacent) == Some(start_component) {
                return true;
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PathfindingContext;
    use crate::pathfinding::can_reach_target;
    use bevy::prelude::Entity;
    use hw_core::constants::MAP_HEIGHT;
    use hw_core::world::DoorState;

    fn assert_parity(map: &WorldMap, cases: &[(GridPos, GridPos, bool)]) {
        let mut cache = WalkabilityConnectivityCache::default();
        let mut context = PathfindingContext::default();

        for &(start, target, target_walkable) in cases {
            assert_eq!(
                cache.can_reach_target(map, start, target, target_walkable),
                can_reach_target(map, &mut context, start, target, target_walkable),
                "cache/A* mismatch for start={start:?}, target={target:?}, target_walkable={target_walkable}",
            );
        }
    }

    #[test]
    fn matches_a_star_for_open_map_and_blocked_endpoint() {
        let mut map = WorldMap::default();
        assert_parity(
            &map,
            &[
                ((10, 10), (80, 80), true),
                ((10, 10), (-1, 10), false),
                ((10, 10), (10, 10), false),
            ],
        );

        map.add_grid_obstacle((20, 20));
        assert_parity(
            &map,
            &[
                ((10, 10), (20, 20), false),
                ((10, 10), (20, 20), true),
                ((20, 20), (20, 20), false),
            ],
        );
    }

    #[test]
    fn matches_a_star_when_diagonal_corner_cutting_is_blocked() {
        let mut map = WorldMap::default();
        let start = (10, 10);
        let target = (11, 11);

        for offset in [(-1, -1), (-1, 0), (-1, 1), (0, -1), (0, 1), (1, -1), (1, 0)] {
            map.add_grid_obstacle((start.0 + offset.0, start.1 + offset.1));
        }

        assert_parity(&map, &[(start, target, true)]);
        let mut cache = WalkabilityConnectivityCache::default();
        assert!(!cache.can_reach_target(&map, start, target, true));
    }

    #[test]
    fn door_cost_changes_reuse_the_cache_but_locked_topology_rebuilds_it() {
        let mut map = WorldMap::default();
        let door = (50, 50);
        for y in 0..MAP_HEIGHT {
            map.add_grid_obstacle((door.0, y));
        }
        map.register_door(door, Entity::PLACEHOLDER, DoorState::Closed);

        let start = (25, 50);
        let target = (75, 50);
        let mut cache = WalkabilityConnectivityCache::default();
        assert!(cache.can_reach_target(&map, start, target, true));
        let closed_version = cache.obstacle_version;

        map.sync_door_passability(door, DoorState::Open);
        assert_parity(&map, &[(start, target, true)]);
        assert!(cache.can_reach_target(&map, start, target, true));
        assert_eq!(cache.obstacle_version, closed_version);

        map.sync_door_passability(door, DoorState::Locked);
        assert_parity(&map, &[(start, target, true)]);
        assert!(!cache.can_reach_target(&map, start, target, true));
        assert_eq!(cache.obstacle_version, Some(map.obstacle_version));

        map.sync_door_passability(door, DoorState::Closed);
        assert_parity(&map, &[(start, target, true)]);
        assert!(cache.can_reach_target(&map, start, target, true));
    }

    #[test]
    fn reset_is_required_when_a_loaded_map_reuses_an_obstacle_version() {
        let mut before_load = WorldMap::default();
        for y in 0..MAP_HEIGHT {
            before_load.add_grid_obstacle((50, y));
        }

        let start = (25, 50);
        let target = (75, 50);
        let mut stale_cache = WalkabilityConnectivityCache::default();
        assert!(!stale_cache.can_reach_target(&before_load, start, target, true));

        let loaded_map = WorldMap {
            obstacle_version: before_load.obstacle_version,
            ..Default::default()
        };
        assert!(!stale_cache.can_reach_target(&loaded_map, start, target, true));

        let mut reset_cache = WalkabilityConnectivityCache::default();
        assert!(reset_cache.can_reach_target(&loaded_map, start, target, true));
    }
}
