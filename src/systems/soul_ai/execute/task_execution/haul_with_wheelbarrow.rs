//! 手押し車による一括運搬タスクの実行処理

use crate::constants::*;
use crate::relationships::{LoadedIn, ParkedAt, PushedBy, WorkingOn};
use crate::systems::logistics::{InStockpile, Wheelbarrow};
use crate::systems::soul_ai::execute::task_execution::common::*;
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
                    let Ok((_, _, _, _, _, stored_in_opt)) =
                        ctx.queries.designation.targets.get(item_entity)
                    else {
                        return None;
                    };
                    Some((item_entity, stored_in_opt.map(|si| si.0)))
                })
                .collect();

            // 収集した情報を使ってアイテムを積み込む
            for (item_entity, stored_in_stockpile) in &items_to_load {
                commands.entity(*item_entity).insert((
                    Visibility::Hidden,
                    LoadedIn(data.wheelbarrow),
                ));
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
                    update_stockpile_on_item_removal(
                        *stock_entity,
                        &mut ctx.queries.storage.stockpiles,
                    );
                }

                ctx.queue_reservation(
                    crate::events::ResourceReservationOp::RecordPickedSource {
                        source: *item_entity,
                        amount: 1,
                    },
                );
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
            // 目的地ストックパイルへ移動（速度ペナルティは movement system で適用）
            let q_stockpiles = &ctx.queries.storage.stockpiles;
            let Ok((_, stock_transform, _, _)) = q_stockpiles.get(data.dest_stockpile) else {
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

            if !reachable {
                cancel_wheelbarrow_task(ctx, &data, commands);
                return;
            }

            if is_near_target(soul_pos, stock_pos) {
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
                    let Ok((_, _, _, res_item_opt, _, _)) =
                        ctx.queries.designation.targets.get(item_entity)
                    else {
                        return None;
                    };
                    Some((item_entity, res_item_opt.map(|r| r.0)))
                })
                .collect();

            // ストックパイルの情報を取得して荷下ろし
            let mut unloaded_items: Vec<Entity> = Vec::new();
            if let Ok((_, stock_transform, mut stockpile_comp, stored_items_opt)) =
                ctx.queries.storage.stockpiles.get_mut(data.dest_stockpile)
            {
                let stock_pos = stock_transform.translation;
                let current_count = stored_items_opt.map(|si| si.len()).unwrap_or(0);

                for (item_entity, res_type_opt) in &item_types {
                    if current_count + unloaded_items.len() >= stockpile_comp.capacity {
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
                        crate::relationships::StoredIn(data.dest_stockpile),
                        InStockpile(data.dest_stockpile),
                    ));
                    commands.entity(*item_entity).remove::<LoadedIn>();
                    commands
                        .entity(*item_entity)
                        .remove::<crate::systems::jobs::IssuedBy>();
                    commands
                        .entity(*item_entity)
                        .remove::<crate::relationships::TaskWorkers>();

                    unloaded_items.push(*item_entity);
                }
            }

            // 予約操作（borrowing conflict 回避のためループ外）
            for _ in &unloaded_items {
                ctx.queue_reservation(
                    crate::events::ResourceReservationOp::RecordStoredDestination {
                        target: data.dest_stockpile,
                    },
                );
            }

            info!(
                "WB_HAUL: Soul {:?} unloaded {} items",
                ctx.soul_entity,
                unloaded_items.len()
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
                        .map(|(tf, _, _, _, _, _)| tf.translation.truncate())
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
                // 到達不能: 現在位置に駐車
                park_wheelbarrow_here(commands, ctx, &data, soul_pos);
                return;
            }

            if is_near_target(soul_pos, parking_pos) {
                park_wheelbarrow_here(commands, ctx, &data, parking_pos);
            }
        }
    }
}

/// 手押し車を現在位置に駐車してタスクを完了
fn park_wheelbarrow_here(
    commands: &mut Commands,
    ctx: &mut TaskExecutionContext,
    data: &HaulWithWheelbarrowData,
    pos: Vec2,
) {
    // 駐車エリアを取得して ParkedAt を設定
    if let Ok(belongs) = ctx.queries.designation.belongs.get(data.wheelbarrow) {
        commands
            .entity(data.wheelbarrow)
            .insert(ParkedAt(belongs.0));
    }

    // PushedBy を削除
    commands.entity(data.wheelbarrow).remove::<PushedBy>();

    // 手押し車の位置を更新
    commands.entity(data.wheelbarrow).insert((
        Visibility::Visible,
        Transform::from_xyz(pos.x, pos.y, Z_ITEM_PICKUP),
    ));

    // インベントリクリア
    ctx.inventory.0 = None;
    commands.entity(ctx.soul_entity).remove::<WorkingOn>();
    clear_task_and_path(ctx.task, ctx.path);

    info!(
        "WB_HAUL: Soul {:?} returned wheelbarrow {:?}",
        ctx.soul_entity, data.wheelbarrow
    );
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
    if let Ok(belongs) = ctx.queries.designation.belongs.get(data.wheelbarrow) {
        commands
            .entity(data.wheelbarrow)
            .insert(ParkedAt(belongs.0));
    }
    commands.entity(data.wheelbarrow).remove::<PushedBy>();
    commands.entity(data.wheelbarrow).insert((
        Visibility::Visible,
        Transform::from_xyz(soul_pos.x, soul_pos.y, Z_ITEM_PICKUP),
    ));

    // 予約解放
    for &item_entity in &data.items {
        ctx.queue_reservation(crate::events::ResourceReservationOp::ReleaseSource {
            source: item_entity,
            amount: 1,
        });
        ctx.queue_reservation(
            crate::events::ResourceReservationOp::ReleaseDestination {
                target: data.dest_stockpile,
            },
        );
    }

    ctx.inventory.0 = None;
    commands.entity(ctx.soul_entity).remove::<WorkingOn>();
    clear_task_and_path(ctx.task, ctx.path);

    info!(
        "WB_HAUL: Soul {:?} canceled wheelbarrow task",
        ctx.soul_entity
    );
}
