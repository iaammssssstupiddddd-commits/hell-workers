//! 運搬タスクの実行処理（ストックパイルへ）

use crate::relationships::WorkingOn;
use crate::systems::familiar_ai::haul_cache::HaulReservationCache;
use crate::systems::soul_ai::task_execution::{
    context::TaskExecutionContext,
    types::{AssignedTask, HaulPhase},
};
use crate::systems::soul_ai::task_execution::common::*;
use crate::systems::soul_ai::work::unassign_task;
use crate::world::map::WorldMap;
use bevy::prelude::*;

pub fn handle_haul_task(
    ctx: &mut TaskExecutionContext,
    item: Entity,
    stockpile: Entity,
    phase: HaulPhase,
    commands: &mut Commands,
    dropped_this_frame: &mut std::collections::HashMap<Entity, usize>,
    haul_cache: &mut HaulReservationCache,
    world_map: &Res<WorldMap>,
) {
    let soul_pos = ctx.soul_pos();
    let q_targets = &ctx.queries.targets;
    let q_stockpiles = &mut ctx.queries.stockpiles;
    let q_belongs = &ctx.queries.belongs;
    match phase {
        HaulPhase::GoingToItem => {
            if let Ok((res_transform, _, _, _res_item_opt, des_opt, stored_in_opt)) =
                q_targets.get(item)
            {
                // 指示がキャンセルされていないか確認
                if cancel_task_if_designation_missing(des_opt, ctx.task, ctx.path) {
                    haul_cache.release(stockpile);
                    return;
                }

                let res_pos = res_transform.translation.truncate();
                // アイテムが障害物の上にある可能性があるため、隣接マスを目的地として設定
                let reachable = update_destination_to_adjacent(ctx.dest, res_pos, ctx.path, soul_pos, world_map, ctx.pf_context);

                if !reachable {
                    // 到達不能: タスクをキャンセル
                    info!("HAUL: Soul {:?} cannot reach item {:?}, canceling", ctx.soul_entity, item);
                    haul_cache.release(stockpile);
                    clear_task_and_path(ctx.task, ctx.path);
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

                    // もしアイテムが備蓄場所にあったなら、その備蓄場所の型管理を更新する
                    if let Some(stored_in) = stored_in_opt {
                        update_stockpile_on_item_removal(stored_in.0, q_stockpiles);
                    }

                    // 管理コンポーネントの削除は pickup_item 内で行われる

                    // GoingToStockpileフェーズに移行する際、目的地を確実に更新する
                    // パスをクリアする前に目的地を更新することで、pathfinding_systemが正しい目的地に向かってパスを計算できるようにする
                    if let Ok((_, stock_transform, _, _)) = q_stockpiles.get(stockpile) {
                        let stock_pos = stock_transform.translation.truncate();
                        // 目的地はストックパイルの属するタイル中心にスナップしておく
                        let stock_grid = WorldMap::world_to_grid(stock_pos);
                        let stock_dest = WorldMap::grid_to_world(stock_grid.0, stock_grid.1);
                        // パスを先にクリアしてから目的地を更新することで、確実に目的地が更新される
                        ctx.path.waypoints.clear();
                        update_destination_if_needed(ctx.dest, stock_dest, ctx.path);
                    }

                    *ctx.task = AssignedTask::Haul(crate::systems::soul_ai::task_execution::types::HaulData {
                        item,
                        stockpile,
                        phase: HaulPhase::GoingToStockpile,
                    });
                    info!("HAUL: Soul {:?} picked up item {:?}", ctx.soul_entity, item);
                }
            } else {
                clear_task_and_path(ctx.task, ctx.path);
                haul_cache.release(stockpile);
            }
        }
        HaulPhase::GoingToStockpile => {
            if let Ok((_, stock_transform, _, _)) = q_stockpiles.get(stockpile) {
                let stock_pos = stock_transform.translation.truncate();
                let stock_grid = WorldMap::world_to_grid(stock_pos);
                let stock_dest = WorldMap::grid_to_world(stock_grid.0, stock_grid.1);
                update_destination_if_needed(ctx.dest, stock_dest, ctx.path);

                if is_near_target(soul_pos, stock_pos) {
                    *ctx.task = AssignedTask::Haul(crate::systems::soul_ai::task_execution::types::HaulData {
                        item,
                        stockpile,
                        phase: HaulPhase::Dropping,
                    });
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
                haul_cache.release(stockpile);
            }
        }
        HaulPhase::Dropping => {
            if let Ok((_, stock_transform, mut stockpile_comp, stored_items_opt)) =
                q_stockpiles.get_mut(stockpile)
            {
                let current_count = stored_items_opt.map(|si| si.len()).unwrap_or(0);

                // アイテムの型と所有権を取得
                let item_info = q_targets.get(item).ok().map(|(_, _, _, ri, _, _)| {
                    let res_type = ri.map(|r| r.0);
                    let belongs = q_belongs.get(item).ok();
                    (res_type, belongs)
                });
                let this_frame = dropped_this_frame.get(&stockpile).cloned().unwrap_or(0);

                    let can_drop = if let Some((Some(res_type), item_belongs)) = item_info {
                        // 所有権チェック
                        let stock_belongs = q_belongs.get(stockpile).ok();
                        let belongs_match = item_belongs == stock_belongs;

                        let type_match = stockpile_comp.resource_type.is_none()
                            || stockpile_comp.resource_type == Some(res_type);
                            
                        // 専用エリアの場合、型チェックを緩和（所有権が一致すれば空/満タンバケツ混在OK）
                        let type_allowed = if stock_belongs.is_some() {
                            belongs_match
                        } else {
                            type_match
                        };

                        // 現在の数 + このフレームですでに置かれた数
                        let capacity_ok = (current_count + this_frame) < stockpile_comp.capacity;
                        belongs_match && type_allowed && capacity_ok
                    } else {
                        false
                    };

                if can_drop {
                    // 資源タイプがNoneなら設定
                    if stockpile_comp.resource_type.is_none() {
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
                    // タスク完了: ManagedTasks を肥大化させないため、管理者を解除する
                    commands.entity(item).remove::<crate::systems::jobs::IssuedBy>();
                    commands.entity(item).remove::<crate::relationships::TaskWorkers>();

                    // カウンタを増やす
                    *dropped_this_frame.entry(stockpile).or_insert(0) += 1;

                    info!(
                        "TASK_EXEC: Soul {:?} dropped item at stockpile. New count: {}",
                        ctx.soul_entity,
                        current_count + this_frame + 1
                    );
                } else {
                    // 到着時に条件を満たさなくなった場合（型違いor満杯）
                    // 片付けタスクを再発行してドロップ
                    unassign_task(
                        commands,
                        ctx.soul_entity,
                        soul_pos,
                        ctx.task,
                        ctx.path,
                        Some(ctx.inventory),
                        item_info.and_then(|(it, _)| it),
                        &ctx.queries,
                        haul_cache,
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
            haul_cache.release(stockpile);
        }
    }
}
