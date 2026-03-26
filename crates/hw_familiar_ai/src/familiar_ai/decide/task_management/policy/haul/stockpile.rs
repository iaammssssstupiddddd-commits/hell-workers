use bevy::prelude::*;
use hw_core::constants::*;
use hw_core::logistics::WheelbarrowDestination;
use hw_logistics::transport_request::can_complete_pick_drop_to_point;

use super::super::super::builders::{
    WheelbarrowHaulSpec, issue_haul_to_stockpile_with_source, issue_haul_with_wheelbarrow,
};
use super::super::super::validator::resolve_haul_to_stockpile_inputs;
use super::demand;
use super::lease_validation;
use super::source_selector;
use super::wheelbarrow;
use crate::familiar_ai::decide::task_management::{
    AssignTaskContext, FamiliarTaskAssignmentQueries, ReservationShadow,
};

pub fn assign_haul_to_stockpile(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    let Some((stockpile, resource_type, item_owner, fixed_source)) =
        resolve_haul_to_stockpile_inputs(ctx.task_entity, queries, shadow)
    else {
        return false;
    };
    let demand_context =
        demand::DemandReadContext::new(queries, shadow, ctx.tile_site_index, ctx.incoming_snapshot);
    let remaining_capacity =
        demand::compute_remaining_stockpile_capacity(stockpile, resource_type, &demand_context);
    if remaining_capacity == 0 {
        return false;
    }

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
            if lease_validation::try_issue_haul_from_lease(
                lease_validation::HaulFromLeaseSpec {
                    task_entity: ctx.task_entity,
                    task_pos,
                    already_commanded,
                    min_valid_items: 1,
                    max_items: 1,
                },
                |item| item == source_item,
                ctx,
                queries,
                shadow,
            ) {
                return true;
            }
            if let Some(wb_entity) =
                wheelbarrow::find_nearest_wheelbarrow(task_pos, queries, shadow)
            {
                issue_haul_with_wheelbarrow(
                    WheelbarrowHaulSpec {
                        wheelbarrow: wb_entity,
                        source_pos,
                        destination: WheelbarrowDestination::Stockpile(stockpile),
                        items: vec![source_item],
                    },
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
        if let Some((source_item, source_pos)) = source_selector::find_nearest_stockpile_source_item(
            resource_type,
            item_owner,
            stock_pos,
            queries,
            shadow,
            ctx.resource_grid,
        ) && can_complete_pick_drop_to_point(source_pos, stock_pos)
        {
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

    let max_items = remaining_capacity.max(1) as usize;
    let min_valid_items = if resource_type.requires_wheelbarrow() {
        1
    } else {
        WHEELBARROW_MIN_BATCH_SIZE
    };
    if lease_validation::try_issue_haul_from_lease(
        lease_validation::HaulFromLeaseSpec {
            task_entity: ctx.task_entity,
            task_pos,
            already_commanded,
            min_valid_items,
            max_items,
        },
        |_| true,
        ctx,
        queries,
        shadow,
    ) {
        return true;
    }

    if resource_type.requires_wheelbarrow() {
        return false;
    }

    let Some((source_item, source_pos)) = source_selector::find_nearest_stockpile_source_item(
        resource_type,
        item_owner,
        task_pos,
        queries,
        shadow,
        ctx.resource_grid,
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
