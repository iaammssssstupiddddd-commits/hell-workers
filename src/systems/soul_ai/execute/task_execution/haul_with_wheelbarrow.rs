//! 手押し車による一括運搬タスクの実行処理

use crate::constants::*;
use crate::relationships::{LoadedIn, ParkedAt, PushedBy, WorkingOn};
use crate::systems::logistics::transport_request::WheelbarrowDestination;
use crate::systems::logistics::{InStockpile, Wheelbarrow};
use crate::systems::soul_ai::execute::task_execution::common::*;
use crate::systems::soul_ai::execute::task_execution::transport_common::{
    reservation, sand_collect,
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
            handle_going_to_parking(ctx, data, commands, world_map, q_wheelbarrows, soul_pos);
        }
        HaulWithWheelbarrowPhase::PickingUpWheelbarrow => {
            handle_picking_up_wheelbarrow(ctx, data, commands);
        }
        HaulWithWheelbarrowPhase::GoingToSource => {
            handle_going_to_source(ctx, data, commands, world_map, soul_pos);
        }
        HaulWithWheelbarrowPhase::Loading => {
            handle_loading(ctx, data, commands);
        }
        HaulWithWheelbarrowPhase::GoingToDestination => {
            handle_going_to_destination(ctx, data, commands, world_map, soul_pos);
        }
        HaulWithWheelbarrowPhase::Unloading => {
            handle_unloading(ctx, data, commands, soul_pos);
        }
        HaulWithWheelbarrowPhase::ReturningWheelbarrow => {
            handle_returning_wheelbarrow(ctx, data, commands, world_map, q_wheelbarrows, soul_pos);
        }
    }
}

