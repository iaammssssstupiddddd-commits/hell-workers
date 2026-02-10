//! river/tank 向けパス設定

use crate::systems::soul_ai::execute::task_execution::context::TaskExecutionContext;
use crate::systems::soul_ai::execute::task_execution::types::{
    AssignedTask, GatherWaterData, GatherWaterPhase,
};
use crate::world::map::WorldMap;
use bevy::prelude::*;

fn assigned_task(
    bucket: Entity,
    tank: Entity,
    phase: GatherWaterPhase,
) -> AssignedTask {
    AssignedTask::GatherWater(GatherWaterData { bucket, tank, phase })
}

/// 川グリッドへの経路を設定する。成功時は Some(())、失敗時は None。
pub fn set_path_to_river(
    ctx: &mut TaskExecutionContext,
    world_map: &WorldMap,
    bucket_entity: Entity,
    tank_entity: Entity,
) -> Option<()> {
    let river_grid = world_map.get_nearest_river_grid(ctx.soul_transform.translation.truncate())?;
    let path = crate::world::pathfinding::find_path_to_adjacent(
        world_map,
        ctx.pf_context,
        WorldMap::world_to_grid(ctx.soul_transform.translation.truncate()),
        river_grid,
    )?;

    *ctx.task = assigned_task(bucket_entity, tank_entity, GatherWaterPhase::GoingToRiver);

    if let Some(last_grid) = path.last() {
        ctx.dest.0 = WorldMap::grid_to_world(last_grid.0, last_grid.1);
    } else {
        ctx.dest.0 = ctx.soul_transform.translation.truncate();
    }

    ctx.path.waypoints = path
        .iter()
        .map(|&(x, y)| WorldMap::grid_to_world(x, y))
        .collect();
    ctx.path.current_index = 0;
    Some(())
}

/// タンク境界への経路を設定する。成功時は Some(())、失敗時は None。
pub fn set_path_to_tank_boundary(
    ctx: &mut TaskExecutionContext,
    world_map: &WorldMap,
    tank_pos: Vec2,
    bucket_entity: Entity,
    tank_entity: Entity,
    next_phase: GatherWaterPhase,
) -> Option<()> {
    let (cx, cy) = WorldMap::world_to_grid(tank_pos);
    let tank_grids = vec![(cx - 1, cy - 1), (cx, cy - 1), (cx - 1, cy), (cx, cy)];

    let path = crate::world::pathfinding::find_path_to_boundary(
        world_map,
        ctx.pf_context,
        WorldMap::world_to_grid(ctx.soul_transform.translation.truncate()),
        &tank_grids,
    )?;

    *ctx.task = assigned_task(bucket_entity, tank_entity, next_phase);

    if let Some(last_grid) = path.last() {
        ctx.dest.0 = WorldMap::grid_to_world(last_grid.0, last_grid.1);
    } else {
        ctx.dest.0 = tank_pos;
    }

    ctx.path.waypoints = path
        .iter()
        .map(|&(x, y)| WorldMap::grid_to_world(x, y))
        .collect();
    ctx.path.current_index = 0;
    Some(())
}

/// 単一グリッド（例: バケツ位置）への経路を設定する。
pub fn set_path_to_grid_boundary(
    ctx: &mut TaskExecutionContext,
    world_map: &WorldMap,
    target_grid: (i32, i32),
    fallback_pos: Vec2,
) -> Option<()> {
    let path = crate::world::pathfinding::find_path_to_boundary(
        world_map,
        ctx.pf_context,
        WorldMap::world_to_grid(ctx.soul_transform.translation.truncate()),
        &[target_grid],
    )?;

    if let Some(last_grid) = path.last() {
        ctx.dest.0 = WorldMap::grid_to_world(last_grid.0, last_grid.1);
    } else {
        ctx.dest.0 = fallback_pos;
    }

    ctx.path.waypoints = path
        .iter()
        .map(|&(x, y)| WorldMap::grid_to_world(x, y))
        .collect();
    ctx.path.current_index = 0;
    Some(())
}
