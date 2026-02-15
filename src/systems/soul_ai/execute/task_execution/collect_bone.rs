use super::common::*;
use super::context::TaskExecutionContext;
use super::types::{AssignedTask, CollectBonePhase};
use crate::assets::GameAssets;
use crate::constants::*;
use crate::systems::logistics::{ResourceItem, ResourceType};
use crate::world::map::WorldMap;
use bevy::prelude::*;

pub fn handle_collect_bone_task(
    ctx: &mut TaskExecutionContext,
    target: Entity,
    phase: CollectBonePhase,
    commands: &mut Commands,
    game_assets: &Res<GameAssets>,
    _time: &Res<Time>,
    world_map: &Res<WorldMap>,
) {
    let soul_pos = ctx.soul_pos();
    let q_targets = &ctx.queries.designation.targets;

    match phase {
        CollectBonePhase::GoingToBone => {
            if let Ok((res_transform, _, _, _, _, des_opt, _)) = q_targets.get(target) {
                // 指定が解除されていたら中止
                if cancel_task_if_designation_missing(des_opt, ctx.task, ctx.path) {
                    return;
                }
                let res_pos = res_transform.translation.truncate();
                let reachable = update_destination_to_adjacent(
                    ctx.dest,
                    res_pos,
                    ctx.path,
                    soul_pos,
                    world_map,
                    ctx.pf_context,
                );

                if !reachable {
                    // 到達不能: タスクをキャンセル
                    info!(
                        "COLLECT_BONE: Soul {:?} cannot reach BonePile {:?}, canceling",
                        ctx.soul_entity, target
                    );
                    commands
                        .entity(target)
                        .remove::<crate::systems::jobs::Designation>();
                    commands
                        .entity(target)
                        .remove::<crate::systems::jobs::TaskSlots>();
                    ctx.queue_reservation(crate::events::ResourceReservationOp::ReleaseSource {
                        source: target,
                        amount: 1,
                    });
                    clear_task_and_path(ctx.task, ctx.path);
                    return;
                }

                if is_near_target(soul_pos, res_pos) {
                    complete_collect_bone_now(ctx, target, res_transform.translation, commands, game_assets);
                    ctx.path.waypoints.clear();
                }
            } else {
                // BonePile が存在しない場合も Designation を削除
                commands
                    .entity(target)
                    .remove::<crate::systems::jobs::Designation>();
                commands
                    .entity(target)
                    .remove::<crate::systems::jobs::TaskSlots>();
                ctx.queue_reservation(crate::events::ResourceReservationOp::ReleaseSource {
                    source: target,
                    amount: 1,
                });
                clear_task_and_path(ctx.task, ctx.path);
            }
        }
        CollectBonePhase::Collecting { .. } => {
            if let Ok(target_data) = q_targets.get(target) {
                let (res_transform, _, _, _, _, des_opt, _) = target_data;
                // 指定が解除されていたら中止
                if cancel_task_if_designation_missing(des_opt, ctx.task, ctx.path) {
                    return;
                }

                complete_collect_bone_now(ctx, target, res_transform.translation, commands, game_assets);
            } else {
                // BonePile が存在しない場合も Designation を削除
                commands
                    .entity(target)
                    .remove::<crate::systems::jobs::Designation>();
                commands
                    .entity(target)
                    .remove::<crate::systems::jobs::TaskSlots>();
                ctx.queue_reservation(crate::events::ResourceReservationOp::ReleaseSource {
                    source: target,
                    amount: 1,
                });
                clear_task_and_path(ctx.task, ctx.path);
            }
        }
        CollectBonePhase::Done => {
            // BonePile の Designation を削除（次回必要なときに再発行される）
            commands
                .entity(target)
                .remove::<crate::systems::jobs::Designation>();
            // 骨の場合もTaskSlotsなどを削除するか？ Sandと同じ挙動にする
            commands
                .entity(target)
                .remove::<crate::systems::jobs::TaskSlots>();
            commands
                .entity(target)
                .remove::<crate::systems::jobs::IssuedBy>();
            ctx.queue_reservation(crate::events::ResourceReservationOp::ReleaseSource {
                source: target,
                amount: 1,
            });
            clear_task_and_path(ctx.task, ctx.path);
        }
    }
}

fn complete_collect_bone_now(
    ctx: &mut TaskExecutionContext,
    target: Entity,
    source_translation: Vec3,
    commands: &mut Commands,
    game_assets: &Res<GameAssets>,
) {
    // Bone をドロップ
    for i in 0..BONE_DROP_AMOUNT {
        let offset = Vec3::new((i as f32) * 4.0, 0.0, 0.0);
        commands.spawn((
            ResourceItem(ResourceType::Bone),
            Sprite {
                image: game_assets.icon_bone_small.clone(),
                custom_size: Some(Vec2::splat(TILE_SIZE * 0.5)),
                ..default()
            },
            Transform::from_translation(source_translation.truncate().extend(Z_ITEM_PICKUP) + offset),
            Name::new("Item (Bone)"),
        ));
    }

    info!("TASK_EXEC: Soul {:?} collected bone instantly", ctx.soul_entity);

    *ctx.task = AssignedTask::CollectBone(
        crate::systems::soul_ai::execute::task_execution::types::CollectBoneData {
            target,
            phase: CollectBonePhase::Done,
        },
    );
    ctx.soul.fatigue = (ctx.soul.fatigue + FATIGUE_GAIN_ON_COMPLETION).min(1.0);
}
