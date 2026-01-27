//! 設計図への運搬タスクの実行処理

use crate::entities::damned_soul::StressBreakdown;
use crate::relationships::WorkingOn;
use crate::systems::familiar_ai::haul_cache::HaulReservationCache;
use crate::systems::soul_ai::task_execution::{
    common::*,
    context::TaskExecutionContext,
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
    haul_cache: &mut HaulReservationCache,
    commands: &mut Commands,
    world_map: &Res<WorldMap>,
) {
    let q_targets = &ctx.queries.targets;
    let q_designations = &ctx.queries.designations;
    let soul_pos = ctx.soul_pos();
    let q_blueprints = &mut ctx.queries.blueprints;
    let q_stockpiles = &mut ctx.queries.stockpiles;
    // 疲労またはストレス崩壊のチェック
    if ctx.soul.fatigue > 0.95 || breakdown_opt.is_some() {
        info!(
            "HAUL_TO_BP: Cancelled for {:?} - Exhausted or Stress breakdown",
            ctx.soul_entity
        );
        let soul_pos = ctx.soul_transform.translation.truncate();
        crate::systems::soul_ai::work::unassign_task(
            commands,
            ctx.soul_entity,
            soul_pos,
            ctx.task,
            ctx.path,
            Some(ctx.inventory),
            None, // アイテムを拾う前なのでNone
            &ctx.queries,
            haul_cache,
            world_map,
            true, // 失敗時はセリフを出す
        );
        return;
    }



    match phase {
        HaulToBpPhase::GoingToItem => {
            if let Ok((item_transform, _, _, _, des_opt, stored_in_opt)) = q_targets.get(item_entity) {
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
                update_destination_to_adjacent(ctx.dest, item_pos, ctx.path, soul_pos, world_map);
                let is_near = is_near_target(soul_pos, item_pos);
                
                if is_near {
                    pickup_item(commands, ctx.soul_entity, item_entity, ctx.inventory);

                    // もしアイテムが備蓄場所にあったなら、その備蓄場所の型管理を更新する
                    if let Some(stored_in) = stored_in_opt {
                        update_stockpile_on_item_removal(stored_in.0, q_stockpiles);
                    }

                    // ブループリントへの目的地設定は、次のフレームの GoingToBlueprint フェーズで
                    // update_destination_to_blueprint により自動的に（一貫したロジックで）行われるため、
                    // ここではパスをクリアするのみとする。

                    *ctx.task = AssignedTask::HaulToBlueprint(crate::systems::soul_ai::task_execution::types::HaulToBlueprintData {
                        item: item_entity,
                        blueprint: blueprint_entity,
                        phase: HaulToBpPhase::GoingToBlueprint,
                    });
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
                clear_task_and_path(ctx.task, ctx.path);
                commands.entity(ctx.soul_entity).remove::<WorkingOn>();
            }
        }
        HaulToBpPhase::GoingToBlueprint => {
            if let Ok((_bp_transform, bp, _)) = q_blueprints.get(blueprint_entity) {
                update_destination_to_blueprint(ctx.dest, &bp.occupied_grids, ctx.path, soul_pos, world_map, ctx.pf_context);

                if is_near_blueprint(soul_pos, &bp.occupied_grids) {
                    info!(
                        "HAUL_TO_BP: Soul {:?} arrived at blueprint {:?}",
                        ctx.soul_entity, blueprint_entity
                    );
                    *ctx.task = AssignedTask::HaulToBlueprint(crate::systems::soul_ai::task_execution::types::HaulToBlueprintData {
                        item: item_entity,
                        blueprint: blueprint_entity,
                        phase: HaulToBpPhase::Delivering,
                    });
                    ctx.path.waypoints.clear();
                }
            } else {
                info!(
                    "HAUL_TO_BP: Cancelled for {:?} - Blueprint {:?} gone",
                    ctx.soul_entity, blueprint_entity
                );
                // Blueprint が消失 - アイテムを解除して再発行
                let dropped_res = q_targets.get(item_entity).ok().and_then(|(_, _, _, ri, _, _)| ri.map(|r| r.0));
                crate::systems::soul_ai::work::unassign_task(
                    commands,
                    ctx.soul_entity,
                    soul_pos,
                    ctx.task,
                    ctx.path,
                    Some(ctx.inventory),
                    dropped_res,
                    &ctx.queries,
                    haul_cache,
                    world_map,
                    true,
                );
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
                        if let Ok((_, _, _designation, managed_by_opt, _, _, _, _)) =
                            q_designations.get(blueprint_entity)
                        {
                            // ManagedByを削除して未割り当て状態にする
                            if managed_by_opt.is_some() {
                                commands.entity(blueprint_entity).remove::<crate::relationships::ManagedBy>();
                            }

                            // Priority(10) を付与して使い魔がタスクを探せるようにする
                            commands.entity(blueprint_entity).insert(crate::systems::jobs::Priority(10));

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

            ctx.inventory.0 = None;
            commands.entity(ctx.soul_entity).remove::<WorkingOn>();
            clear_task_and_path(ctx.task, ctx.path);
            ctx.soul.fatigue = (ctx.soul.fatigue + 0.05).min(1.0);
        }
    }
}