fn handle_going_to_parking(
    ctx: &mut TaskExecutionContext,
    data: HaulWithWheelbarrowData,
    commands: &mut Commands,
    world_map: &Res<WorldMap>,
    q_wheelbarrows: &Query<(&Transform, Option<&ParkedAt>), With<Wheelbarrow>>,
    soul_pos: Vec2,
) {
    let Ok((wb_transform, _)) = q_wheelbarrows.get(data.wheelbarrow) else {
        info!(
            "WB_HAUL: Wheelbarrow {:?} not found, canceling",
            data.wheelbarrow
        );
        // まだ猫車を取得していないので予約解放のみ
        cancel_wheelbarrow_task(ctx, &data, commands);
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
        cancel_wheelbarrow_task(ctx, &data, commands);
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

fn handle_picking_up_wheelbarrow(
    ctx: &mut TaskExecutionContext,
    data: HaulWithWheelbarrowData,
    commands: &mut Commands,
) {
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

fn handle_going_to_source(
    ctx: &mut TaskExecutionContext,
    data: HaulWithWheelbarrowData,
    commands: &mut Commands,
    world_map: &Res<WorldMap>,
    soul_pos: Vec2,
) {
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
        // 搬入先の空き容量チェック
        if let WheelbarrowDestination::Stockpile(stockpile) = data.destination {
            if let Ok((_, _, stock, stored_items)) = ctx.queries.storage.stockpiles.get(stockpile) {
                let current_count = stored_items.map(|s| s.len()).unwrap_or(0);
                let incoming = ctx
                    .queries
                    .reservation
                    .incoming_deliveries_query
                    .get(stockpile)
                    .ok()
                    .map(|inc: &crate::relationships::IncomingDeliveries| inc.len())
                    .unwrap_or(0);
                if (current_count + incoming) >= stock.capacity {
                    cancel_wheelbarrow_task(ctx, &data, commands);
                    return;
                }
            }
        }

        *ctx.task = AssignedTask::HaulWithWheelbarrow(HaulWithWheelbarrowData {
            phase: HaulWithWheelbarrowPhase::Loading,
            ..data
        });
        ctx.path.waypoints.clear();
    }
}

fn handle_loading(
    ctx: &mut TaskExecutionContext,
    data: HaulWithWheelbarrowData,
    commands: &mut Commands,
) {
    if let Some(source_entity) = data.collect_source {
        let collect_amount = data.collect_amount.max(1);
        if data.collect_resource_type != Some(crate::systems::logistics::ResourceType::Sand) {
            cancel_wheelbarrow_task(ctx, &data, commands);
            return;
        }
        if ctx.queries.designation.targets.get(source_entity).is_err() {
            cancel_wheelbarrow_task(ctx, &data, commands);
            return;
        }

        let collected_items = sand_collect::spawn_loaded_sand_items(
            commands,
            data.wheelbarrow,
            data.source_pos,
            collect_amount,
        );
        if collected_items.is_empty() {
            cancel_wheelbarrow_task(ctx, &data, commands);
            return;
        }

        sand_collect::clear_collect_sand_designation(commands, source_entity);
        reservation::release_source(ctx, source_entity, 1);

        let loaded_count = collected_items.len();
        for &item in &collected_items {
            commands.entity(item).insert(crate::relationships::DeliveringTo(data.destination.stockpile_or_blueprint().unwrap()));
        }
        let mut next_data = data;
        next_data.items = collected_items;
        next_data.collect_source = None;
        next_data.collect_amount = 0;
        next_data.collect_resource_type = None;
        next_data.phase = HaulWithWheelbarrowPhase::GoingToDestination;
        *ctx.task = AssignedTask::HaulWithWheelbarrow(next_data);

        info!(
            "WB_HAUL: Soul {:?} collected {} sand directly into wheelbarrow",
            ctx.soul_entity, loaded_count
        );
        return;
    }

    // アイテム情報を先に収集（borrowing conflict 回避）
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

    // 改善: 1個も積載できなかった場合はキャンセル
    if items_to_load.is_empty() {
        info!(
            "WB_HAUL: Soul {:?} found no loadable items, canceling",
            ctx.soul_entity
        );
        cancel_wheelbarrow_task(ctx, &data, commands);
        return;
    }

    // 収集した情報を使ってアイテムを積み込む
    for (item_entity, stored_in_stockpile) in &items_to_load {
        release_mixer_mud_storage_for_item(ctx, *item_entity, commands);
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

    // 消失したアイテムの予約を解放
    let loaded_count = items_to_load.len();
    let total_count = data.items.len();
    if loaded_count < total_count {
        let loaded_entities: std::collections::HashSet<Entity> =
            items_to_load.iter().map(|(e, _)| *e).collect();
                for &item_entity in &data.items {
                    if !loaded_entities.contains(&item_entity) {
                        reservation::release_source(ctx, item_entity, 1);
                        commands
                            .entity(item_entity)
                            .remove::<crate::relationships::DeliveringTo>();
                    }
                }
        info!(
            "WB_HAUL: {} of {} items missing, released reservations",
            total_count - loaded_count,
            total_count
        );
    }

    *ctx.task = AssignedTask::HaulWithWheelbarrow(HaulWithWheelbarrowData {
        phase: HaulWithWheelbarrowPhase::GoingToDestination,
        ..data
    });

    info!(
        "WB_HAUL: Soul {:?} loaded {} items into wheelbarrow",
        ctx.soul_entity, loaded_count
    );
}

fn handle_going_to_destination(
    ctx: &mut TaskExecutionContext,
    data: HaulWithWheelbarrowData,
    commands: &mut Commands,
    world_map: &Res<WorldMap>,
    soul_pos: Vec2,
) {
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
            let Ok((_, blueprint, _)) = ctx.queries.storage.blueprints.get(blueprint_entity) else {
                info!("WB_HAUL: Destination blueprint destroyed, dropping items");
                drop_items_and_cancel(ctx, &data, commands);
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
                info!("WB_HAUL: Destination mixer not found, dropping items");
                drop_items_and_cancel(ctx, &data, commands);
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
            (
                reachable,
                is_near_target_or_dest(soul_pos, mixer_pos, ctx.dest.0),
            )
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

fn handle_unloading(
    ctx: &mut TaskExecutionContext,
    data: HaulWithWheelbarrowData,
    commands: &mut Commands,
    soul_pos: Vec2,
) {
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
            if let Ok((_, stock_transform, mut stockpile_comp, stored_items_opt)) =
                ctx.queries.storage.stockpiles.get_mut(dest_stockpile)
            {
                let stock_pos = stock_transform.translation;
                let incoming = ctx
                    .queries
                    .reservation
                    .incoming_deliveries_query
                    .get(dest_stockpile)
                    .ok()
                    .map(|inc: &crate::relationships::IncomingDeliveries| inc.len())
                    .unwrap_or(0);
                let current_count = stored_items_opt.map(|si| si.len()).unwrap_or(0);

                for (item_entity, res_type_opt) in &item_types {
                    // 現在の在庫 + 搬入中（自分を含む）が容量を超えないようにする
                    // すでに自分たちが積んでいる分は incoming に含まれているはずなので、
                    // ここでの unloaded_count 加算は重複になる可能性があるが、
                    // 同一フレーム内の安全策として unloaded_count も考慮する。
                    if current_count + incoming + unloaded_count >= stockpile_comp.capacity {
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
                    if !res_type.can_store_in_stockpile() {
                        continue;
                    }

                    commands.entity(*item_entity).insert((
                        Visibility::Visible,
                        Transform::from_xyz(stock_pos.x, stock_pos.y, Z_ITEM_PICKUP),
                        crate::relationships::StoredIn(dest_stockpile),
                        InStockpile(dest_stockpile),
                    ));
                    commands.entity(*item_entity).remove::<LoadedIn>();
                    commands.entity(*item_entity).remove::<crate::relationships::DeliveringTo>();
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
                    // DeliveringTo is removed with despawn
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
                // Blueprint が destroyed されていた場合、アイテムを地面にドロップ
                info!("WB_HAUL: Blueprint destroyed during unloading, dropping items");
                drop_items_and_cancel(ctx, &data, commands);
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
                        // DeliveringTo is removed with despawn
                        unloaded_count += 1;
                    } else {
                        // 溢れ時は地面にドロップ
                        commands.entity(*item_entity).remove::<LoadedIn>();
                        commands.entity(*item_entity).insert((
                            Visibility::Visible,
                            Transform::from_xyz(soul_pos.x, soul_pos.y, Z_ITEM_PICKUP),
                        ));
                    }

                    mixer_release_types.push(res_type);
                }
            } else {
                // Mixer が破壊されていた場合、アイテムを地面にドロップ
                info!("WB_HAUL: Mixer destroyed during unloading, dropping items");
                drop_items_and_cancel(ctx, &data, commands);
                return;
            }
        }
    }

    // 予約解放
    match data.destination {
        WheelbarrowDestination::Stockpile(target) | WheelbarrowDestination::Blueprint(target) => {
            for _ in 0..destination_store_count {
                reservation::record_stored_destination(ctx, target);
            }
        }
        WheelbarrowDestination::Mixer {
            entity: target, ..
        } => {
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

fn handle_returning_wheelbarrow(
    ctx: &mut TaskExecutionContext,
    data: HaulWithWheelbarrowData,
    commands: &mut Commands,
    world_map: &Res<WorldMap>,
    q_wheelbarrows: &Query<(&Transform, Option<&ParkedAt>), With<Wheelbarrow>>,
    soul_pos: Vec2,
) {
    let Ok(_) = q_wheelbarrows.get(data.wheelbarrow) else {
        // 手押し車消失
        reservation::release_source(ctx, data.wheelbarrow, 1);
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
        reservation::release_source(ctx, data.wheelbarrow, 1);
        wheelbarrow_common::complete_wheelbarrow_task(commands, ctx, &data, soul_pos);
        info!(
            "WB_HAUL: Soul {:?} returned wheelbarrow {:?} (unreachable, parked here)",
            ctx.soul_entity, data.wheelbarrow
        );
        return;
    }

    if is_near_target(soul_pos, parking_pos) {
        reservation::release_source(ctx, data.wheelbarrow, 1);
        wheelbarrow_common::complete_wheelbarrow_task(commands, ctx, &data, parking_pos);
        info!(
            "WB_HAUL: Soul {:?} returned wheelbarrow {:?}",
            ctx.soul_entity, data.wheelbarrow
        );
    }
}

// --- キャンセル処理 ---

/// 手押し車タスクのキャンセル処理（全フェーズ共通）
/// 積載済みアイテムを地面にドロップし、猫車を駐車に戻し、全予約を解放する。
fn cancel_wheelbarrow_task(
    ctx: &mut TaskExecutionContext,
    data: &HaulWithWheelbarrowData,
    commands: &mut Commands,
) {
    let soul_pos = ctx.soul_pos();
    // 積載済みアイテムを地面にドロップ
    if let Some(loaded_items) = ctx.queries.storage.loaded_items.get(data.wheelbarrow).ok() {
        for item_entity in loaded_items.iter() {
            commands.entity(item_entity).insert((
                Visibility::Visible,
                Transform::from_xyz(soul_pos.x, soul_pos.y, Z_ITEM_PICKUP),
            ));
            commands
                .entity(item_entity)
                .remove::<crate::relationships::DeliveringTo>();
            commands.entity(item_entity).remove::<LoadedIn>();
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

    // 全予約を解放
    release_all_reservations(ctx, data);

    ctx.inventory.0 = None;
    commands.entity(ctx.soul_entity).remove::<WorkingOn>();
    clear_task_and_path(ctx.task, ctx.path);

    info!(
        "WB_HAUL: Soul {:?} canceled wheelbarrow task",
        ctx.soul_entity
    );
}

/// 宛先が破壊された場合: アイテムを地面にドロップしてキャンセル
/// cancel_wheelbarrow_task と同等だが、宛先の予約も確実に解放する。
fn drop_items_and_cancel(
    ctx: &mut TaskExecutionContext,
    data: &HaulWithWheelbarrowData,
    commands: &mut Commands,
) {
    cancel_wheelbarrow_task(ctx, data, commands);
}

/// 全アイテムの予約（ソース + 宛先）を解放
fn release_all_reservations(ctx: &mut TaskExecutionContext, data: &HaulWithWheelbarrowData) {
    reservation::release_source(ctx, data.wheelbarrow, 1);

    if let Some(source_entity) = data.collect_source {
        reservation::release_source(ctx, source_entity, 1);
    }

    for &item_entity in &data.items {
        reservation::release_source(ctx, item_entity, 1);

        match data.destination {
            WheelbarrowDestination::Stockpile(_) | WheelbarrowDestination::Blueprint(_) => {
                // DeliveringTo リレーションシップの削除は
                // cancel_wheelbarrow_task や unload 各所で行われる。
                // 旧予約キャッシュ(Hashmap)にはもう destination_reservations が存在しないため、
                // ここでの Stockpile/Blueprint 向け release_destination は不要（かつエラー）。
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
}
