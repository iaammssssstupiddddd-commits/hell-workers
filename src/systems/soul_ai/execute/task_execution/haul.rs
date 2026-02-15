//! 運搬タスクの実行処理（ストックパイルへ）

use crate::relationships::WorkingOn;
use crate::systems::soul_ai::execute::task_execution::common::*;
use crate::systems::soul_ai::execute::task_execution::transport_common::{
    cancel, reservation,
};
use crate::systems::soul_ai::execute::task_execution::{
    context::TaskExecutionContext,
    types::{AssignedTask, HaulPhase},
};
use crate::systems::soul_ai::helpers::work::unassign_task;
use crate::world::map::WorldMap;
use bevy::prelude::*;

pub fn handle_haul_task(
    ctx: &mut TaskExecutionContext,
    item: Entity,
    stockpile: Entity,
    phase: HaulPhase,
    commands: &mut Commands,
    // haul_cache is now accessed via ctx.queries.resource_cache
    world_map: &Res<WorldMap>,
) {
    let soul_pos = ctx.soul_pos();
    let q_targets = &ctx.queries.designation.targets;
    let q_belongs = &ctx.queries.designation.belongs;
    match phase {
        HaulPhase::GoingToItem => {
            if let Ok((res_transform, _, _, _, _res_item_opt, _, stored_in_opt)) =
                q_targets.get(item)
            {
                let res_pos = res_transform.translation.truncate();
                let stored_in_entity = stored_in_opt.map(|stored_in| stored_in.0);
                // アイテムが障害物の上にある可能性があるため、隣接マスを目的地として設定
                let reachable = update_destination_to_adjacent(
                    ctx.dest,
                    res_pos,
                    ctx.path,
                    soul_pos,
                    world_map,
                    ctx.pf_context,
                );

                if !reachable {
                    info!(
                        "HAUL: Soul {:?} cannot reach item {:?}, canceling",
                        ctx.soul_entity, item
                    );
                    cancel::cancel_haul_to_stockpile(ctx, item, stockpile);
                    return;
                }

                let is_near = can_pickup_item(soul_pos, res_pos);

                if is_near {
                    if !try_pickup_item(
                        commands,
                        ctx.soul_entity,
                        item,
                        ctx.inventory,
                        soul_pos,
                        res_pos,
                        ctx.task,
                        ctx.path,
                    ) {
                        return;
                    }
                    release_mixer_mud_storage_for_item(ctx, item, commands);

                    if let Some(stored_in) = stored_in_entity {
                        update_stockpile_on_item_removal(stored_in, &mut ctx.queries.storage.stockpiles);
                    }

                    if let Ok((_, stock_transform, _, _)) = ctx.queries.storage.stockpiles.get(stockpile) {
                        let stock_pos = stock_transform.translation.truncate();
                        let stock_grid = WorldMap::world_to_grid(stock_pos);
                        let stock_dest = WorldMap::grid_to_world(stock_grid.0, stock_grid.1);
                        ctx.path.waypoints.clear();
                        update_destination_if_needed(ctx.dest, stock_dest, ctx.path);
                    }

                    *ctx.task = AssignedTask::Haul(
                        crate::systems::soul_ai::execute::task_execution::types::HaulData {
                            item,
                            stockpile,
                            phase: HaulPhase::GoingToStockpile,
                        },
                    );
                    reservation::record_picked_source(ctx, item, 1);
                    info!("HAUL: Soul {:?} picked up item {:?}", ctx.soul_entity, item);
                }
            } else {
                cancel::cancel_haul_to_stockpile(ctx, item, stockpile);
            }
        }
        HaulPhase::GoingToStockpile => {
            if let Ok((_, stock_transform, _, _)) = ctx.queries.storage.stockpiles.get(stockpile) {
                let stock_pos = stock_transform.translation.truncate();
                let stock_grid = WorldMap::world_to_grid(stock_pos);
                let stock_dest = WorldMap::grid_to_world(stock_grid.0, stock_grid.1);
                update_destination_if_needed(ctx.dest, stock_dest, ctx.path);

                if is_near_target(soul_pos, stock_pos) {
                    *ctx.task = AssignedTask::Haul(
                        crate::systems::soul_ai::execute::task_execution::types::HaulData {
                            item,
                            stockpile,
                            phase: HaulPhase::Dropping,
                        },
                    );
                    ctx.path.waypoints.clear();
                }
            } else {
                warn!(
                    "HAUL: Soul {:?} stockpile {:?} not found",
                    ctx.soul_entity, stockpile
                );
                if let Some(held_item_entity) = ctx.inventory.0 {
                    commands
                        .entity(held_item_entity)
                        .insert(Visibility::Visible);
                }
                ctx.inventory.0 = None;
                commands.entity(ctx.soul_entity).remove::<WorkingOn>();
                clear_task_and_path(ctx.task, ctx.path);
            }
        }
        HaulPhase::Dropping => {
            if let Ok((_, stock_transform, mut stockpile_comp, stored_items_opt)) =
                ctx.queries.storage.stockpiles.get_mut(stockpile)
            {
                let current_count = stored_items_opt.map(|si| si.len()).unwrap_or(0);
                let is_bucket_storage = ctx.queries.storage.bucket_storages.get(stockpile).is_ok();

                // アイテムの型と所有権を取得
                let item_info = q_targets.get(item).ok().map(|(_, _, _, _, ri, _, _)| {
                    let res_type = ri.map(|r| r.0);
                    let belongs = q_belongs.get(item).ok().map(|b| b.0);
                    (res_type, belongs)
                });
                let can_drop = if let Some((Some(res_type), item_belongs)) = item_info {
                    // 所有権チェック
                    let stock_belongs = q_belongs.get(stockpile).ok().map(|b| b.0);
                    let belongs_match = item_belongs == stock_belongs;

                    let is_bucket_item = matches!(
                        res_type,
                        crate::systems::logistics::ResourceType::BucketEmpty
                            | crate::systems::logistics::ResourceType::BucketWater
                    );
                    let type_match = stockpile_comp.resource_type.is_none()
                        || stockpile_comp.resource_type == Some(res_type);

                    let ownership_ok = if is_bucket_storage {
                        stock_belongs.is_some() && item_belongs.is_some() && belongs_match
                    } else {
                        belongs_match
                    };

                    let type_allowed = if is_bucket_storage {
                        let bucket_storage_type_ok = matches!(
                            stockpile_comp.resource_type,
                            None | Some(crate::systems::logistics::ResourceType::BucketEmpty)
                                | Some(crate::systems::logistics::ResourceType::BucketWater)
                        );
                        is_bucket_item && bucket_storage_type_ok
                    } else {
                        type_match && res_type.can_store_in_stockpile()
                    };

                    // IncomingDeliveries の長さ（＝このセルに向かっているアイテム数）を取得
                    let incoming_count = ctx.queries.reservation.incoming_deliveries_query.get(stockpile).ok()
                        .map(|incoming: &crate::relationships::IncomingDeliveries| incoming.len())
                        .unwrap_or(0);
                    
                    // すでに自分の分が IncomingDeliveries に含まれているため、
                    // (現在庫 + 搬入予定数) <= capacity で満杯チェック
                    let capacity_ok = (current_count + incoming_count) <= stockpile_comp.capacity;

                    ownership_ok && type_allowed && capacity_ok
                } else {
                    false
                };

                if can_drop {
                    // 資源タイプがNoneなら設定
                    if !is_bucket_storage && stockpile_comp.resource_type.is_none() {
                        if let Some((res_type, _)) = item_info {
                            stockpile_comp.resource_type = res_type;
                        }
                    }

                    commands.entity(item).insert((
                        Visibility::Visible,
                        Transform::from_xyz(
                            stock_transform.translation.x,
                            stock_transform.translation.y,
                            0.6,
                        ),
                        crate::relationships::StoredIn(stockpile),
                        crate::systems::logistics::InStockpile(stockpile),
                    ));
                    commands.entity(item).remove::<crate::relationships::DeliveringTo>();
                    // タスク完了: ManagedTasks を肥大化させないため、管理者を解除する
                    commands
                        .entity(item)
                        .remove::<crate::systems::jobs::IssuedBy>();
                    commands
                        .entity(item)
                        .remove::<crate::relationships::TaskWorkers>();

                    reservation::record_stored_destination(ctx, stockpile);

                    info!(
                        "TASK_EXEC: Soul {:?} dropped item at stockpile. Count ~ {}",
                        ctx.soul_entity,
                        current_count // 正確な数はnext frameだが
                    );
                } else {
                    // 到着時に条件を満たさなくなった場合（型違いor満杯）
                    // 片付けタスクを再発行してドロップ
                    // unassign_task 内で release_destination が呼ばれるべきだが、
                    // ここで haul_cache.release_destination を読んでしまうと unassign_task で二重解放になる？
                    // unassign_task は AssignedTask を見て判断する。
                    // 今は Haul(Dropping) なので、unassign_task は release_destination を呼ぶ。
                    // なのでここでは何もしなくていい。
                    unassign_task(
                        commands,
                        ctx.soul_entity,
                        soul_pos,
                        ctx.task,
                        ctx.path,
                        Some(ctx.inventory),
                        item_info.and_then(|(it, _)| it),
                        ctx.queries,
                        // haul_cache removed
                        world_map,
                        true,
                    );
                }
            } else {
                // 備蓄場所消失
                if ctx.inventory.0.is_some() {
                    drop_item(commands, ctx.soul_entity, item, soul_pos);
                }
            }

            ctx.inventory.0 = None;
            commands.entity(ctx.soul_entity).remove::<WorkingOn>();
            clear_task_and_path(ctx.task, ctx.path);
            ctx.soul.fatigue = (ctx.soul.fatigue + 0.05).min(1.0);

            // 搬送予約を解放
            // ドロップ成功時に record_stored しているので、ここでは呼ばない！
            // record_stored していない場合（= drop失敗時やStockpile消失時）は release_destination する必要があるが...
            // 上記 else ブロック（消失）では release_destination している。
            // 正常終了時は既に record_stored 済みなので何もしない、と言いたいが
            // Dropping フェーズが終わる＝タスク完了。
            // もし record_stored で release 済みなら二重解放になる。
            // release_destination は 0未満にならないようになっているので安全ではある。
            // しかし、コードフロー的にここを通るのは「ドロップ完了後」または「キャンセル後」。
            // Dropping フェーズ内での分岐で処理済みなら不要。
            // ここでは念のため release_destination を呼んでおくのが無難か？いや、record_stored で消えているはず。
            // 余計な処理はしない。
        }
    }
}
