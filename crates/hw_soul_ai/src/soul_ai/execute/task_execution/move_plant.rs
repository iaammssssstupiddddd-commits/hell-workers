use crate::soul_ai::execute::task_execution::{
    common::{is_near_target_or_dest, update_task_destination_to_adjacent},
    context::{TaskExecutionContext, TaskHandlerControl},
    types::{AssignedTask, MovePlantData, MovePlantPhase},
};
use bevy::prelude::*;
use hw_core::constants::TILE_SIZE;
use hw_jobs::{Building, BuildingType, Designation, PendingBuildingMove};
use hw_world::{PathSearchResult, WorldMap};

mod commit;
pub use commit::apply_pending_building_move_system;
#[cfg(test)]
mod tests;

pub use hw_jobs::MovePlanned;

#[derive(Component, Debug, Clone)]
pub struct MovePlantReservation {
    pub occupied: Vec<(i32, i32)>,
}

pub fn handle_move_plant_task(
    ctx: &mut TaskExecutionContext,
    data: MovePlantData,
    commands: &mut Commands,
) -> TaskHandlerControl {
    match data.phase {
        MovePlantPhase::GoToBuilding => {
            let Ok((building_transform, _, _)) = ctx.queries.storage.buildings.get(data.building)
            else {
                return cleanup_move_task(ctx, commands, data.task_entity, data.building, false);
            };

            let building_pos = building_transform.translation.truncate();
            let soul_pos = ctx.soul_pos();
            match update_task_destination_to_adjacent(ctx, building_pos) {
                PathSearchResult::Found(()) => {}
                PathSearchResult::Deferred => return TaskHandlerControl::Continue,
                PathSearchResult::Unreachable => {
                    return cleanup_move_task(
                        ctx,
                        commands,
                        data.task_entity,
                        data.building,
                        false,
                    );
                }
            }

            if is_near_target_or_dest(soul_pos, building_pos, ctx.dest.0)
                || soul_pos.distance(building_pos) <= TILE_SIZE * 1.5
            {
                *ctx.task = AssignedTask::MovePlant(MovePlantData {
                    phase: MovePlantPhase::Moving,
                    ..data
                });
                ctx.dest.0 = soul_pos;
                ctx.path.waypoints.clear();
                ctx.path.current_index = 0;
            }
        }
        MovePlantPhase::Moving => {
            if let Ok(pending) = ctx.queries.pending_moves.get(data.building)
                && pending.worker == ctx.soul_entity
                && pending.expected_identity == ctx.task_identity()
            {
                return if pending.rejected {
                    cleanup_move_task(ctx, commands, data.task_entity, data.building, false)
                } else {
                    TaskHandlerControl::Continue
                };
            }
            let Ok((building_transform, building, _)) =
                ctx.queries.storage.buildings.get(data.building)
            else {
                return cleanup_move_task(ctx, commands, data.task_entity, data.building, false);
            };

            let old_anchor =
                anchor_grid_for_kind(building.kind, building_transform.translation.truncate());
            let new_anchor = data.destination_grid;
            let old_occupied = occupied_grids_for_kind(building.kind, old_anchor);
            let new_occupied = occupied_grids_for_kind(building.kind, new_anchor);

            let mut next_transform = *building_transform;
            let destination_pos = spawn_pos_for_kind(building.kind, new_anchor);
            next_transform.translation.x = destination_pos.x;
            next_transform.translation.y = destination_pos.y;

            commands.entity(data.building).insert(PendingBuildingMove {
                worker: ctx.soul_entity,
                task_entity: data.task_entity,
                expected_identity: ctx.task_identity(),
                expected_transform: *building_transform,
                proposed_transform: next_transform,
                expected_kind: building.kind,
                old_occupied,
                new_occupied,
                companion_anchor: data.companion_anchor,
                rejected: false,
            });
        }
        MovePlantPhase::Done => {
            return cleanup_move_task(ctx, commands, data.task_entity, data.building, true);
        }
    }

    TaskHandlerControl::Continue
}

fn cleanup_move_task(
    ctx: &mut TaskExecutionContext,
    commands: &mut Commands,
    task_entity: Entity,
    building_entity: Entity,
    completed: bool,
) -> TaskHandlerControl {
    commands.entity(task_entity).remove::<Designation>();
    commands.entity(task_entity).despawn();
    if ctx
        .queries
        .move_planned
        .get(building_entity)
        .is_ok_and(|planned| planned.task_entity == task_entity)
    {
        commands
            .entity(building_entity)
            .remove::<(MovePlanned, PendingBuildingMove)>();
    }
    if completed {
        ctx.complete_task(commands, "move plant done")
    } else {
        ctx.abort_closed(commands, "move plant cancelled")
    }
}

fn is_two_by_two(kind: BuildingType) -> bool {
    matches!(
        kind,
        BuildingType::Tank
            | BuildingType::MudMixer
            | BuildingType::RestArea
            | BuildingType::WheelbarrowParking
    )
}

fn anchor_grid_for_kind(kind: BuildingType, world_pos: Vec2) -> (i32, i32) {
    if is_two_by_two(kind) {
        WorldMap::world_to_grid(world_pos - Vec2::splat(TILE_SIZE * 0.5))
    } else {
        WorldMap::world_to_grid(world_pos)
    }
}

fn spawn_pos_for_kind(kind: BuildingType, anchor_grid: (i32, i32)) -> Vec2 {
    let base = WorldMap::grid_to_world(anchor_grid.0, anchor_grid.1);
    if is_two_by_two(kind) {
        base + Vec2::splat(TILE_SIZE * 0.5)
    } else {
        base
    }
}

fn occupied_grids_for_kind(kind: BuildingType, anchor_grid: (i32, i32)) -> Vec<(i32, i32)> {
    if is_two_by_two(kind) {
        vec![
            anchor_grid,
            (anchor_grid.0 + 1, anchor_grid.1),
            (anchor_grid.0, anchor_grid.1 + 1),
            (anchor_grid.0 + 1, anchor_grid.1 + 1),
        ]
    } else {
        vec![anchor_grid]
    }
}
