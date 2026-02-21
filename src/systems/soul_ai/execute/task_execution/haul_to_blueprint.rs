//! 設計図への運搬タスクの実行処理

use crate::entities::damned_soul::StressBreakdown;
use crate::relationships::WorkingOn;
// use crate::systems::familiar_ai::perceive::resource_sync::SharedResourceCache; // Removed unused import
use crate::systems::soul_ai::execute::task_execution::{
    common::*,
    context::TaskExecutionContext,
    transport_common::{cancel, reservation},
    types::{AssignedTask, HaulToBpPhase},
};
use crate::world::map::WorldMap;
use bevy::prelude::*;

pub fn handle_haul_to_blueprint_task(
    ctx: &mut TaskExecutionContext,
    breakdown_opt: Option<&StressBreakdown>,
    item_entity: Entity,
    blueprint_entity: Entity,
    phase: HaulToBpPhase,
    commands: &mut Commands,
    world_map: &Res<WorldMap>,
) {
    let q_targets = &ctx.queries.designation.targets;
    let q_designations = &ctx.queries.designation.designations;
    let soul_pos = ctx.soul_pos();
    let q_blueprints = &mut ctx.queries.storage.blueprints;
    // 疲労またはストレス崩壊のチェック
    if ctx.soul.fatigue > 0.95 || breakdown_opt.is_some() {
        info!(
            "HAUL_TO_BP: Cancelled for {:?} - Exhausted or Stress breakdown",
            ctx.soul_entity
        );
        let soul_pos = ctx.soul_transform.translation.truncate();
        crate::systems::soul_ai::helpers::work::unassign_task(
            commands,
            ctx.soul_entity,
            soul_pos,
            ctx.task,
            ctx.path,
            Some(ctx.inventory),
            None, // アイテムを拾う前なのでNone
            ctx.queries,
            world_map,
            true, // 失敗時はセリフを出す
        );
        return;
    }

    match phase {
        HaulToBpPhase::GoingToItem => {
            if let Ok((item_transform, _, _, _, _, _, stored_in_opt)) = q_targets.get(item_entity) {
                let item_pos = item_transform.translation.truncate();
                let stored_in_entity = stored_in_opt.map(|stored_in| stored_in.0);
                update_destination_to_adjacent(
                    ctx.dest,
                    item_pos,
                    ctx.path,
                    soul_pos,
                    world_map,
                    ctx.pf_context,
                );
                let is_near = can_pickup_item(soul_pos, item_pos);

                if is_near {
                    if !try_pickup_item(
                        commands,
                        ctx.soul_entity,
                        item_entity,
                        ctx.inventory,
                        soul_pos,
                        item_pos,
                        ctx.task,
                        ctx.path,
                    ) {
                        return;
                    }
                    release_mixer_mud_storage_for_item(ctx, item_entity, commands);

                    // もしアイテムが備蓄場所にあったなら、その備蓄場所の型管理を更新する
                    if let Some(stored_in) = stored_in_entity {
                        update_stockpile_on_item_removal(
                            stored_in,
                            &mut ctx.queries.storage.stockpiles,
                        );
                    }

                    // ブループリントへの目的地設定は、次のフレームの GoingToBlueprint フェーズで
                    // update_destination_to_blueprint により自動的に（一貫したロジックで）行われるため、
                    // ここではパスをクリアするのみとする。

                    reservation::record_picked_source(ctx, item_entity, 1);

                    *ctx.task = AssignedTask::HaulToBlueprint(
                        crate::systems::soul_ai::execute::task_execution::types::HaulToBlueprintData {
                            item: item_entity,
                            blueprint: blueprint_entity,
                            phase: HaulToBpPhase::GoingToBlueprint,
                        },
                    );
                    ctx.path.waypoints.clear();
                    info!(
                        "HAUL_TO_BP: Soul {:?} picked up item {:?}, heading to blueprint {:?}",
                        ctx.soul_entity, item_entity, blueprint_entity
                    );
                }
            } else {
                info!(
                    "HAUL_TO_BP: Cancelled for {:?} - Item {:?} gone",
                    ctx.soul_entity, item_entity
                );
                cancel::cancel_haul_to_blueprint(ctx, item_entity, blueprint_entity);
            }
        }
        HaulToBpPhase::GoingToBlueprint => {
            if let Ok((_bp_transform, bp, _)) = q_blueprints.get(blueprint_entity) {
                let reachable = update_destination_to_blueprint(
                    ctx.dest,
                    &bp.occupied_grids,
                    ctx.path,
                    soul_pos,
                    world_map,
                    ctx.pf_context,
                );
                if !reachable {
                    info!(
                        "HAUL_TO_BP: Cancelled for {:?} - Blueprint {:?} unreachable",
                        ctx.soul_entity, blueprint_entity
                    );
                    cancel::cancel_haul_to_blueprint(ctx, item_entity, blueprint_entity);
                    return;
                }

                if is_near_blueprint(soul_pos, &bp.occupied_grids) {
                    info!(
                        "HAUL_TO_BP: Soul {:?} arrived at blueprint {:?}",
                        ctx.soul_entity, blueprint_entity
                    );
                    *ctx.task = AssignedTask::HaulToBlueprint(
                        crate::systems::soul_ai::execute::task_execution::types::HaulToBlueprintData {
                            item: item_entity,
                            blueprint: blueprint_entity,
                            phase: HaulToBpPhase::Delivering,
                        },
                    );
                    ctx.path.waypoints.clear();
                }
            } else {
                info!(
                    "HAUL_TO_BP: Cancelled for {:?} - Blueprint {:?} gone",
                    ctx.soul_entity, blueprint_entity
                );
                // Blueprint が消失 - アイテムを解除して再発行
                let dropped_res = q_targets
                    .get(item_entity)
                    .ok()
                    .and_then(|(_, _, _, _, ri, _, _)| ri.map(|r| r.0));
                crate::systems::soul_ai::helpers::work::unassign_task(
                    commands,
                    ctx.soul_entity,
                    soul_pos,
                    ctx.task,
                    ctx.path,
                    Some(ctx.inventory),
                    dropped_res,
                    ctx.queries,
                    world_map,
                    true,
                );
            }
        }
        HaulToBpPhase::Delivering => {
            if let Ok((_, mut bp, _)) = q_blueprints.get_mut(blueprint_entity) {
                // アイテムの資材タイプを取得
                if let Ok((_, _, _, _, Some(res_item), _, _)) = q_targets.get(item_entity) {
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
                        if let Ok((_, _, _designation, managed_by_opt, _, _, _, _)) =
                            q_designations.get(blueprint_entity)
                        {
                            // ManagedByを削除して未割り当て状態にする
                            if managed_by_opt.is_some() {
                                commands
                                    .entity(blueprint_entity)
                                    .remove::<crate::relationships::ManagedBy>();
                            }

                            // Priority(10) を付与して使い魔がタスクを探せるようにする
                            commands
                                .entity(blueprint_entity)
                                .insert(crate::systems::jobs::Priority(10));

                            info!(
                                "HAUL_TO_BP: Blueprint {:?} materials complete, reissuing DesignationCreatedEvent for build task",
                                blueprint_entity
                            );
                        }
                    }

                    // ====== 修正点: 参照先の Item(task_entity)が despawn される前に WorkingOn 等を外す ======
                    ctx.inventory.0 = None;
                    commands.entity(ctx.soul_entity).remove::<WorkingOn>();
                    clear_task_and_path(ctx.task, ctx.path);

                    // アイテムを消費
                    commands.entity(item_entity).despawn();
                    // DeliveringTo is removed with despawn
                }
            }

            ctx.soul.fatigue = (ctx.soul.fatigue + 0.05).min(1.0);

            reservation::release_destination(ctx, blueprint_entity);
        }
    }
}
