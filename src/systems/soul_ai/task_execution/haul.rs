//! 運搬タスクの実行処理（ストックパイルへ）

use crate::relationships::{Holding, WorkingOn};
use crate::systems::familiar_ai::haul_cache::HaulReservationCache;
use crate::systems::jobs::{Designation, IssuedBy, TaskSlots};
use crate::systems::logistics::Stockpile;
use crate::systems::soul_ai::task_execution::{
    common::*,
    context::TaskExecutionContext,
    types::{AssignedTask, HaulPhase},
};
use crate::world::map::WorldMap;
use bevy::prelude::*;

pub fn handle_haul_task(
    ctx: &mut TaskExecutionContext,
    holding: Option<&Holding>,
    item: Entity,
    stockpile: Entity,
    phase: HaulPhase,
    q_targets: &Query<(
        &Transform,
        Option<&crate::systems::jobs::Tree>,
        Option<&crate::systems::jobs::Rock>,
        Option<&crate::systems::logistics::ResourceItem>,
        Option<&Designation>,
        Option<&crate::relationships::StoredIn>,
    )>,
    q_stockpiles: &mut Query<(
        &Transform,
        &mut Stockpile,
        Option<&crate::relationships::StoredItems>,
    )>,
    commands: &mut Commands,
    dropped_this_frame: &mut std::collections::HashMap<Entity, usize>,
    haul_cache: &mut HaulReservationCache,
    world_map: &Res<WorldMap>,
) {
    let soul_pos = ctx.soul_pos();
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
                update_destination_to_adjacent(ctx.dest, res_pos, ctx.path, soul_pos, world_map);

                if is_near_target(soul_pos, res_pos) {
                    pickup_item(commands, ctx.soul_entity, item);

                    // もしアイテムが備蓄場所にあったなら、その備蓄場所の型管理を更新する
                    if let Some(stored_in) = stored_in_opt {
                        update_stockpile_on_item_removal(stored_in.0, q_stockpiles);
                    }

                    // 元のコンポーネントを削除
                    commands
                        .entity(item)
                        .remove::<crate::relationships::StoredIn>();
                    commands.entity(item).remove::<Designation>();
                    commands.entity(item).remove::<IssuedBy>();
                    commands.entity(item).remove::<TaskSlots>();

                    *ctx.task = AssignedTask::Haul {
                        item,
                        stockpile,
                        phase: HaulPhase::GoingToStockpile,
                    };
                    ctx.path.waypoints.clear();
                    info!("HAUL: Soul {:?} picked up item {:?}", ctx.soul_entity, item);
                }
            } else {
                clear_task_and_path(ctx.task, ctx.path);
                haul_cache.release(stockpile);
            }
        }
        HaulPhase::GoingToStockpile => {
            if let Ok((stock_transform, _, _)) = q_stockpiles.get(stockpile) {
                let stock_pos = stock_transform.translation.truncate();
                update_destination_to_adjacent(ctx.dest, stock_pos, ctx.path, soul_pos, world_map);

                if is_near_target(soul_pos, stock_pos) {
                    *ctx.task = AssignedTask::Haul {
                        item,
                        stockpile,
                        phase: HaulPhase::Dropping,
                    };
                    ctx.path.waypoints.clear();
                }
            } else {
                warn!(
                    "HAUL: Soul {:?} stockpile {:?} not found",
                    ctx.soul_entity, stockpile
                );
                if let Some(Holding(held_item_entity)) = holding {
                    commands
                        .entity(*held_item_entity)
                        .insert(Visibility::Visible);
                }
                commands.entity(ctx.soul_entity).remove::<Holding>();
                commands.entity(ctx.soul_entity).remove::<WorkingOn>();
                clear_task_and_path(ctx.task, ctx.path);
                haul_cache.release(stockpile);
            }
        }
        HaulPhase::Dropping => {
            if let Ok((stock_transform, mut stockpile_comp, stored_items_opt)) =
                q_stockpiles.get_mut(stockpile)
            {
                let current_count = stored_items_opt.map(|si| si.len()).unwrap_or(0);

                // アイテムの型を取得
                let item_type = q_targets
                    .get(item)
                    .ok()
                    .and_then(|(_, _, _, ri, _, _): (_, _, _, Option<&crate::systems::logistics::ResourceItem>, _, _)| ri.map(|r| r.0));
                let this_frame = dropped_this_frame.get(&stockpile).cloned().unwrap_or(0);

                let can_drop = if let Some(it) = item_type {
                    let type_match = stockpile_comp.resource_type.is_none()
                        || stockpile_comp.resource_type == Some(it);
                    // 現在の数 + このフレームですでに置かれた数
                    let capacity_ok = (current_count + this_frame) < stockpile_comp.capacity;
                    type_match && capacity_ok
                } else {
                    false
                };

                if can_drop {
                    // 資源タイプがNoneなら設定
                    if stockpile_comp.resource_type.is_none() {
                        stockpile_comp.resource_type = item_type;
                    }

                    commands.entity(item).insert((
                        Visibility::Visible,
                        Transform::from_xyz(
                            stock_transform.translation.x,
                            stock_transform.translation.y,
                            0.6,
                        ),
                        crate::relationships::StoredIn(stockpile),
                    ));

                    // カウンタを増やす
                    *dropped_this_frame.entry(stockpile).or_insert(0) += 1;

                    info!(
                        "TASK_EXEC: Soul {:?} dropped item at stockpile. New count: {}",
                        ctx.soul_entity,
                        current_count + this_frame + 1
                    );
                } else {
                    // 到着時に条件を満たさなくなった場合（型違いor満杯）
                    // 本来は代替地を探すべきだが、今回はシンプルにその場にドロップする
                    warn!("HAUL: Stockpile condition changed. Dropping item on the ground.");
                    drop_item(commands, ctx.soul_entity, item, soul_pos);
                }
            } else {
                // 備蓄場所消失
                if holding.is_some() {
                    drop_item(commands, ctx.soul_entity, item, soul_pos);
                }
            }

            commands.entity(ctx.soul_entity).remove::<Holding>();
            commands.entity(ctx.soul_entity).remove::<WorkingOn>();
            clear_task_and_path(ctx.task, ctx.path);
            ctx.soul.fatigue = (ctx.soul.fatigue + 0.05).min(1.0);

            // 搬送予約を解放
            haul_cache.release(stockpile);
        }
    }
}
