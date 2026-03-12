use super::common::*;
use super::context::TaskExecutionContext;
use super::types::{AssignedTask, CollectSandPhase};

use hw_jobs::WorkType;
use hw_logistics::transport_request::TransportRequestKind;
use hw_logistics::{ResourceItem, ResourceType};
use hw_world::WorldMap;
use bevy::prelude::*;
use hw_core::constants::*;

pub fn handle_collect_sand_task(
    ctx: &mut TaskExecutionContext,
    target: Entity,
    phase: CollectSandPhase,
    commands: &mut Commands,
    soul_handles: &hw_visual::SoulTaskHandles,
    _time: &Res<Time>,
    world_map: &WorldMap,
) {
    let soul_pos = ctx.soul_pos();
    let q_targets = &ctx.queries.designation.targets;

    match phase {
        CollectSandPhase::GoingToSand => {
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
                        "COLLECT_SAND: Soul {:?} cannot reach SandPile {:?}, canceling",
                        ctx.soul_entity, target
                    );
                    commands
                        .entity(target)
                        .remove::<hw_jobs::Designation>();
                    commands
                        .entity(target)
                        .remove::<hw_jobs::TaskSlots>();
                    ctx.queue_reservation(hw_core::events::ResourceReservationOp::ReleaseSource {
                        source: target,
                        amount: 1,
                    });
                    clear_task_and_path(ctx.task, ctx.path);
                    return;
                }

                if is_near_target(soul_pos, res_pos) {
                    complete_collect_sand_now(
                        ctx,
                        target,
                        res_transform.translation,
                        collect_amount_for_target(ctx, target),
                        commands,
                        soul_handles,
                    );
                    ctx.path.waypoints.clear();
                }
            } else {
                // SandPile が存在しない場合も Designation を削除
                commands
                    .entity(target)
                    .remove::<hw_jobs::Designation>();
                commands
                    .entity(target)
                    .remove::<hw_jobs::TaskSlots>();
                ctx.queue_reservation(hw_core::events::ResourceReservationOp::ReleaseSource {
                    source: target,
                    amount: 1,
                });
                clear_task_and_path(ctx.task, ctx.path);
            }
        }
        CollectSandPhase::Collecting { .. } => {
            if let Ok(target_data) = q_targets.get(target) {
                let (res_transform, _, _, _, _, des_opt, _) = target_data;
                // 指定が解除されていたら中止
                if cancel_task_if_designation_missing(des_opt, ctx.task, ctx.path) {
                    return;
                }

                complete_collect_sand_now(
                    ctx,
                    target,
                    res_transform.translation,
                    collect_amount_for_target(ctx, target),
                    commands,
                    soul_handles,
                );
            } else {
                // SandPile が存在しない場合も Designation を削除
                commands
                    .entity(target)
                    .remove::<hw_jobs::Designation>();
                commands
                    .entity(target)
                    .remove::<hw_jobs::TaskSlots>();
                ctx.queue_reservation(hw_core::events::ResourceReservationOp::ReleaseSource {
                    source: target,
                    amount: 1,
                });
                clear_task_and_path(ctx.task, ctx.path);
            }
        }
        CollectSandPhase::Done => {
            // SandPile の Designation を削除（次回必要なときに再発行される）
            commands
                .entity(target)
                .remove::<hw_jobs::Designation>();
            commands
                .entity(target)
                .remove::<hw_jobs::TaskSlots>();
            commands
                .entity(target)
                .remove::<hw_jobs::IssuedBy>();
            ctx.queue_reservation(hw_core::events::ResourceReservationOp::ReleaseSource {
                source: target,
                amount: 1,
            });
            clear_task_and_path(ctx.task, ctx.path);
        }
    }
}

fn complete_collect_sand_now(
    ctx: &mut TaskExecutionContext,
    target: Entity,
    source_translation: Vec3,
    collect_amount: u32,
    commands: &mut Commands,
    soul_handles: &hw_visual::SoulTaskHandles,
) {
    // Sand をドロップ（砂タイル/砂置き場とも無限ソースとして扱う）
    for i in 0..collect_amount {
        let offset = Vec3::new((i as f32) * 4.0, 0.0, 0.0);
        commands.spawn((
            ResourceItem(ResourceType::Sand),
            Sprite {
                image: soul_handles.icon_sand_small.clone(),
                custom_size: Some(Vec2::splat(TILE_SIZE * 0.5)),
                ..default()
            },
            Transform::from_translation(
                source_translation.truncate().extend(Z_ITEM_PICKUP) + offset,
            ),
            Name::new("Item (Sand)"),
        ));
    }

    info!(
        "TASK_EXEC: Soul {:?} collected sand instantly",
        ctx.soul_entity
    );

    *ctx.task = AssignedTask::CollectSand(
        crate::soul_ai::execute::task_execution::types::CollectSandData {
            target,
            phase: CollectSandPhase::Done,
        },
    );
    ctx.soul.fatigue = (ctx.soul.fatigue + FATIGUE_GAIN_ON_COMPLETION).min(1.0);
}

fn collect_amount_for_target(ctx: &TaskExecutionContext, target: Entity) -> u32 {
    let familiar = ctx.queries.designation.designations.iter().find_map(
        |(entity, _, designation, managed_by_opt, _, _, _, _)| {
            if entity == target && designation.work_type == WorkType::CollectSand {
                managed_by_opt.map(|managed_by| managed_by.0)
            } else {
                None
            }
        },
    );

    let Some(familiar) = familiar else {
        return SAND_DROP_AMOUNT.max(1);
    };

    let remaining_sand_demand = ctx
        .queries
        .transport_request_status
        .iter()
        .filter(|(request, _, _, _, _)| {
            request.issued_by == familiar
                && request.resource_type == ResourceType::Sand
                && matches!(
                    request.kind,
                    TransportRequestKind::DeliverToMixerSolid
                        | TransportRequestKind::DeliverToBlueprint
                )
        })
        .map(|(_, demand, _, _, _)| demand.remaining())
        .sum::<u32>();

    remaining_sand_demand.max(SAND_DROP_AMOUNT).max(1)
}
