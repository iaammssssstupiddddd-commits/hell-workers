use super::common::*;
use super::context::{TaskExecutionContext, TaskHandlerControl};
use super::types::{AssignedTask, CollectBoneData, CollectBonePhase};

use bevy::prelude::*;
use hw_core::constants::*;
use hw_logistics::{ResourceItem, ResourceType};

pub fn handle_collect_bone_task(
    ctx: &mut TaskExecutionContext,
    data: CollectBoneData,
    commands: &mut Commands,
) -> TaskHandlerControl {
    let CollectBoneData { target, phase } = data;
    let soul_pos = ctx.soul_pos();
    let q_targets = &ctx.queries.designation.targets;

    match phase {
        CollectBonePhase::GoingToBone => {
            let (res_pos, res_translation, has_designation) = {
                let Ok((res_transform, _, _, _, _, des_opt, _)) =
                    ctx.queries.designation.targets.get(target)
                else {
                    return cleanup_collect_target(ctx, target, commands);
                };
                (
                    res_transform.translation.truncate(),
                    res_transform.translation,
                    des_opt.is_some(),
                )
            };
            match navigate_to_adjacent(
                ctx,
                has_designation,
                res_pos,
                soul_pos,
                ctx.env.world_map,
                commands,
            ) {
                NavOutcome::Moving => {}
                NavOutcome::Ended(control) => return control,
                NavOutcome::Deferred => return TaskHandlerControl::Continue,
                NavOutcome::Unreachable => {
                    debug!(
                        "COLLECT_BONE: Soul {:?} cannot reach BonePile {:?}, canceling",
                        ctx.soul_entity, target
                    );
                    return cleanup_collect_target(ctx, target, commands);
                }
                NavOutcome::Arrived => {
                    complete_collect_bone_now(ctx, target, res_translation, commands);
                    ctx.path.waypoints.clear();
                }
            }
        }
        CollectBonePhase::Collecting { .. } => {
            let Ok(target_data) = q_targets.get(target) else {
                return cleanup_collect_target(ctx, target, commands);
            };
            let (res_transform, _, _, _, _, des_opt, _) = target_data;
            if des_opt.is_none() {
                return ctx.abort_closed(commands, "designation missing");
            }
            complete_collect_bone_now(ctx, target, res_transform.translation, commands);
        }
        CollectBonePhase::Done => {
            return finalize_collect_task(ctx, target, commands);
        }
    }

    TaskHandlerControl::Continue
}

fn complete_collect_bone_now(
    ctx: &mut TaskExecutionContext,
    target: Entity,
    source_translation: Vec3,
    commands: &mut Commands,
) {
    // Bone をドロップ
    for i in 0..BONE_DROP_AMOUNT {
        let offset = Vec3::new((i as f32) * 4.0, 0.0, 0.0);
        commands.spawn((
            ResourceItem(ResourceType::Bone),
            Sprite {
                image: ctx.env.soul_handles.icon_bone_small.clone(),
                custom_size: Some(Vec2::splat(TILE_SIZE * 0.5)),
                ..default()
            },
            Transform::from_translation(
                source_translation.truncate().extend(Z_ITEM_PICKUP) + offset,
            ),
            Name::new("Item (Bone)"),
        ));
    }

    debug!(
        "TASK_EXEC: Soul {:?} collected bone instantly",
        ctx.soul_entity
    );

    *ctx.task = AssignedTask::CollectBone(
        crate::soul_ai::execute::task_execution::types::CollectBoneData {
            target,
            phase: CollectBonePhase::Done,
        },
    );
    ctx.soul.fatigue = (ctx.soul.fatigue + FATIGUE_GAIN_ON_COMPLETION).min(1.0);
}
