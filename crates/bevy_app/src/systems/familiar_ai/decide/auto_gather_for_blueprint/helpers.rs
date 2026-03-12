pub(super) use hw_familiar_ai::familiar_ai::decide::auto_gather_for_blueprint::helpers::{
    OwnerInfo, STAGE_COUNT, SourceCandidate, SupplyBucket, compare_auto_idle_for_cleanup,
    resource_rank, work_type_for_resource,
};

use crate::world::map::WorldMap;
use crate::world::pathfinding::{self, PathfindingContext};
use bevy::prelude::Vec2;

pub(super) fn is_reachable(
    start_grid: (i32, i32),
    target_pos: Vec2,
    world_map: &WorldMap,
    pf_context: &mut PathfindingContext,
) -> bool {
    let target_grid = WorldMap::world_to_grid(target_pos);
    pathfinding::find_path_to_adjacent(world_map, pf_context, start_grid, target_grid, true)
        .is_some()
}
