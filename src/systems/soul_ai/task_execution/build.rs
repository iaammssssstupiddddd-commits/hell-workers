//! 建築タスクの実行処理

use crate::relationships::WorkingOn;
use crate::systems::soul_ai::task_execution::{
    common::*,
    context::TaskExecutionContext,
    types::{AssignedTask, BuildPhase},
};
use crate::world::map::WorldMap;
use bevy::prelude::*;

pub fn handle_build_task(
    ctx: &mut TaskExecutionContext,
    blueprint_entity: Entity,
    phase: BuildPhase,
    commands: &mut Commands,
    time: &Res<Time>,
    world_map: &Res<WorldMap>,
) {
    let soul_pos = ctx.soul_pos();
    let q_blueprints = &mut ctx.queries.blueprints;

    match phase {
        BuildPhase::GoingToBlueprint => {
            if let Ok((_bp_transform, bp, des_opt)) = q_blueprints.get(blueprint_entity) {
                // 指定が解除されていたら中止
                if cancel_task_if_designation_missing(des_opt, ctx.task, ctx.path) {
                    commands.entity(ctx.soul_entity).remove::<WorkingOn>();
                    return;
                }

                // 資材が揃っていない場合は中止（資材運搬は別タスク）
                if !bp.materials_complete() {
                    info!(
                        "BUILD: Soul {:?} waiting for materials at blueprint {:?}",
                        ctx.soul_entity, blueprint_entity
                    );
                    clear_task_and_path(ctx.task, ctx.path);
                    commands.entity(ctx.soul_entity).remove::<WorkingOn>();
                    return;
                }

                update_destination_to_blueprint(ctx.dest, &bp.occupied_grids, ctx.path, soul_pos, world_map, ctx.pf_context);

                if is_near_blueprint(soul_pos, &bp.occupied_grids) {
                    *ctx.task = AssignedTask::Build(crate::systems::soul_ai::task_execution::types::BuildData {
                        blueprint: blueprint_entity,
                        phase: BuildPhase::Building { progress: 0.0 },
                    });
                    ctx.path.waypoints.clear();
                    info!(
                        "BUILD: Soul {:?} started building at {:?}",
                        ctx.soul_entity, blueprint_entity
                    );
                }
            } else {
                // 設計図が消失
                clear_task_and_path(ctx.task, ctx.path);
                commands.entity(ctx.soul_entity).remove::<WorkingOn>();
            }
        }
        BuildPhase::Building { mut progress } => {
            if let Ok((_, mut bp, des_opt)) = q_blueprints.get_mut(blueprint_entity) {
                // 指定が解除されていたら中止
                if cancel_task_if_designation_missing(des_opt, ctx.task, ctx.path) {
                    commands.entity(ctx.soul_entity).remove::<WorkingOn>();
                    return;
                }

                // 進捗を更新（3秒で完了）
                progress += time.delta_secs() * 0.33;
                bp.progress = progress;

                if progress >= 1.0 {
                    *ctx.task = AssignedTask::Build(crate::systems::soul_ai::task_execution::types::BuildData {
                        blueprint: blueprint_entity,
                        phase: BuildPhase::Done,
                    });
                    ctx.soul.fatigue = (ctx.soul.fatigue + 0.15).min(1.0);
                    info!(
                        "BUILD: Soul {:?} completed building {:?}",
                        ctx.soul_entity, blueprint_entity
                    );
                } else {
                    *ctx.task = AssignedTask::Build(crate::systems::soul_ai::task_execution::types::BuildData {
                        blueprint: blueprint_entity,
                        phase: BuildPhase::Building { progress },
                    });
                }
            } else {
                clear_task_and_path(ctx.task, ctx.path);
                commands.entity(ctx.soul_entity).remove::<WorkingOn>();
            }
        }
        BuildPhase::Done => {
            commands.entity(ctx.soul_entity).remove::<WorkingOn>();
            clear_task_and_path(ctx.task, ctx.path);
        }
    }
}
