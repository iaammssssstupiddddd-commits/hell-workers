//! 設計図への運搬タスクの実行処理

use crate::soul_ai::execute::task_execution::{
    chain,
    common::*,
    context::{TaskExecutionContext, TaskHandlerControl},
    transport_common::{cancel, reservation},
    types::{AssignedTask, HaulToBlueprintData, HaulToBpPhase},
};
use bevy::prelude::*;

pub fn handle_haul_to_blueprint_task(
    ctx: &mut TaskExecutionContext,
    data: HaulToBlueprintData,
    commands: &mut Commands,
) -> TaskHandlerControl {
    let HaulToBlueprintData {
        item,
        blueprint,
        phase,
    } = data;
    let item_entity = item;
    let blueprint_entity = blueprint;
    let soul_pos = ctx.soul_pos();
    // 疲労またはストレス崩壊のチェック
    if ctx.soul.fatigue > 0.95 || ctx.env.breakdown.is_some() {
        debug!(
            "HAUL_TO_BP: Cancelled for {:?} - Exhausted or Stress breakdown",
            ctx.soul_entity
        );
        return ctx.abort_retryable(
            commands,
            "haul to blueprint interrupted by fatigue or breakdown",
        );
    }

    match phase {
        HaulToBpPhase::GoingToItem => {
            let q_targets = &ctx.queries.designation.targets;
            if let Ok((item_transform, _, _, _, _, _, stored_in_opt)) = q_targets.get(item_entity) {
                let item_pos = item_transform.translation.truncate();
                let stored_in_entity = stored_in_opt.map(|stored_in| stored_in.0);
                update_destination_to_adjacent(
                    ctx.dest,
                    item_pos,
                    ctx.path,
                    soul_pos,
                    ctx.env.world_map,
                    ctx.pf_context,
                );
                let is_near = can_pickup_item(soul_pos, item_pos);

                if is_near {
                    pickup_item(commands, ctx.soul_entity, item_entity, ctx.inventory);
                    release_mixer_mud_storage_for_item(ctx, item_entity, commands);

                    if let Some(stored_in) = stored_in_entity {
                        update_stockpile_on_item_removal(
                            stored_in,
                            &mut ctx.queries.storage.stockpiles,
                        );
                    }

                    reservation::record_picked_source(ctx, item_entity, 1);

                    *ctx.task = AssignedTask::HaulToBlueprint(
                        crate::soul_ai::execute::task_execution::types::HaulToBlueprintData {
                            item: item_entity,
                            blueprint: blueprint_entity,
                            phase: HaulToBpPhase::GoingToBlueprint,
                        },
                    );
                    ctx.path.waypoints.clear();
                    debug!(
                        "HAUL_TO_BP: Soul {:?} picked up item {:?}, heading to blueprint {:?}",
                        ctx.soul_entity, item_entity, blueprint_entity
                    );
                }
            } else {
                debug!(
                    "HAUL_TO_BP: Cancelled for {:?} - Item {:?} gone",
                    ctx.soul_entity, item_entity
                );
                return cancel::cancel_haul_to_blueprint(
                    ctx,
                    item_entity,
                    blueprint_entity,
                    commands,
                );
            }
        }
        HaulToBpPhase::GoingToBlueprint => {
            if let Ok((_bp_transform, bp, _)) = ctx.queries.storage.blueprints.get(blueprint_entity)
            {
                let reachable = update_destination_to_blueprint(
                    ctx.dest,
                    &bp.occupied_grids,
                    ctx.path,
                    soul_pos,
                    ctx.env.world_map,
                    ctx.pf_context,
                );
                if !reachable {
                    debug!(
                        "HAUL_TO_BP: Cancelled for {:?} - Blueprint {:?} unreachable",
                        ctx.soul_entity, blueprint_entity
                    );
                    return cancel::cancel_haul_to_blueprint(
                        ctx,
                        item_entity,
                        blueprint_entity,
                        commands,
                    );
                }

                if is_near_blueprint(soul_pos, &bp.occupied_grids) {
                    debug!(
                        "HAUL_TO_BP: Soul {:?} arrived at blueprint {:?}",
                        ctx.soul_entity, blueprint_entity
                    );
                    *ctx.task = AssignedTask::HaulToBlueprint(
                        crate::soul_ai::execute::task_execution::types::HaulToBlueprintData {
                            item: item_entity,
                            blueprint: blueprint_entity,
                            phase: HaulToBpPhase::Delivering,
                        },
                    );
                    ctx.path.waypoints.clear();
                }
            } else {
                debug!(
                    "HAUL_TO_BP: Cancelled for {:?} - Blueprint {:?} gone",
                    ctx.soul_entity, blueprint_entity
                );
                return ctx.abort_closed(commands, "haul to blueprint destination disappeared");
            }
        }
        HaulToBpPhase::Delivering => {
            // Step 1: アイテムのリソースタイプを先に取得（blueprints の借用不要）
            let Some(resource_type) = ctx
                .queries
                .designation
                .targets
                .get(item_entity)
                .ok()
                .and_then(|(_, _, _, _, ri, _, _)| ri.map(|r| r.0))
            else {
                ctx.soul.fatigue = (ctx.soul.fatigue + 0.05).min(1.0);
                return ctx.abort_closed(commands, "haul to blueprint item disappeared");
            };

            // Step 2: 搬入処理（blueprints の可変借用をスコープ内に閉じ込める）
            enum DeliverResult {
                Done { materials_complete: bool },
                Cancel,
            }
            let result = {
                if let Ok((_, mut bp, _)) = ctx.queries.storage.blueprints.get_mut(blueprint_entity)
                {
                    if bp.remaining_material_amount(resource_type) == 0 {
                        debug!(
                            "HAUL_TO_BP: Cancelled delivery for {:?} - blueprint {:?} no longer needs {:?}",
                            ctx.soul_entity, blueprint_entity, resource_type
                        );
                        DeliverResult::Cancel
                    } else {
                        bp.deliver_material(resource_type, 1);
                        debug!(
                            "HAUL_TO_BP: Soul {:?} delivered {:?} to blueprint {:?}. Progress: {:?}/{:?}",
                            ctx.soul_entity,
                            resource_type,
                            blueprint_entity,
                            bp.delivered_materials,
                            bp.required_materials
                        );
                        let done = bp.materials_complete();
                        DeliverResult::Done {
                            materials_complete: done,
                        }
                    }
                    // bp と blueprints の可変借用がここで解放される
                } else {
                    // Blueprint が消失
                    DeliverResult::Cancel
                }
            };

            let materials_complete = match result {
                DeliverResult::Cancel => {
                    return cancel::cancel_haul_to_blueprint(
                        ctx,
                        item_entity,
                        blueprint_entity,
                        commands,
                    );
                }
                DeliverResult::Done { materials_complete } => materials_complete,
            };

            // Step 3: 素材完成時のサイドエフェクト（ManagedBy 削除・Priority 付与）
            if materials_complete
                && let Ok((_, _, _, managed_by_opt, _, _, _, _)) =
                    ctx.queries.designation.designations.get(blueprint_entity)
            {
                if managed_by_opt.is_some() {
                    commands
                        .entity(blueprint_entity)
                        .remove::<hw_core::relationships::ManagedBy>();
                }
                commands
                    .entity(blueprint_entity)
                    .insert(hw_jobs::Priority(10));
                debug!(
                    "HAUL_TO_BP: Blueprint {:?} materials complete, reissuing for build task",
                    blueprint_entity
                );
            }

            // Step 4: チェーン判定（blueprints の借用が解放済みなので ctx を安全に渡せる）
            if let Some(opp) = chain::find_chain_opportunity(
                blueprint_entity,
                resource_type,
                Some(materials_complete),
                ctx,
            ) {
                ctx.inventory.0 = None;
                chain::execute_chain(opp, ctx, commands);
                commands.entity(item_entity).despawn();
                reservation::release_destination(ctx, blueprint_entity);
                return TaskHandlerControl::Continue;
            }

            // Step 5: チェーンなし — 通常のクリーンアップ
            ctx.inventory.0 = None;
            commands.entity(item_entity).despawn();
            ctx.soul.fatigue = (ctx.soul.fatigue + 0.05).min(1.0);
            reservation::release_destination(ctx, blueprint_entity);
            return ctx.complete_task(commands, "haul to blueprint done");
        }
    }

    TaskHandlerControl::Continue
}
