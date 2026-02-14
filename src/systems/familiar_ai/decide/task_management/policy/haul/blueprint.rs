//! Blueprint 向け運搬タスクの割り当て

use crate::systems::familiar_ai::decide::task_management::{AssignTaskContext, ReservationShadow};
use crate::systems::logistics::transport_request::{
    can_complete_pick_drop_to_blueprint, TransportRequestKind, WheelbarrowDestination,
};
use bevy::prelude::*;

use super::super::super::builders::{
    issue_haul_to_blueprint, issue_haul_to_blueprint_with_source, issue_haul_with_wheelbarrow,
};
use super::super::super::validator::{
    resolve_haul_to_blueprint_inputs, source_not_reserved,
};
use super::lease_validation;
use super::source_selector;
use super::wheelbarrow;

pub fn assign_haul_to_blueprint(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    let Some((blueprint, resource_type)) = resolve_haul_to_blueprint_inputs(ctx.task_entity, queries)
    else {
        return false;
    };

    let is_request_task = queries
        .transport_requests
        .get(ctx.task_entity)
        .is_ok_and(|req| matches!(req.kind, TransportRequestKind::DeliverToBlueprint));

    if resource_type.requires_wheelbarrow() {
        let can_try_pick_drop =
            !is_request_task || queries.wheelbarrow_leases.get(ctx.task_entity).is_err();
        if can_try_pick_drop
            && let Ok((bp_transform, bp, _)) = queries.storage.blueprints.get(blueprint)
        {
            let bp_pos = bp_transform.translation.truncate();
            let pick_drop_source = if is_request_task {
                source_selector::find_nearest_blueprint_source_item(
                    resource_type,
                    bp_pos,
                    queries,
                    shadow,
                )
            } else if source_not_reserved(ctx.task_entity, queries, shadow) {
                Some((ctx.task_entity, task_pos))
            } else {
                None
            };

            if let Some((source_item, source_pos)) = pick_drop_source {
                if can_complete_pick_drop_to_blueprint(source_pos, &bp.occupied_grids) {
                    if is_request_task {
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
                        issue_haul_to_blueprint(
                            blueprint,
                            task_pos,
                            already_commanded,
                            ctx,
                            queries,
                            shadow,
                        );
                    }
                    return true;
                }
            }
        }

        if let Ok(lease) = queries.wheelbarrow_leases.get(ctx.task_entity) {
            if lease_validation::validate_lease(lease, queries, shadow, 1) {
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

        // lease が無い場合のフォールバック:
        // request タスクでも、最寄りの猫車 + 最寄りのソース1件で直接割り当てる。
        if let Some(wb_entity) = wheelbarrow::find_nearest_wheelbarrow(task_pos, queries, shadow) {
            let source = if is_request_task {
                source_selector::find_nearest_blueprint_source_item(
                    resource_type,
                    task_pos,
                    queries,
                    shadow,
                )
            } else if source_not_reserved(ctx.task_entity, queries, shadow) {
                Some((ctx.task_entity, task_pos))
            } else {
                None
            };

            if let Some((source_item, source_pos)) = source {
                issue_haul_with_wheelbarrow(
                    wb_entity,
                    source_pos,
                    WheelbarrowDestination::Blueprint(blueprint),
                    vec![source_item],
                    task_pos,
                    already_commanded,
                    ctx,
                    queries,
                    shadow,
                );
                return true;
            }
        }

        return false;
    }

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
    true
}
