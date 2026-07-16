//! 建築タスクの実行処理

use crate::soul_ai::execute::task_execution::{
    common::*,
    context::{TaskExecutionContext, TaskHandlerControl},
    types::{AssignedTask, BuildData, BuildPhase},
};
use bevy::prelude::*;

pub fn handle_build_task(
    ctx: &mut TaskExecutionContext,
    data: BuildData,
    commands: &mut Commands,
) -> TaskHandlerControl {
    let BuildData { blueprint, phase } = data;
    let blueprint_entity = blueprint;
    let soul_pos = ctx.soul_pos();

    match phase {
        BuildPhase::GoingToBlueprint => {
            if let Ok((_bp_transform, bp, des_opt)) =
                ctx.queries.storage.blueprints.get(blueprint_entity)
            {
                if des_opt.is_none() {
                    return ctx.abort_closed(commands, "designation missing");
                }

                if !bp.materials_complete() {
                    debug!(
                        "BUILD: Soul {:?} waiting for materials at blueprint {:?}",
                        ctx.soul_entity, blueprint_entity
                    );
                    return ctx.abort_retryable(commands, "build waiting for materials");
                }

                match update_destination_to_blueprint(
                    &mut ctx.dest,
                    &bp.occupied_grids,
                    &mut ctx.path,
                    soul_pos,
                    ctx.env.world_map,
                    ctx.pf_context,
                    ctx.path_budget,
                ) {
                    PathSearchResult::Found(()) => {}
                    PathSearchResult::Deferred => return TaskHandlerControl::Continue,
                    PathSearchResult::Unreachable => {
                        debug!(
                            "BUILD: Soul {:?} cannot reach blueprint {:?}, canceling",
                            ctx.soul_entity, blueprint_entity
                        );
                        return ctx.abort_retryable(commands, "build blueprint unreachable");
                    }
                }

                if is_near_blueprint(soul_pos, &bp.occupied_grids) {
                    *ctx.task = AssignedTask::Build(BuildData {
                        blueprint: blueprint_entity,
                        phase: BuildPhase::Building { progress: 0.0 },
                    });
                    ctx.path.waypoints.clear();
                    debug!(
                        "BUILD: Soul {:?} started building at {:?}",
                        ctx.soul_entity, blueprint_entity
                    );
                }
            } else {
                return ctx.abort_closed(commands, "build blueprint gone");
            }
        }
        BuildPhase::Building { mut progress } => {
            if let Ok((_, mut bp, des_opt)) =
                ctx.queries.storage.blueprints.get_mut(blueprint_entity)
            {
                if des_opt.is_none() {
                    return ctx.abort_closed(commands, "designation missing");
                }

                if !is_near_blueprint(soul_pos, &bp.occupied_grids) {
                    *ctx.task = AssignedTask::Build(BuildData {
                        blueprint: blueprint_entity,
                        phase: BuildPhase::GoingToBlueprint,
                    });
                    return TaskHandlerControl::Continue;
                }

                progress += ctx.env.time.delta_secs() * 0.33;
                bp.progress = progress;

                if progress >= 1.0 {
                    ctx.soul.fatigue = (ctx.soul.fatigue + 0.15).min(1.0);
                    debug!(
                        "BUILD: Soul {:?} completed building {:?}",
                        ctx.soul_entity, blueprint_entity
                    );
                    ctx.queue_reservation(hw_core::events::ResourceReservationOp::ReleaseSource {
                        source: blueprint_entity,
                        amount: 1,
                    });
                    return ctx.complete_task(commands, "build done");
                } else {
                    *ctx.task = AssignedTask::Build(BuildData {
                        blueprint: blueprint_entity,
                        phase: BuildPhase::Building { progress },
                    });
                }
            } else {
                return ctx.abort_closed(commands, "build blueprint gone during build");
            }
        }
        BuildPhase::Done => {
            // This phase can survive only in an older save or a deferred
            // transition. Never publish completion for a vanished blueprint.
            if ctx
                .queries
                .storage
                .blueprints
                .get(blueprint_entity)
                .is_err()
            {
                return ctx.abort_closed(commands, "build blueprint gone before completion");
            }
            ctx.queue_reservation(hw_core::events::ResourceReservationOp::ReleaseSource {
                source: blueprint_entity,
                amount: 1,
            });
            return ctx.complete_task(commands, "build done");
        }
    }

    TaskHandlerControl::Continue
}
