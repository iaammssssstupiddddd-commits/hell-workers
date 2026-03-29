//! 設計図への運搬タスクの実行処理

use hw_core::relationships::WorkingOn;
use hw_core::soul::StressBreakdown;
use crate::soul_ai::execute::task_execution::{
    chain,
    common::*,
    context::TaskExecutionContext,
    transport_common::{cancel, reservation},
    types::{AssignedTask, HaulToBpPhase},
};
use bevy::prelude::*;
use hw_world::WorldMap;

pub fn handle_haul_to_blueprint_task(
    ctx: &mut TaskExecutionContext,
    breakdown_opt: Option<&StressBreakdown>,
    item_entity: Entity,
    blueprint_entity: Entity,
    phase: HaulToBpPhase,
    commands: &mut Commands,
    world_map: &WorldMap,
) {
    let soul_pos = ctx.soul_pos();
    // 疲労またはストレス崩壊のチェック
    if ctx.soul.fatigue > 0.95 || breakdown_opt.is_some() {
        info!(
            "HAUL_TO_BP: Cancelled for {:?} - Exhausted or Stress breakdown",
            ctx.soul_entity
        );
        let soul_pos = ctx.soul_transform.translation.truncate();
        crate::soul_ai::helpers::work::cleanup_task_assignment(
            commands,
            crate::soul_ai::helpers::work::SoulDropCtx {
                soul_entity: ctx.soul_entity,
                drop_pos: soul_pos,
                inventory: Some(ctx.inventory),
                dropped_item_res: None,
            },
            ctx.task,
            ctx.path,
            ctx.queries,
            world_map,
            true,
        );
        return;
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
                    world_map,
                    ctx.pf_context,
                );
                let is_near = can_pickup_item(soul_pos, item_pos);

                if is_near {
                    if !try_pickup_item(
                        commands,
                        PickupLocations {
                            soul_entity: ctx.soul_entity,
                            item_entity,
                            soul_pos,
                            item_pos,
                        },
                        ctx.inventory,
                        ctx.task,
                        ctx.path,
                    ) {
                        return;
                    }
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
                cancel::cancel_haul_to_blueprint(ctx, item_entity, blueprint_entity, commands);
            }
        }
        HaulToBpPhase::GoingToBlueprint => {
            if let Ok((_bp_transform, bp, _)) =
                ctx.queries.storage.blueprints.get(blueprint_entity)
            {
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
                    cancel::cancel_haul_to_blueprint(ctx, item_entity, blueprint_entity, commands);
                    return;
                }

                if is_near_blueprint(soul_pos, &bp.occupied_grids) {
                    info!(
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
                info!(
                    "HAUL_TO_BP: Cancelled for {:?} - Blueprint {:?} gone",
                    ctx.soul_entity, blueprint_entity
                );
                let dropped_res = ctx
                    .queries
                    .designation
                    .targets
                    .get(item_entity)
                    .ok()
                    .and_then(|(_, _, _, _, ri, _, _)| ri.map(|r| r.0));
                crate::soul_ai::helpers::work::cleanup_task_assignment(
                    commands,
                    crate::soul_ai::helpers::work::SoulDropCtx {
                        soul_entity: ctx.soul_entity,
                        drop_pos: soul_pos,
                        inventory: Some(ctx.inventory),
                        dropped_item_res: dropped_res,
                    },
                    ctx.task,
                    ctx.path,
                    ctx.queries,
                    world_map,
                    true,
                );
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
                reservation::release_destination(ctx, blueprint_entity);
                return;
            };

            // Step 2: 搬入処理（blueprints の可変借用をスコープ内に閉じ込める）
            enum DeliverResult {
                Done { materials_complete: bool },
                Cancel,
            }
            let result = {
                if let Ok((_, mut bp, _)) =
                    ctx.queries.storage.blueprints.get_mut(blueprint_entity)
                {
                    if bp.remaining_material_amount(resource_type) == 0 {
                        info!(
                            "HAUL_TO_BP: Cancelled delivery for {:?} - blueprint {:?} no longer needs {:?}",
                            ctx.soul_entity, blueprint_entity, resource_type
                        );
                        DeliverResult::Cancel
                    } else {
                        bp.deliver_material(resource_type, 1);
                        info!(
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
                    cancel::cancel_haul_to_blueprint(ctx, item_entity, blueprint_entity, commands);
                    return;
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
                    info!(
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
                return;
            }

            // Step 5: チェーンなし — 通常のクリーンアップ
            ctx.inventory.0 = None;
            commands.entity(ctx.soul_entity).remove::<WorkingOn>();
            clear_task_and_path(ctx.task, ctx.path);
            commands.entity(item_entity).despawn();
            ctx.soul.fatigue = (ctx.soul.fatigue + 0.05).min(1.0);
            reservation::release_destination(ctx, blueprint_entity);
        }
    }
}
