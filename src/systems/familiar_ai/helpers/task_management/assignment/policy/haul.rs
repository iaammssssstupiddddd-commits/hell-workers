use crate::systems::familiar_ai::task_management::{AssignTaskContext, ReservationShadow};
use bevy::prelude::*;

use super::super::builders::{
    issue_haul_to_blueprint, issue_haul_to_mixer, issue_haul_to_stockpile,
};
use super::super::validator::{
    can_accept_mixer_item, find_best_stockpile_for_item, resolve_haul_to_mixer_inputs,
    source_not_reserved,
};

pub(super) fn assign_haul_to_mixer(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::task_execution::context::TaskAssignmentQueries,
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

    if !source_not_reserved(ctx.task_entity, queries, shadow) {
        debug!(
            "ASSIGN: HaulToMixer item {:?} is already reserved",
            ctx.task_entity
        );
        return false;
    }

    let can_accept = can_accept_mixer_item(mixer_entity, item_type, queries, shadow);

    if !can_accept {
        debug!(
            "ASSIGN: Mixer {:?} cannot accept item {:?} (Full or Reserved)",
            mixer_entity, item_type
        );
        return false;
    }

    issue_haul_to_mixer(
        mixer_entity,
        item_type,
        task_pos,
        already_commanded,
        ctx,
        queries,
        shadow,
    );
    true
}

pub(super) fn assign_haul(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    if let Ok(target_bp) = queries.storage.target_blueprints.get(ctx.task_entity) {
        if !source_not_reserved(ctx.task_entity, queries, shadow) {
            debug!(
                "ASSIGN: Item {:?} (for BP) is already reserved",
                ctx.task_entity
            );
            return false;
        }

        issue_haul_to_blueprint(
            target_bp.0,
            task_pos,
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

    if let Some(stock_entity) = best_stockpile {
        issue_haul_to_stockpile(
            stock_entity,
            task_pos,
            already_commanded,
            ctx,
            queries,
            shadow,
        );
        return true;
    }
    debug!(
        "ASSIGN: No suitable stockpile found for item {:?} (type: {:?})",
        ctx.task_entity, item_type
    );
    false
}
