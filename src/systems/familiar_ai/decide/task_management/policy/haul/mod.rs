//! 運搬タスクの割り当てポリシー

mod blueprint;
mod lease_validation;
mod source_selector;
mod stockpile;
mod wheelbarrow;

use crate::systems::familiar_ai::decide::task_management::{AssignTaskContext, ReservationShadow};
use crate::systems::logistics::transport_request::can_complete_pick_drop_to_point;
use bevy::prelude::*;

use super::super::builders::{
    issue_haul_to_mixer, issue_haul_to_stockpile_with_source, issue_haul_with_wheelbarrow,
};
use super::super::validator::{
    find_bucket_return_assignment, resolve_haul_to_mixer_inputs, resolve_return_bucket_tank,
};
fn mixer_can_accept_item(
    mixer_entity: Entity,
    item_type: crate::systems::logistics::ResourceType,
    mixer_already_reserved: bool,
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> bool {
    let Ok((_, storage, _)) = queries.storage.mixers.get(mixer_entity) else {
        return false;
    };
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
}

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
            "ASSIGN: HaulToMixer request {:?} has no resolver input",
            ctx.task_entity
        );
        return false;
    };

    if item_type.requires_wheelbarrow() {
        // その場ピック→ドロップで完了できるなら、猫車より徒歩運搬を優先する
        let can_try_pick_drop = queries.wheelbarrow_leases.get(ctx.task_entity).is_err();
        if can_try_pick_drop
            && let Ok((mixer_transform, _, _)) = queries.storage.mixers.get(mixer_entity)
        {
            let mixer_pos = mixer_transform.translation.truncate();
            let pick_drop_source =
                source_selector::find_nearest_mixer_source_item(item_type, mixer_pos, queries, shadow);

            if let Some((source_item, source_pos)) = pick_drop_source {
                if can_complete_pick_drop_to_point(source_pos, mixer_pos)
                    && mixer_can_accept_item(mixer_entity, item_type, false, queries, shadow)
                {
                    issue_haul_to_mixer(
                        source_item,
                        mixer_entity,
                        item_type,
                        false,
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

        // 猫車必須資源は徒歩運搬へフォールバックしない
        return false;
    }

    let Some((source_item, source_pos)) =
        source_selector::find_nearest_mixer_source_item(item_type, task_pos, queries, shadow)
    else {
        debug!(
            "ASSIGN: HaulToMixer request {:?} has no available {:?} source",
            ctx.task_entity, item_type
        );
        return false;
    };

    let can_accept = mixer_can_accept_item(mixer_entity, item_type, false, queries, shadow);

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
        false,
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
    if blueprint::assign_haul_to_blueprint(task_pos, already_commanded, ctx, queries, shadow) {
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

    if stockpile::assign_haul_to_stockpile(task_pos, already_commanded, ctx, queries, shadow) {
        return true;
    }
    debug!(
        "ASSIGN: Haul task {:?} is not a valid transport request candidate",
        ctx.task_entity
    );
    false
}
