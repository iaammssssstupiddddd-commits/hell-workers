//! Stockpile 向け運搬タスクの割り当て

use crate::constants::*;
use crate::systems::familiar_ai::decide::task_management::{AssignTaskContext, ReservationShadow};
use crate::systems::logistics::transport_request::{can_complete_pick_drop_to_point, WheelbarrowDestination};
use bevy::prelude::*;

use super::super::super::builders::{
    issue_haul_to_stockpile_with_source, issue_haul_with_wheelbarrow,
};
use super::super::super::validator::{
    compute_centroid, resolve_haul_to_stockpile_inputs, resolve_wheelbarrow_batch_for_stockpile,
};
use super::lease_validation;
use super::source_selector;
use super::wheelbarrow;

pub fn assign_haul_to_stockpile(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    let Some((stockpile, resource_type, item_owner, fixed_source)) =
        resolve_haul_to_stockpile_inputs(ctx.task_entity, queries, shadow)
    else {
        return false;
    };

    if let Some(fixed_source_item) = fixed_source {
        let Some((source_item, source_pos)) = source_selector::find_fixed_stockpile_source_item(
            fixed_source_item,
            resource_type,
            item_owner,
            queries,
            shadow,
        ) else {
            debug!(
                "ASSIGN: Manual stockpile request {:?} fixed source {:?} unavailable",
                ctx.task_entity, fixed_source_item
            );
            return false;
        };

        if resource_type.requires_wheelbarrow()
            && let Ok((_, stock_transform, _, _)) = queries.storage.stockpiles.get(stockpile)
        {
            let stock_pos = stock_transform.translation.truncate();
            if can_complete_pick_drop_to_point(source_pos, stock_pos) {
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
            if let Some(wb_entity) = wheelbarrow::find_nearest_wheelbarrow(task_pos, queries, shadow)
            {
                issue_haul_with_wheelbarrow(
                    wb_entity,
                    source_pos,
                    WheelbarrowDestination::Stockpile(stockpile),
                    vec![source_item],
                    task_pos,
                    already_commanded,
                    ctx,
                    queries,
                    shadow,
                );
                return true;
            }
            return false;
        }

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

    if resource_type.requires_wheelbarrow()
        && queries.wheelbarrow_leases.get(ctx.task_entity).is_err()
        && let Ok((_, stock_transform, _, _)) = queries.storage.stockpiles.get(stockpile)
    {
        let stock_pos = stock_transform.translation.truncate();
        if let Some((source_item, source_pos)) =
            source_selector::find_nearest_stockpile_source_item(
                resource_type,
                item_owner,
                stock_pos,
                queries,
                shadow,
            )
        {
            if can_complete_pick_drop_to_point(source_pos, stock_pos) {
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
        }
    }

    if let Ok(lease) = queries.wheelbarrow_leases.get(ctx.task_entity) {
        let min_valid_items = if resource_type.requires_wheelbarrow() {
            1
        } else {
            WHEELBARROW_MIN_BATCH_SIZE
        };
        if lease_validation::validate_lease(lease, queries, shadow, min_valid_items) {
            issue_haul_with_wheelbarrow(
                lease.wheelbarrow,
                lease.source_pos,
                lease.destination,
                lease.items.clone(),
                task_pos,
                already_commanded,
                ctx,
                queries,
                shadow,
            );
            return true;
        }
    }

    if resource_type.requires_wheelbarrow() {
        return false;
    }

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
            WheelbarrowDestination::Stockpile(stockpile),
            items,
            task_pos,
            already_commanded,
            ctx,
            queries,
            shadow,
        );
        return true;
    }

    let Some((source_item, source_pos)) = source_selector::find_nearest_stockpile_source_item(
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
    true
}
