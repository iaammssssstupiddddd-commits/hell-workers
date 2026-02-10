use crate::constants::*;
use crate::systems::familiar_ai::decide::task_management::{AssignTaskContext, ReservationShadow};
use crate::systems::jobs::WorkType;
use crate::systems::logistics::transport_request::TransportRequestKind;
use crate::systems::logistics::ResourceType;
use bevy::prelude::*;

use super::super::builders::{
    issue_haul_to_blueprint, issue_haul_to_blueprint_with_source, issue_haul_to_mixer,
    issue_haul_to_stockpile, issue_haul_to_stockpile_with_source, issue_haul_with_wheelbarrow,
};
use super::super::validator::{
    compute_centroid, find_best_stockpile_for_item, resolve_haul_to_blueprint_inputs,
    resolve_haul_to_mixer_inputs, resolve_haul_to_stockpile_inputs,
    resolve_wheelbarrow_batch_for_stockpile, source_not_reserved,
};

pub(super) fn assign_haul_to_mixer(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    let Some((mixer_entity, item_type)) = resolve_haul_to_mixer_inputs(ctx.task_entity, queries)
    else {
        debug!(
            "ASSIGN: HaulToMixer task {:?} has no TargetMixer",
            ctx.task_entity
        );
        return false;
    };

    let is_request_task = queries
        .transport_requests
        .get(ctx.task_entity)
        .is_ok_and(|req| matches!(req.kind, TransportRequestKind::DeliverToMixerSolid));
    let (source_item, source_pos) = if is_request_task {
        let Some((source, pos)) =
            find_nearest_mixer_source_item(item_type, task_pos, queries, shadow)
        else {
            debug!(
                "ASSIGN: HaulToMixer request {:?} has no available {:?} source",
                ctx.task_entity, item_type
            );
            return false;
        };
        (source, pos)
    } else {
        if !source_not_reserved(ctx.task_entity, queries, shadow) {
            debug!(
                "ASSIGN: HaulToMixer item {:?} is already reserved",
                ctx.task_entity
            );
            return false;
        }
        (ctx.task_entity, task_pos)
    };

    let mixer_already_reserved =
        !is_request_task && queries.reserved_for_task.get(ctx.task_entity).is_ok();
    let can_accept = if let Ok((_, storage, _)) = queries.storage.mixers.get(mixer_entity) {
        let reserved = queries
            .reservation
            .resource_cache
            .get_mixer_destination_reservation(mixer_entity, item_type)
            + shadow.mixer_reserved(mixer_entity, item_type);
        // 既に発行時に予約済みなら、割り当て時は追加1件を見込まない
        let required = if mixer_already_reserved {
            reserved as u32
        } else {
            (reserved + 1) as u32
        };
        storage.can_accept(item_type, required)
    } else {
        false
    };

    if !can_accept {
        debug!(
            "ASSIGN: Mixer {:?} cannot accept item {:?} (Full or Reserved)",
            mixer_entity, item_type
        );
        return false;
    }

    issue_haul_to_mixer(
        source_item,
        mixer_entity,
        item_type,
        mixer_already_reserved,
        source_pos,
        already_commanded,
        ctx,
        queries,
        shadow,
    );
    true
}

