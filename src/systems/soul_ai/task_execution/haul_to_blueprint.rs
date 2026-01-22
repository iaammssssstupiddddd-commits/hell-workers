//! 設計図への運搬タスクの実行処理

use crate::entities::damned_soul::StressBreakdown;
use crate::relationships::{Holding, WorkingOn};
use crate::systems::familiar_ai::haul_cache::HaulReservationCache;
use crate::systems::jobs::{
    Blueprint, Designation, DesignationCreatedEvent, IssuedBy, TaskSlots, WorkType,
};
use crate::systems::logistics::Stockpile;
use crate::systems::soul_ai::task_execution::{
    common::*,
    context::TaskExecutionContext,
    types::{AssignedTask, HaulToBpPhase},
};
use bevy::prelude::*;

pub fn handle_haul_to_blueprint_task(
    ctx: &mut TaskExecutionContext,
    holding: Option<&Holding>,
    breakdown_opt: Option<&StressBreakdown>,
    item_entity: Entity,
    blueprint_entity: Entity,
    phase: HaulToBpPhase,
    q_targets: &Query<(
        &Transform,
        Option<&crate::systems::jobs::Tree>,
        Option<&crate::systems::jobs::Rock>,
        Option<&crate::systems::logistics::ResourceItem>,
        Option<&Designation>,
        Option<&crate::relationships::StoredIn>,
    )>,
    q_designations: &Query<(
        Entity,
        &Transform,
        &Designation,
        Option<&IssuedBy>,
        Option<&TaskSlots>,
        Option<&crate::relationships::TaskWorkers>,
    )>,
    q_blueprints: &mut Query<(&Transform, &mut Blueprint, Option<&Designation>)>,
    q_stockpiles: &mut Query<(
        &Transform,
        &mut Stockpile,
        Option<&crate::relationships::StoredItems>,
    )>,
    haul_cache: &mut HaulReservationCache,
    commands: &mut Commands,
    ev_created: &mut MessageWriter<DesignationCreatedEvent>,
) {
    // 疲労またはストレス崩壊のチェック
    if ctx.soul.fatigue > 0.95 || breakdown_opt.is_some() {
        info!(
            "HAUL_TO_BP: Cancelled for {:?} - Exhausted or Stress breakdown",
            ctx.soul_entity
        );
        crate::systems::soul_ai::work::unassign_task(
            commands,
            ctx.soul_entity,
            ctx.soul_pos(),
            ctx.task,
            ctx.path,
            holding,
            q_designations,
            haul_cache,
            Some(ev_created),
            true, // 失敗時はセリフを出す
        );
        return;
    }

    let soul_pos = ctx.soul_pos();

    match phase {
        HaulToBpPhase::GoingToItem => {
            if let Ok((item_transform, _, _, _, des_opt, _)) = q_targets.get(item_entity) {
                // 指示がキャンセルされていないか確認
                if cancel_task_if_designation_missing(des_opt, ctx.task, ctx.path) {
                    info!(
                        "HAUL_TO_BP: Cancelled for {:?} - Designation missing",
                        ctx.soul_entity
                    );
                    commands.entity(ctx.soul_entity).remove::<WorkingOn>();
                    return;
                }

                let item_pos = item_transform.translation.truncate();
                update_destination_if_needed(ctx.dest, item_pos, ctx.path);

                if is_near_target(soul_pos, item_pos) {
                    pickup_item(commands, ctx.soul_entity, item_entity);

                    // もしアイテムが備蓄場所にあったなら、その備蓄場所の型管理を更新する
                    if let Ok((_, _, _, _, _, stored_in_opt)) = q_targets.get(item_entity) {
                        if let Some(stored_in) = stored_in_opt {
                            update_stockpile_on_item_removal(stored_in.0, q_stockpiles);
                        }
                    }

                    // 元のコンポーネントを削除
                    commands
                        .entity(item_entity)
                        .remove::<crate::relationships::StoredIn>();
                    commands.entity(item_entity).remove::<Designation>();
                    commands.entity(item_entity).remove::<IssuedBy>();
                    commands.entity(item_entity).remove::<TaskSlots>();

                    *ctx.task = AssignedTask::HaulToBlueprint {
                        item: item_entity,
                        blueprint: blueprint_entity,
                        phase: HaulToBpPhase::GoingToBlueprint,
                    };
                    ctx.path.waypoints.clear();
                    info!(
                        "HAUL_TO_BP: Soul {:?} picked up item {:?}",
                        ctx.soul_entity, item_entity
                    );
                }
            } else {
                info!(
                    "HAUL_TO_BP: Cancelled for {:?} - Item {:?} gone",
                    ctx.soul_entity, item_entity
                );
                clear_task_and_path(ctx.task, ctx.path);
                commands.entity(ctx.soul_entity).remove::<WorkingOn>();
            }
        }
        HaulToBpPhase::GoingToBlueprint => {
            if let Ok((bp_transform, _, _)) = q_blueprints.get(blueprint_entity) {
                let bp_pos = bp_transform.translation.truncate();
                update_destination_if_needed(ctx.dest, bp_pos, ctx.path);

                if is_near_target(soul_pos, bp_pos) {
                    info!(
                        "HAUL_TO_BP: Soul {:?} arrived at blueprint {:?}",
                        ctx.soul_entity, blueprint_entity
                    );
                    *ctx.task = AssignedTask::HaulToBlueprint {
                        item: item_entity,
                        blueprint: blueprint_entity,
                        phase: HaulToBpPhase::Delivering,
                    };
                    ctx.path.waypoints.clear();
                }
            } else {
                info!(
                    "HAUL_TO_BP: Cancelled for {:?} - Blueprint {:?} gone",
                    ctx.soul_entity, blueprint_entity
                );
                // Blueprint が消失 - アイテムをドロップ
                if holding.is_some() {
                    drop_item(commands, ctx.soul_entity, item_entity, soul_pos);
                }
                commands.entity(ctx.soul_entity).remove::<Holding>();
                commands.entity(ctx.soul_entity).remove::<WorkingOn>();
                clear_task_and_path(ctx.task, ctx.path);
            }
        }
        HaulToBpPhase::Delivering => {
            if let Ok((_, mut bp, _)) = q_blueprints.get_mut(blueprint_entity) {
                // アイテムの資材タイプを取得
                if let Ok((_, _, _, Some(res_item), _, _)) = q_targets.get(item_entity) {
                    let resource_type = res_item.0;

                    // Blueprint に資材を搬入
                    bp.deliver_material(resource_type, 1);
                    info!(
                        "HAUL_TO_BP: Soul {:?} delivered {:?} to blueprint {:?}. Progress: {:?}/{:?}",
                        ctx.soul_entity,
                        resource_type,
                        blueprint_entity,
                        bp.delivered_materials,
                        bp.required_materials
                    );

                    // 資材が揃った場合、BlueprintエンティティのIssuedByを削除して未割り当て状態にする
                    // そして、DesignationCreatedEventを再発行して使い魔が建築タスクを探せるようにする
                    if bp.materials_complete() {
                        if let Ok((_, _, _designation, issued_by_opt, _, _)) =
                            q_designations.get(blueprint_entity)
                        {
                            // IssuedByを削除して未割り当て状態にする
                            if issued_by_opt.is_some() {
                                commands.entity(blueprint_entity).remove::<IssuedBy>();
                            }

                            // DesignationCreatedEventを再発行して使い魔がタスクを探せるようにする
                            ev_created.write(DesignationCreatedEvent {
                                entity: blueprint_entity,
                                work_type: WorkType::Build,
                                issued_by: None, // 未割り当て状態
                                priority: 10,    // 建築タスクは高優先度
                            });

                            info!(
                                "HAUL_TO_BP: Blueprint {:?} materials complete, reissuing DesignationCreatedEvent for build task",
                                blueprint_entity
                            );
                        }
                    }

                    // アイテムを消費
                    commands.entity(item_entity).despawn();
                }
            }

            commands.entity(ctx.soul_entity).remove::<Holding>();
            commands.entity(ctx.soul_entity).remove::<WorkingOn>();
            clear_task_and_path(ctx.task, ctx.path);
            ctx.soul.fatigue = (ctx.soul.fatigue + 0.05).min(1.0);
        }
    }
}
