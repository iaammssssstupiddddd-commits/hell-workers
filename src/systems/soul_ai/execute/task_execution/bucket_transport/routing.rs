//! バケツ搬送共通ルーティング

use crate::systems::soul_ai::execute::task_execution::common::update_destination_to_adjacent;
use crate::systems::soul_ai::execute::task_execution::context::TaskExecutionContext;
use crate::systems::soul_ai::execute::task_execution::types::{
    AssignedTask, BucketTransportData, BucketTransportDestination, BucketTransportPhase,
    BucketTransportSource,
};
use crate::world::map::WorldMap;
use bevy::prelude::*;

fn set_task_phase(
    ctx: &mut TaskExecutionContext,
    data: &BucketTransportData,
    new_phase: BucketTransportPhase,
) {
    *ctx.task = AssignedTask::BucketTransport(BucketTransportData {
        phase: new_phase,
        ..data.clone()
    });
}

/// 川グリッドへの経路を設定する。成功時は Some(())、失敗時は None。
pub fn set_path_to_river(
    ctx: &mut TaskExecutionContext,
    world_map: &WorldMap,
    data: &BucketTransportData,
) -> Option<()> {
    let river_grid = world_map.get_nearest_river_grid(ctx.soul_transform.translation.truncate())?;
    let path = crate::world::pathfinding::find_path_to_adjacent(
        world_map,
        ctx.pf_context,
        WorldMap::world_to_grid(ctx.soul_transform.translation.truncate()),
        river_grid,
        true,
    )?;

    set_task_phase(ctx, data, BucketTransportPhase::GoingToSource);

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

/// グリッド境界への経路を設定する（バケツ位置への移動など）。
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

/// タンク境界への経路を設定する。
pub fn set_path_to_tank_boundary(
    ctx: &mut TaskExecutionContext,
    world_map: &WorldMap,
    tank_pos: Vec2,
    data: &BucketTransportData,
    next_phase: BucketTransportPhase,
) -> Option<()> {
    let (cx, cy) = WorldMap::world_to_grid(tank_pos);
    let tank_grids = vec![(cx - 1, cy - 1), (cx, cy - 1), (cx - 1, cy), (cx, cy)];

    let path = crate::world::pathfinding::find_path_to_boundary(
        world_map,
        ctx.pf_context,
        WorldMap::world_to_grid(ctx.soul_transform.translation.truncate()),
        &tank_grids,
    )?;

    set_task_phase(ctx, data, next_phase);

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

/// ソースへの遷移: River→川グリッド, Tank→タンク境界
pub fn transition_to_source(
    commands: &mut Commands,
    ctx: &mut TaskExecutionContext,
    data: &BucketTransportData,
    soul_pos: Vec2,
    world_map: &WorldMap,
) {
    match data.source {
        BucketTransportSource::River => {
            if set_path_to_river(ctx, world_map, data).is_none() {
                super::abort::abort_with_bucket(commands, ctx, data, world_map);
            } else {
                commands
                    .entity(data.bucket)
                    .remove::<crate::relationships::DeliveringTo>();
            }
        }
        BucketTransportSource::Tank { tank, .. } => {
            if let Ok(tank_data) = ctx.queries.storage.stockpiles.get(tank) {
                let (_, tank_transform, _, _) = tank_data;
                let tank_pos = tank_transform.translation.truncate();
                commands
                    .entity(data.bucket)
                    .remove::<crate::relationships::DeliveringTo>();

                let new_data = BucketTransportData {
                    phase: BucketTransportPhase::GoingToSource,
                    ..data.clone()
                };
                *ctx.task = AssignedTask::BucketTransport(new_data);
                ctx.dest.0 = tank_pos;
                ctx.path.waypoints.clear();
            } else {
                let mixer = match data.destination {
                    BucketTransportDestination::Mixer(m) => m,
                    _ => {
                        super::abort::abort_with_bucket(commands, ctx, data, world_map);
                        return;
                    }
                };
                super::abort::abort_and_drop_bucket_mixer(
                    commands,
                    ctx,
                    data.bucket,
                    match data.source {
                        BucketTransportSource::Tank { tank, .. } => tank,
                        _ => data.bucket,
                    },
                    mixer,
                    soul_pos,
                );
            }
        }
    }
}

/// デスティネーションへの遷移: Tank→タンク境界, Mixer→ミキサー隣接
pub fn transition_to_destination(
    commands: &mut Commands,
    ctx: &mut TaskExecutionContext,
    data: &BucketTransportData,
    soul_pos: Vec2,
    world_map: &WorldMap,
) {
    let tank_entity = match data.source {
        BucketTransportSource::Tank { tank, .. } => Some(tank),
        BucketTransportSource::River => None,
    };

    match data.destination {
        BucketTransportDestination::Tank(tank_entity) => {
            if let Ok((tank_transform, _, _, _, _, _, _)) =
                ctx.queries.designation.targets.get(tank_entity)
            {
                let tank_pos = tank_transform.translation.truncate();
                if set_path_to_tank_boundary(
                    ctx,
                    world_map,
                    tank_pos,
                    data,
                    BucketTransportPhase::GoingToDestination,
                )
                .is_some()
                {
                    commands
                        .entity(data.bucket)
                        .try_insert(crate::relationships::DeliveringTo(tank_entity));
                } else {
                    super::abort::abort_with_bucket(commands, ctx, data, world_map);
                }
            } else {
                super::abort::abort_with_bucket(commands, ctx, data, world_map);
            }
        }
        BucketTransportDestination::Mixer(mixer_entity) => {
            if let Ok(mixer_data) = ctx.queries.storage.mixers.get(mixer_entity) {
                let (mixer_transform, _, _) = mixer_data;
                let mixer_pos = mixer_transform.translation.truncate();
                commands
                    .entity(data.bucket)
                    .try_insert(crate::relationships::DeliveringTo(mixer_entity));

                let new_data = BucketTransportData {
                    phase: BucketTransportPhase::GoingToDestination,
                    ..data.clone()
                };
                *ctx.task = AssignedTask::BucketTransport(new_data);
                update_destination_to_adjacent(
                    ctx.dest,
                    mixer_pos,
                    ctx.path,
                    soul_pos,
                    world_map,
                    ctx.pf_context,
                );
            } else {
                let tank = tank_entity.unwrap_or(data.bucket);
                super::abort::abort_and_drop_bucket_mixer(
                    commands,
                    ctx,
                    data.bucket,
                    tank,
                    mixer_entity,
                    soul_pos,
                );
            }
        }
    }
}
