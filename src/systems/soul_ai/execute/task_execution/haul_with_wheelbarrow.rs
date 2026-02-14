//! 手押し車による一括運搬タスクの実行処理

use crate::constants::*;
use crate::relationships::{LoadedIn, ParkedAt, PushedBy, WorkingOn};
use crate::systems::logistics::transport_request::WheelbarrowDestination;
use crate::systems::logistics::{InStockpile, Wheelbarrow};
use crate::systems::soul_ai::execute::task_execution::common::*;
use crate::systems::soul_ai::execute::task_execution::transport_common::{
    reservation,
    wheelbarrow as wheelbarrow_common,
};
use crate::systems::soul_ai::execute::task_execution::{
    context::TaskExecutionContext,
    types::{AssignedTask, HaulWithWheelbarrowData, HaulWithWheelbarrowPhase},
};
use crate::world::map::WorldMap;
use bevy::prelude::*;

pub fn handle_haul_with_wheelbarrow_task(
    ctx: &mut TaskExecutionContext,
    data: HaulWithWheelbarrowData,
    commands: &mut Commands,
    world_map: &Res<WorldMap>,
    q_wheelbarrows: &Query<(&Transform, Option<&ParkedAt>), With<Wheelbarrow>>,
) {
    let soul_pos = ctx.soul_pos();

    match data.phase {
        HaulWithWheelbarrowPhase::GoingToParking => {
            // 駐車エリア（手押し車の位置）へ移動
            let Ok((wb_transform, _)) = q_wheelbarrows.get(data.wheelbarrow) else {
                info!(
                    "WB_HAUL: Wheelbarrow {:?} not found, canceling",
                    data.wheelbarrow
                );
                clear_task_and_path(ctx.task, ctx.path);
                return;
            };

            let wb_pos = wb_transform.translation.truncate();
            let reachable = update_destination_to_adjacent(
                ctx.dest,
                wb_pos,
                ctx.path,
                soul_pos,
                world_map,
                ctx.pf_context,
            );

            if !reachable {
                clear_task_and_path(ctx.task, ctx.path);
                return;
            }

            if is_near_target(soul_pos, wb_pos) {
                *ctx.task = AssignedTask::HaulWithWheelbarrow(HaulWithWheelbarrowData {
                    phase: HaulWithWheelbarrowPhase::PickingUpWheelbarrow,
                    ..data
                });
                ctx.path.waypoints.clear();
            }
        }

        HaulWithWheelbarrowPhase::PickingUpWheelbarrow => {
            // 手押し車を取得: ParkedAt 削除, PushedBy 設定, Inventory に設定
            commands.entity(data.wheelbarrow).remove::<ParkedAt>();
            commands
                .entity(data.wheelbarrow)
                .insert(PushedBy(ctx.soul_entity));
            commands
                .entity(data.wheelbarrow)
                .insert(Visibility::Visible);
            ctx.inventory.0 = Some(data.wheelbarrow);

            info!(
                "WB_HAUL: Soul {:?} picked up wheelbarrow {:?}",
                ctx.soul_entity, data.wheelbarrow
            );

            *ctx.task = AssignedTask::HaulWithWheelbarrow(HaulWithWheelbarrowData {
                phase: HaulWithWheelbarrowPhase::GoingToSource,
                ..data
            });
        }

        HaulWithWheelbarrowPhase::GoingToSource => {
            // 積み込み元（アイテム集積地点）へ移動
            let reachable = update_destination_to_adjacent(
                ctx.dest,
                data.source_pos,
                ctx.path,
                soul_pos,
                world_map,
                ctx.pf_context,
            );

            if !reachable {
                cancel_wheelbarrow_task(ctx, &data, commands);
                return;
            }

            if is_near_target(soul_pos, data.source_pos) {
                *ctx.task = AssignedTask::HaulWithWheelbarrow(HaulWithWheelbarrowData {
                    phase: HaulWithWheelbarrowPhase::Loading,
                    ..data
                });
                ctx.path.waypoints.clear();
            }
        }

        HaulWithWheelbarrowPhase::Loading => {
            // アイテム情報を先に収集（borrowing conflict 回避）
            // 距離制限なし: 予約済みアイテムは全て積み込む
            let items_to_load: Vec<(Entity, Option<Entity>)> = data
                .items
                .iter()
                .filter_map(|&item_entity| {
                    let Ok((_, _, _, _, _, _, stored_in_opt)) =
                        ctx.queries.designation.targets.get(item_entity)
                    else {
                        return None;
                    };
                    Some((item_entity, stored_in_opt.map(|si| si.0)))
                })
                .collect();

            // 収集した情報を使ってアイテムを積み込む
            for (item_entity, stored_in_stockpile) in &items_to_load {
                commands
                    .entity(*item_entity)
                    .insert((Visibility::Hidden, LoadedIn(data.wheelbarrow)));
                commands
                    .entity(*item_entity)
                    .remove::<crate::relationships::StoredIn>();
                commands.entity(*item_entity).remove::<InStockpile>();
                commands
                    .entity(*item_entity)
                    .remove::<crate::systems::jobs::Designation>();
                commands
                    .entity(*item_entity)
                    .remove::<crate::systems::jobs::TaskSlots>();
                commands
                    .entity(*item_entity)
                    .remove::<crate::systems::jobs::Priority>();
                commands
                    .entity(*item_entity)
                    .remove::<crate::systems::logistics::ReservedForTask>();

                if let Some(stock_entity) = stored_in_stockpile {
                    update_stockpile_on_item_removal(*stock_entity, &mut ctx.queries.storage.stockpiles);
                }

                reservation::record_picked_source(ctx, *item_entity, 1);
            }

            // 全アイテムの積み込み完了後、移動先へ
            *ctx.task = AssignedTask::HaulWithWheelbarrow(HaulWithWheelbarrowData {
                phase: HaulWithWheelbarrowPhase::GoingToDestination,
                ..data
            });

            info!(
                "WB_HAUL: Soul {:?} loaded {} items into wheelbarrow",
                ctx.soul_entity,
                items_to_load.len()
            );
        }

        HaulWithWheelbarrowPhase::GoingToDestination => {
            // 目的地へ移動（速度ペナルティは movement system で適用）
            let (reachable, arrived) = match data.destination {
                WheelbarrowDestination::Stockpile(stockpile_entity) => {
                    let Ok((_, stock_transform, _, _)) =
                        ctx.queries.storage.stockpiles.get(stockpile_entity)
                    else {
                        info!("WB_HAUL: Destination stockpile not found, canceling");
                        cancel_wheelbarrow_task(ctx, &data, commands);
                        return;
                    };

                    let stock_pos = stock_transform.translation.truncate();
                    let reachable = update_destination_to_adjacent(
                        ctx.dest,
                        stock_pos,
                        ctx.path,
                        soul_pos,
                        world_map,
                        ctx.pf_context,
                    );
                    (reachable, is_near_target(soul_pos, stock_pos))
                }
                WheelbarrowDestination::Blueprint(blueprint_entity) => {
                    let Ok((_, blueprint, _)) = ctx.queries.storage.blueprints.get(blueprint_entity)
                    else {
                        info!("WB_HAUL: Destination blueprint not found, canceling");
                        cancel_wheelbarrow_task(ctx, &data, commands);
                        return;
                    };

                    update_destination_to_blueprint(
                        ctx.dest,
                        &blueprint.occupied_grids,
                        ctx.path,
                        soul_pos,
                        world_map,
                        ctx.pf_context,
                    );
                    (
                        true,
                        is_near_blueprint(soul_pos, &blueprint.occupied_grids),
                    )
                }
                WheelbarrowDestination::Mixer { entity, .. } => {
                    let Ok((mixer_transform, _, _)) = ctx.queries.storage.mixers.get(entity) else {
                        info!("WB_HAUL: Destination mixer not found, canceling");
                        cancel_wheelbarrow_task(ctx, &data, commands);
                        return;
                    };

                    let mixer_pos = mixer_transform.translation.truncate();
                    let reachable = update_destination_to_adjacent(
                        ctx.dest,
                        mixer_pos,
                        ctx.path,
                        soul_pos,
                        world_map,
                        ctx.pf_context,
                    );
                    (reachable, is_near_target_or_dest(soul_pos, mixer_pos, ctx.dest.0))
                }
            };

            if !reachable {
                cancel_wheelbarrow_task(ctx, &data, commands);
                return;
            }

            if arrived {
                *ctx.task = AssignedTask::HaulWithWheelbarrow(HaulWithWheelbarrowData {
                    phase: HaulWithWheelbarrowPhase::Unloading,
                    ..data
                });
                ctx.path.waypoints.clear();
            }
        }

        HaulWithWheelbarrowPhase::Unloading => {
            // アイテムの型情報を先に収集（borrowing conflict 回避）
            let item_types: Vec<(Entity, Option<crate::systems::logistics::ResourceType>)> = data
                .items
                .iter()
                .filter_map(|&item_entity| {
                    let Ok((_, _, _, _, res_item_opt, _, _)) =
                        ctx.queries.designation.targets.get(item_entity)
                    else {
                        return None;
                    };
                    Some((item_entity, res_item_opt.map(|r| r.0)))
                })
                .collect();

            let mut unloaded_count = 0usize;
            let mut destination_store_count = 0usize;
            let mut mixer_release_types = Vec::new();

            match data.destination {
                WheelbarrowDestination::Stockpile(dest_stockpile) => {
                    // ストックパイルの情報を取得して荷下ろし
                    if let Ok((_, stock_transform, mut stockpile_comp, stored_items_opt)) =
                        ctx.queries.storage.stockpiles.get_mut(dest_stockpile)
                    {
                        let stock_pos = stock_transform.translation;
                        let current_count = stored_items_opt.map(|si| si.len()).unwrap_or(0);

                        for (item_entity, res_type_opt) in &item_types {
                            if current_count + unloaded_count >= stockpile_comp.capacity {
                                break;
                            }
                            let Some(res_type) = res_type_opt else {
                                continue;
                            };

                            if stockpile_comp.resource_type.is_none() {
                                stockpile_comp.resource_type = Some(*res_type);
                            } else if stockpile_comp.resource_type != Some(*res_type) {
                                continue;
                            }

                            commands.entity(*item_entity).insert((
                                Visibility::Visible,
                                Transform::from_xyz(stock_pos.x, stock_pos.y, Z_ITEM_PICKUP),
                                crate::relationships::StoredIn(dest_stockpile),
                                InStockpile(dest_stockpile),
                            ));
                            commands.entity(*item_entity).remove::<LoadedIn>();
                            commands
                                .entity(*item_entity)
                                .remove::<crate::systems::jobs::IssuedBy>();
                            commands
                                .entity(*item_entity)
                                .remove::<crate::relationships::TaskWorkers>();

                            destination_store_count += 1;
                            unloaded_count += 1;
                        }
                    } else {
                        cancel_wheelbarrow_task(ctx, &data, commands);
                        return;
                    }
                }
                WheelbarrowDestination::Blueprint(blueprint_entity) => {
                    if let Ok((_, mut blueprint, _)) =
                        ctx.queries.storage.blueprints.get_mut(blueprint_entity)
                    {
                        for (item_entity, res_type_opt) in &item_types {
                            let Some(res_type) = res_type_opt else {
                                continue;
                            };

                            blueprint.deliver_material(*res_type, 1);
                            commands.entity(*item_entity).despawn();
                            destination_store_count += 1;
                            unloaded_count += 1;
                        }

                        if blueprint.materials_complete() {
                            commands
                                .entity(blueprint_entity)
                                .remove::<crate::relationships::ManagedBy>();
                            commands
                                .entity(blueprint_entity)
                                .insert(crate::systems::jobs::Priority(10));
                        }
                    } else {
                        cancel_wheelbarrow_task(ctx, &data, commands);
                        return;
                    }
                }
                WheelbarrowDestination::Mixer {
                    entity: mixer_entity,
                    resource_type,
                } => {
                    if let Ok((_, mut storage, _)) = ctx.queries.storage.mixers.get_mut(mixer_entity) {
                        for (item_entity, res_type_opt) in &item_types {
                            let res_type = (*res_type_opt).unwrap_or(resource_type);

                            if storage.add_material(res_type).is_ok() {
                                commands.entity(*item_entity).despawn();
                                unloaded_count += 1;
                            } else {
                                commands.entity(*item_entity).remove::<LoadedIn>();
                                commands.entity(*item_entity).insert((
                                    Visibility::Visible,
                                    Transform::from_xyz(soul_pos.x, soul_pos.y, Z_ITEM_PICKUP),
                                ));
                            }

                            mixer_release_types.push(res_type);
                        }
                    } else {
                        cancel_wheelbarrow_task(ctx, &data, commands);
                        return;
                    }
                }
            }

            match data.destination {
                WheelbarrowDestination::Stockpile(target)
                | WheelbarrowDestination::Blueprint(target) => {
                    for _ in 0..destination_store_count {
                        reservation::record_stored_destination(ctx, target);
                    }
                }
                WheelbarrowDestination::Mixer { entity: target, .. } => {
                    for res_type in mixer_release_types {
                        reservation::release_mixer_destination(ctx, target, res_type);
                    }
                }
            }

            info!(
                "WB_HAUL: Soul {:?} unloaded {} items",
                ctx.soul_entity, unloaded_count
            );

            // 手押し車を返却するフェーズへ
            *ctx.task = AssignedTask::HaulWithWheelbarrow(HaulWithWheelbarrowData {
                phase: HaulWithWheelbarrowPhase::ReturningWheelbarrow,
                ..data
            });
        }

        HaulWithWheelbarrowPhase::ReturningWheelbarrow => {
            // 手押し車の元の駐車エリアへ移動
            let Ok(_) = q_wheelbarrows.get(data.wheelbarrow) else {
                // 手押し車消失
                ctx.inventory.0 = None;
                commands.entity(ctx.soul_entity).remove::<WorkingOn>();
                clear_task_and_path(ctx.task, ctx.path);
                return;
            };

            // 駐車エリア（BelongsTo 先の建物）の位置を取得
            let parking_pos = ctx
                .queries
                .designation
                .belongs
                .get(data.wheelbarrow)
                .ok()
                .and_then(|belongs| {
                    ctx.queries
                        .designation
                        .targets
                        .get(belongs.0)
                        .ok()
                        .map(|(tf, _, _, _, _, _, _)| tf.translation.truncate())
                })
                .unwrap_or(soul_pos);

            let reachable = update_destination_to_adjacent(
                ctx.dest,
                parking_pos,
                ctx.path,
                soul_pos,
                world_map,
                ctx.pf_context,
            );

            if !reachable {
                wheelbarrow_common::complete_wheelbarrow_task(commands, ctx, &data, soul_pos);
                info!(
                    "WB_HAUL: Soul {:?} returned wheelbarrow {:?} (unreachable, parked here)",
                    ctx.soul_entity, data.wheelbarrow
                );
                return;
            }

            if is_near_target(soul_pos, parking_pos) {
                wheelbarrow_common::complete_wheelbarrow_task(commands, ctx, &data, parking_pos);
                info!(
                    "WB_HAUL: Soul {:?} returned wheelbarrow {:?}",
                    ctx.soul_entity, data.wheelbarrow
                );
            }
        }
    }
}

