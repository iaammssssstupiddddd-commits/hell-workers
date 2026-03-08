use crate::world::map::WorldMap;

pub use hw_world::pathfinding::{PathGoalPolicy, PathNode, PathWorld, PathfindingContext};

impl PathWorld for WorldMap {
    fn pos_to_idx(&self, x: i32, y: i32) -> Option<usize> {
        WorldMap::pos_to_idx(self, x, y)
    }

    fn idx_to_pos(&self, idx: usize) -> (i32, i32) {
        WorldMap::idx_to_pos(idx)
    }

    fn is_walkable(&self, x: i32, y: i32) -> bool {
        WorldMap::is_walkable(self, x, y)
    }

    fn get_door_cost(&self, x: i32, y: i32) -> i32 {
        WorldMap::get_door_cost(self, x, y)
    }
}

pub fn find_path(
    world_map: &WorldMap,
    context: &mut PathfindingContext,
    start: (i32, i32),
    goal: (i32, i32),
    goal_policy: PathGoalPolicy,
) -> Option<Vec<(i32, i32)>> {
    hw_world::pathfinding::find_path(world_map, context, start, goal, goal_policy)
}

pub fn find_path_to_adjacent(
    world_map: &WorldMap,
    context: &mut PathfindingContext,
    start: (i32, i32),
    target: (i32, i32),
    allow_goal_blocked: bool,
) -> Option<Vec<(i32, i32)>> {
    hw_world::pathfinding::find_path_to_adjacent(
        world_map,
        context,
        start,
        target,
        allow_goal_blocked,
    )
}

pub fn find_path_to_boundary(
    world_map: &WorldMap,
    context: &mut PathfindingContext,
    start: (i32, i32),
    target_grids: &[(i32, i32)],
) -> Option<Vec<(i32, i32)>> {
    hw_world::pathfinding::find_path_to_boundary(world_map, context, start, target_grids)
}

pub fn can_reach_target(
    world_map: &WorldMap,
    context: &mut PathfindingContext,
    start: (i32, i32),
    target: (i32, i32),
    target_walkable: bool,
) -> bool {
    hw_world::pathfinding::can_reach_target(world_map, context, start, target, target_walkable)
}
