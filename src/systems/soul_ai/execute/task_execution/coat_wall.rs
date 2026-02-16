//! Provisional wall coating task execution

use crate::constants::FATIGUE_GAIN_ON_COMPLETION;
use crate::relationships::WorkingOn;
use crate::systems::jobs::BuildingType;
use crate::systems::soul_ai::execute::task_execution::{
    common::*,
    context::TaskExecutionContext,
    types::{AssignedTask, CoatWallData, CoatWallPhase},
};
use crate::world::map::WorldMap;
use bevy::prelude::*;

const WALL_COAT_DURATION_SECS: f32 = 2.0;

fn cancel_coat_wall_task(
    ctx: &mut TaskExecutionContext,
    wall_entity: Entity,
    commands: &mut Commands,
    reason: &str,
) {
    info!(
        "COAT_WALL: Cancelled for {:?} - wall {:?} ({})",
        ctx.soul_entity, wall_entity, reason
    );
    ctx.queue_reservation(crate::events::ResourceReservationOp::ReleaseSource {
        source: wall_entity,
        amount: 1,
    });
    clear_task_and_path(ctx.task, ctx.path);
    commands.entity(ctx.soul_entity).remove::<WorkingOn>();
}

pub fn handle_coat_wall_task(
    ctx: &mut TaskExecutionContext,
    wall_entity: Entity,
    phase: CoatWallPhase,
    commands: &mut Commands,
    time: &Res<Time>,
    world_map: &Res<WorldMap>,
) {
    let soul_pos = ctx.soul_pos();

    match phase {
        CoatWallPhase::GoingToWall => {
            let Ok((wall_transform, building, provisional_opt)) =
                ctx.queries.storage.buildings.get_mut(wall_entity)
            else {
                cancel_coat_wall_task(ctx, wall_entity, commands, "wall gone");
                return;
            };

            if building.kind != BuildingType::Wall || !building.is_provisional {
                cancel_coat_wall_task(ctx, wall_entity, commands, "not provisional wall");
                return;
            }

            let Some(provisional) = provisional_opt else {
                cancel_coat_wall_task(ctx, wall_entity, commands, "missing provisional marker");
                return;
            };

            if !provisional.mud_delivered {
                cancel_coat_wall_task(ctx, wall_entity, commands, "mud not delivered");
                return;
            }

            let wall_pos = wall_transform.translation.truncate();
            let reachable = update_destination_to_adjacent(
                ctx.dest,
                wall_pos,
                ctx.path,
                soul_pos,
                world_map,
                ctx.pf_context,
            );
            if !reachable {
                cancel_coat_wall_task(ctx, wall_entity, commands, "wall unreachable");
                return;
            }

            if is_near_target_or_dest(soul_pos, wall_pos, ctx.dest.0) {
                *ctx.task = AssignedTask::CoatWall(CoatWallData {
                    wall: wall_entity,
                    phase: CoatWallPhase::PickingUpMud,
                });
                ctx.path.waypoints.clear();
            }
        }
        CoatWallPhase::PickingUpMud => {
            let Ok((_, building, provisional_opt)) =
                ctx.queries.storage.buildings.get_mut(wall_entity)
            else {
                cancel_coat_wall_task(ctx, wall_entity, commands, "wall gone");
                return;
            };

            if building.kind != BuildingType::Wall || !building.is_provisional {
                cancel_coat_wall_task(ctx, wall_entity, commands, "not provisional wall");
                return;
            }
            let Some(provisional) = provisional_opt else {
                cancel_coat_wall_task(ctx, wall_entity, commands, "missing provisional marker");
                return;
            };
            if !provisional.mud_delivered {
                cancel_coat_wall_task(ctx, wall_entity, commands, "mud not delivered");
                return;
            }

            *ctx.task = AssignedTask::CoatWall(CoatWallData {
                wall: wall_entity,
                phase: CoatWallPhase::Coating { progress_bp: 0 },
            });
            ctx.path.waypoints.clear();
        }
        CoatWallPhase::Coating { progress_bp } => {
            let Ok((_, mut building, provisional_opt)) =
                ctx.queries.storage.buildings.get_mut(wall_entity)
            else {
                cancel_coat_wall_task(ctx, wall_entity, commands, "wall gone");
                return;
            };

            if building.kind != BuildingType::Wall || !building.is_provisional {
                cancel_coat_wall_task(ctx, wall_entity, commands, "not provisional wall");
                return;
            }

            let Some(provisional) = provisional_opt else {
                cancel_coat_wall_task(ctx, wall_entity, commands, "missing provisional marker");
                return;
            };
            if !provisional.mud_delivered {
                cancel_coat_wall_task(ctx, wall_entity, commands, "mud not delivered");
                return;
            }

            const MAX_PROGRESS_BP: u16 = 10_000;
            let delta_bp = ((time.delta_secs() / WALL_COAT_DURATION_SECS * MAX_PROGRESS_BP as f32)
                .round()
                .max(1.0)) as u16;
            let new_progress_bp = progress_bp.saturating_add(delta_bp).min(MAX_PROGRESS_BP);

            if new_progress_bp >= MAX_PROGRESS_BP {
                building.is_provisional = false;
                commands
                    .entity(wall_entity)
                    .remove::<crate::systems::jobs::ProvisionalWall>();
                commands
                    .entity(wall_entity)
                    .remove::<crate::systems::jobs::Designation>()
                    .remove::<crate::systems::jobs::TaskSlots>()
                    .remove::<crate::systems::jobs::Priority>();

                ctx.soul.fatigue = (ctx.soul.fatigue + FATIGUE_GAIN_ON_COMPLETION).min(1.0);
                *ctx.task = AssignedTask::CoatWall(CoatWallData {
                    wall: wall_entity,
                    phase: CoatWallPhase::Done,
                });
            } else {
                *ctx.task = AssignedTask::CoatWall(CoatWallData {
                    wall: wall_entity,
                    phase: CoatWallPhase::Coating {
                        progress_bp: new_progress_bp,
                    },
                });
            }
        }
        CoatWallPhase::Done => {
            ctx.queue_reservation(crate::events::ResourceReservationOp::ReleaseSource {
                source: wall_entity,
                amount: 1,
            });
            commands.entity(ctx.soul_entity).remove::<WorkingOn>();
            clear_task_and_path(ctx.task, ctx.path);
        }
    }
}
