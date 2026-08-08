use bevy::prelude::*;
use hw_jobs::{
    AssignedTask, DeconstructData, DeconstructPhase, DeconstructionCommitRequest, WorkType,
    supports_basic_deconstruction_cleanup,
};

use super::common::{NavOutcome, is_near_target, navigate_to_pos};
use super::context::{TaskExecutionContext, TaskHandlerControl};

const DECONSTRUCTION_PROGRESS_PER_SECOND: f32 = 0.33;

#[derive(Debug, Clone, Copy)]
struct LiveDeconstructionTarget {
    position: Vec2,
}

fn validate_live_target(
    ctx: &mut TaskExecutionContext<'_, '_, '_>,
    data: &DeconstructData,
) -> Result<LiveDeconstructionTarget, &'static str> {
    let Ok((_, _, designation, _, _, _, _, _)) =
        ctx.queries.designation.designations.get(data.order)
    else {
        return Err("deconstruction order disappeared");
    };
    if designation.work_type != WorkType::Deconstruct {
        return Err("deconstruction order is malformed");
    }
    let order_target = ctx
        .queries
        .deconstruction_order_targets
        .get(data.order)
        .map_err(|_| "deconstruction order target disappeared")?;
    if order_target.0 != data.target {
        return Err("deconstruction order target changed");
    }
    let pending = ctx
        .queries
        .deconstruction_pending
        .get(data.target)
        .map_err(|_| "deconstruction pending gate disappeared")?;
    if pending.order != data.order {
        return Err("deconstruction pending gate belongs to another order");
    }
    if ctx.queries.deconstruction_claims.get(data.target).is_ok() {
        return Err("deconstruction target is already being committed");
    }
    if ctx.queries.move_planned.get(data.target).is_ok() {
        return Err("deconstruction target is moving");
    }
    if ctx
        .queries
        .deconstruction_blockers
        .get(data.order)
        .is_ok_and(|blocker| blocker.active)
    {
        return Err("deconstruction order is blocked");
    }
    let Ok((transform, building, provisional_wall)) =
        ctx.queries.storage.buildings.get_mut(data.target)
    else {
        return Err("deconstruction target disappeared");
    };
    if building.is_provisional || provisional_wall.is_some() {
        return Err("deconstruction target is provisional");
    }
    if !supports_basic_deconstruction_cleanup(building.kind) {
        return Err("deconstruction target cleanup is not supported by M2");
    }

    Ok(LiveDeconstructionTarget {
        position: transform.translation.truncate(),
    })
}

pub fn handle_deconstruct_task(
    ctx: &mut TaskExecutionContext<'_, '_, '_>,
    data: DeconstructData,
    commands: &mut Commands,
) -> TaskHandlerControl {
    if matches!(data.phase, DeconstructPhase::AwaitingCommit) {
        return TaskHandlerControl::Continue;
    }

    let target = match validate_live_target(ctx, &data) {
        Ok(target) => target,
        Err(reason) => return ctx.abort_retryable(commands, reason),
    };
    let soul_pos = ctx.soul_pos();

    match data.phase {
        DeconstructPhase::GoingToTarget => {
            match navigate_to_pos(ctx, target.position, soul_pos, ctx.env.world_map) {
                NavOutcome::Moving => {}
                NavOutcome::Arrived => {
                    ctx.path.waypoints.clear();
                    *ctx.task = AssignedTask::Deconstruct(DeconstructData {
                        phase: DeconstructPhase::Dismantling { progress: 0.0 },
                        ..data
                    });
                }
                NavOutcome::Deferred => return TaskHandlerControl::Continue,
                NavOutcome::Unreachable => {
                    return ctx.abort_retryable(commands, "deconstruction target unreachable");
                }
                NavOutcome::Ended(control) => return control,
            }
        }
        DeconstructPhase::Dismantling { progress } => {
            if !is_near_target(soul_pos, target.position) {
                *ctx.task = AssignedTask::Deconstruct(DeconstructData {
                    phase: DeconstructPhase::GoingToTarget,
                    ..data
                });
                return TaskHandlerControl::Continue;
            }

            let next_progress =
                progress + ctx.env.time.delta_secs() * DECONSTRUCTION_PROGRESS_PER_SECOND;
            if next_progress < 1.0 {
                *ctx.task = AssignedTask::Deconstruct(DeconstructData {
                    phase: DeconstructPhase::Dismantling {
                        progress: next_progress,
                    },
                    ..data
                });
            } else {
                ctx.queue_deconstruction_commit(DeconstructionCommitRequest {
                    world_epoch: ctx.world_epoch,
                    worker: ctx.soul_entity,
                    identity: ctx.task_identity(),
                    order: data.order,
                    target: data.target,
                });
                *ctx.task = AssignedTask::Deconstruct(DeconstructData {
                    phase: DeconstructPhase::AwaitingCommit,
                    ..data
                });
            }
        }
        DeconstructPhase::AwaitingCommit => unreachable!("handled before target validation"),
    }

    TaskHandlerControl::Continue
}