fn find_nearest_mixer_source_item(
    item_type: ResourceType,
    mixer_pos: Vec2,
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> Option<(Entity, Vec2)> {
    queries
        .free_resource_items
        .iter()
        .filter(|(_, _, visibility, res_item)| {
            **visibility != Visibility::Hidden && res_item.0 == item_type
        })
        .filter(|(entity, _, _, _)| source_not_reserved(*entity, queries, shadow))
        .min_by(|(_, t1, _, _), (_, t2, _, _)| {
            let d1 = t1.translation.truncate().distance_squared(mixer_pos);
            let d2 = t2.translation.truncate().distance_squared(mixer_pos);
            d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(e, t, _, _)| (e, t.translation.truncate()))
}

fn find_nearest_stockpile_source_item(
    resource_type: ResourceType,
    item_owner: Option<Entity>,
    stock_pos: Vec2,
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> Option<(Entity, Vec2)> {
    queries
        .free_resource_items
        .iter()
        .filter(|(_, _, visibility, res_item)| {
            **visibility != Visibility::Hidden && res_item.0 == resource_type
        })
        .filter(|(entity, _, _, _)| source_not_reserved(*entity, queries, shadow))
        .filter(|(entity, _, _, _)| {
            let belongs = queries.designation.belongs.get(*entity).ok().map(|b| b.0);
            item_owner == belongs
        })
        .min_by(|(_, t1, _, _), (_, t2, _, _)| {
            let d1 = t1.translation.truncate().distance_squared(stock_pos);
            let d2 = t2.translation.truncate().distance_squared(stock_pos);
            d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(e, t, _, _)| (e, t.translation.truncate()))
}

fn find_nearest_blueprint_source_item(
    resource_type: ResourceType,
    bp_pos: Vec2,
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> Option<(Entity, Vec2)> {
    queries
        .free_resource_items
        .iter()
        .filter(|(_, _, visibility, res_item)| {
            **visibility != Visibility::Hidden && res_item.0 == resource_type
        })
        .filter(|(entity, _, _, _)| source_not_reserved(*entity, queries, shadow))
        .min_by(|(_, t1, _, _), (_, t2, _, _)| {
            let d1 = t1.translation.truncate().distance_squared(bp_pos);
            let d2 = t2.translation.truncate().distance_squared(bp_pos);
            d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(e, t, _, _)| (e, t.translation.truncate()))
}

pub(super) fn assign_haul(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    if let Some((blueprint, resource_type)) =
        resolve_haul_to_blueprint_inputs(ctx.task_entity, queries)
    {
        let is_request_task = queries
            .transport_requests
            .get(ctx.task_entity)
            .is_ok_and(|req| matches!(req.kind, TransportRequestKind::DeliverToBlueprint));

        if is_request_task {
            let Some((source_item, source_pos)) =
                find_nearest_blueprint_source_item(resource_type, task_pos, queries, shadow)
            else {
                debug!(
                    "ASSIGN: Blueprint request {:?} has no available {:?} source",
                    ctx.task_entity, resource_type
                );
                return false;
            };
            issue_haul_to_blueprint_with_source(
                source_item,
                blueprint,
                source_pos,
                already_commanded,
                ctx,
                queries,
                shadow,
            );
        } else {
            if !source_not_reserved(ctx.task_entity, queries, shadow) {
                debug!(
                    "ASSIGN: Item {:?} (for BP) is already reserved",
                    ctx.task_entity
                );
                return false;
            }
            issue_haul_to_blueprint(blueprint, task_pos, already_commanded, ctx, queries, shadow);
        }
        return true;
    }

    if let Some((stockpile, resource_type, item_owner)) =
        resolve_haul_to_stockpile_inputs(ctx.task_entity, queries)
    {
        // M6: 手押し車バッチを優先（request resolver で遅延解決）
        if let Some((wb_entity, items)) = resolve_wheelbarrow_batch_for_stockpile(
            stockpile,
            resource_type,
            item_owner,
            task_pos,
            queries,
            shadow,
        ) {
            let source_pos = compute_centroid(&items, queries);
            issue_haul_with_wheelbarrow(
                wb_entity,
                source_pos,
                stockpile,
                items,
                task_pos,
                already_commanded,
                ctx,
                queries,
                shadow,
            );
            return true;
        }

        let Some((source_item, source_pos)) = find_nearest_stockpile_source_item(
            resource_type,
            item_owner,
            task_pos,
            queries,
            shadow,
        ) else {
            debug!(
                "ASSIGN: Stockpile request {:?} has no available {:?} source",
                ctx.task_entity, resource_type
            );
            return false;
        };
        issue_haul_to_stockpile_with_source(
            source_item,
            stockpile,
            source_pos,
            already_commanded,
            ctx,
            queries,
            shadow,
        );
        return true;
    }

    if !source_not_reserved(ctx.task_entity, queries, shadow) {
        debug!("ASSIGN: Item {:?} is already reserved", ctx.task_entity);
        return false;
    }

    let item_info = queries.items.get(ctx.task_entity).ok().map(|(it, _)| it.0);
    let item_owner = queries
        .designation
        .belongs
        .get(ctx.task_entity)
        .ok()
        .map(|b| b.0);

    let Some(item_type) = item_info else {
        debug!(
            "ASSIGN: Haul item {:?} has no ResourceItem",
            ctx.task_entity
        );
        return false;
    };

    let best_stockpile = find_best_stockpile_for_item(
        task_pos,
        ctx.task_area_opt,
        item_type,
        item_owner,
        queries,
        shadow,
    );

    let Some(stock_entity) = best_stockpile else {
        debug!(
            "ASSIGN: No suitable stockpile found for item {:?} (type: {:?})",
            ctx.task_entity, item_type
        );
        return false;
    };

    // 手押し車による一括運搬を検討（アイテム直接 Designation 経路）
    if item_type.is_loadable() {
        if let Some(wb_entity) = find_nearest_wheelbarrow(task_pos, queries, shadow) {
            let batch_items =
                collect_nearby_haulable_items(ctx.task_entity, task_pos, queries, shadow);

            if batch_items.len() >= WHEELBARROW_MIN_BATCH_SIZE {
                let dest_capacity = remaining_stockpile_capacity(stock_entity, queries, shadow);
                let max_items = dest_capacity.min(WHEELBARROW_CAPACITY);
                let items: Vec<Entity> = batch_items.into_iter().take(max_items).collect();

                if items.len() >= WHEELBARROW_MIN_BATCH_SIZE {
                    let source_pos = compute_centroid(&items, queries);

                    issue_haul_with_wheelbarrow(
                        wb_entity,
                        source_pos,
                        stock_entity,
                        items,
                        task_pos,
                        already_commanded,
                        ctx,
                        queries,
                        shadow,
                    );
                    return true;
                }
            }
        }
    }

    // 通常の運搬
    issue_haul_to_stockpile(
        stock_entity,
        task_pos,
        already_commanded,
        ctx,
        queries,
        shadow,
    );
    true
}

/// タスク位置に最も近い利用可能な手押し車を検索
fn find_nearest_wheelbarrow(
    task_pos: Vec2,
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> Option<Entity> {
    queries
        .wheelbarrows
        .iter()
        .filter(|(wb_entity, _)| source_not_reserved(*wb_entity, queries, shadow))
        .min_by(|(_, t1), (_, t2)| {
            let d1 = t1.translation.truncate().distance_squared(task_pos);
            let d2 = t2.translation.truncate().distance_squared(task_pos);
            d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(e, _)| e)
}

/// 指定アイテムの近くにある、手押し車に積載可能な未予約 Haul アイテムを収集
fn collect_nearby_haulable_items(
    primary_item: Entity,
    task_pos: Vec2,
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> Vec<Entity> {
    let search_radius_sq = (TILE_SIZE * 10.0) * (TILE_SIZE * 10.0);

    let mut items: Vec<(Entity, f32)> = queries
        .designation
        .designations
        .iter()
        .filter_map(|(entity, transform, designation, _, _, task_workers, _, _)| {
            // Haul タスクのみ
            if designation.work_type != WorkType::Haul {
                return None;
            }
            // 既にワーカーが付いているものは除外
            if task_workers.is_some_and(|tw| !tw.is_empty()) {
                return None;
            }
            // 予約済みは除外
            if !source_not_reserved(entity, queries, shadow) {
                return None;
            }
            // 積載可能か確認
            let item_type = queries.items.get(entity).ok().map(|(it, _)| it.0)?;
            if !item_type.is_loadable() {
                return None;
            }
            // 距離チェック
            let pos = transform.translation.truncate();
            let dist_sq = pos.distance_squared(task_pos);
            if dist_sq > search_radius_sq {
                return None;
            }
            Some((entity, dist_sq))
        })
        .collect();

    // 近い順にソート
    items.sort_by(|(_, d1), (_, d2)| d1.partial_cmp(d2).unwrap_or(std::cmp::Ordering::Equal));

    // primary_item を先頭に確保
    let mut result: Vec<Entity> = Vec::new();
    result.push(primary_item);
    for (entity, _) in items {
        if entity == primary_item {
            continue;
        }
        result.push(entity);
    }

    result
}

/// ストックパイルの残り容量を計算
fn remaining_stockpile_capacity(
    stockpile: Entity,
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> usize {
    if let Ok((_, _, stock, stored)) = queries.storage.stockpiles.get(stockpile) {
        let current = stored.map(|s| s.len()).unwrap_or(0);
        let reserved = queries
            .reservation
            .resource_cache
            .get_destination_reservation(stockpile)
            + shadow.destination_reserved(stockpile);
        let used = current + reserved;
        if used >= stock.capacity {
            0
        } else {
            stock.capacity - used
        }
    } else {
        0
    }
}

