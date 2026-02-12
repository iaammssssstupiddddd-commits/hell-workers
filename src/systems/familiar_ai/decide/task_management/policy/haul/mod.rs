//! 運搬タスクの割り当てポリシー

mod lease_validation;
mod source_selector;
mod wheelbarrow;

use crate::constants::*;
use crate::systems::familiar_ai::decide::task_management::{AssignTaskContext, ReservationShadow};
use crate::systems::logistics::transport_request::TransportRequestKind;
use bevy::prelude::*;

use super::super::builders::{
    issue_haul_to_blueprint, issue_haul_to_blueprint_with_source, issue_haul_to_mixer,
    issue_haul_to_stockpile, issue_haul_to_stockpile_with_source, issue_haul_with_wheelbarrow,
};
use super::super::validator::{
    compute_centroid, find_best_stockpile_for_item, find_bucket_return_assignment,
    resolve_haul_to_blueprint_inputs, resolve_haul_to_mixer_inputs,
    resolve_haul_to_stockpile_inputs, resolve_return_bucket_tank,
    resolve_wheelbarrow_batch_for_stockpile, source_not_reserved,
};

pub fn assign_haul_to_mixer(
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
            source_selector::find_nearest_mixer_source_item(item_type, task_pos, queries, shadow)
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

pub fn assign_haul(
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
                source_selector::find_nearest_blueprint_source_item(
                    resource_type,
                    task_pos,
                    queries,
                    shadow,
                )
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

    if let Some(tank) = resolve_return_bucket_tank(ctx.task_entity, queries) {
        let Some((source_item, source_pos, destination_stockpile)) =
            find_bucket_return_assignment(tank, task_pos, queries, shadow)
        else {
            debug!(
                "ASSIGN: ReturnBucket request {:?} has no valid source/destination for tank {:?}",
                ctx.task_entity, tank
            );
            return false;
        };
        issue_haul_to_stockpile_with_source(
            source_item,
            destination_stockpile,
            source_pos,
            already_commanded,
            ctx,
            queries,
            shadow,
        );
        return true;
    }

    if let Some((stockpile, resource_type, item_owner)) =
        resolve_haul_to_stockpile_inputs(ctx.task_entity, queries)
    {
        if let Ok(lease) = queries.wheelbarrow_leases.get(ctx.task_entity) {
            if lease_validation::validate_lease(lease, queries, shadow) {
                let source_pos = lease.source_pos;
                let items = lease.items.clone();
                let wb = lease.wheelbarrow;
                let dest = lease.dest_stockpile;
                issue_haul_with_wheelbarrow(
                    wb,
                    source_pos,
                    dest,
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

    if item_type.is_loadable() {
        if let Some(wb_entity) = wheelbarrow::find_nearest_wheelbarrow(task_pos, queries, shadow) {
            let batch_items =
                wheelbarrow::collect_nearby_haulable_items(ctx.task_entity, task_pos, queries, shadow);

            if batch_items.len() >= WHEELBARROW_MIN_BATCH_SIZE {
                let dest_capacity =
                    wheelbarrow::remaining_stockpile_capacity(stock_entity, queries, shadow);
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