/// 手押し車タスクのキャンセル処理
fn cancel_wheelbarrow_task(
    ctx: &mut TaskExecutionContext,
    data: &HaulWithWheelbarrowData,
    commands: &mut Commands,
) {
    let soul_pos = ctx.soul_pos();

    // 積載中のアイテムを地面に落とす
    for &item_entity in &data.items {
        if commands.get_entity(item_entity).is_ok() {
            commands.entity(item_entity).remove::<LoadedIn>();
            commands.entity(item_entity).insert((
                Visibility::Visible,
                Transform::from_xyz(soul_pos.x, soul_pos.y, Z_ITEM_PICKUP),
            ));
        }
    }

    // 手押し車を駐車状態に戻す
    let parking_anchor = ctx
        .queries
        .designation
        .belongs
        .get(data.wheelbarrow)
        .ok()
        .map(|b| b.0);
    wheelbarrow_common::park_wheelbarrow_entity(
        commands,
        data.wheelbarrow,
        parking_anchor,
        soul_pos,
    );

    for &item_entity in &data.items {
        reservation::release_source(ctx, item_entity, 1);

        match data.destination {
            WheelbarrowDestination::Stockpile(target)
            | WheelbarrowDestination::Blueprint(target) => {
                reservation::release_destination(ctx, target);
            }
            WheelbarrowDestination::Mixer {
                entity: target,
                resource_type,
            } => {
                let item_type = ctx
                    .queries
                    .reservation
                    .resources
                    .get(item_entity)
                    .ok()
                    .map(|r| r.0)
                    .unwrap_or(resource_type);
                reservation::release_mixer_destination(ctx, target, item_type);
            }
        }
    }

    ctx.inventory.0 = None;
    commands.entity(ctx.soul_entity).remove::<WorkingOn>();
    clear_task_and_path(ctx.task, ctx.path);

    info!(
        "WB_HAUL: Soul {:?} canceled wheelbarrow task",
        ctx.soul_entity
    );
}
