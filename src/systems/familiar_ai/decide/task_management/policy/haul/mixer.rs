//! Mixer 向け運搬タスク

use crate::systems::familiar_ai::decide::task_management::{AssignTaskContext, ReservationShadow};
use crate::systems::logistics::ResourceType;
use bevy::prelude::*;

use super::super::super::builders::issue_haul_to_mixer;
use super::super::super::validator::resolve_haul_to_mixer_inputs;
use super::lease_validation;
use super::source_selector;

fn mixer_can_accept_item(
    mixer_entity: Entity,
    item_type: ResourceType,
    mixer_already_reserved: bool,
    queries: &crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries,
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
    queries: &mut crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries,
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

    if lease_validation::try_issue_haul_from_lease(
        ctx.task_entity,
        task_pos,
        already_commanded,
        1,
        usize::MAX,
        |_| true,
        ctx,
        queries,
        shadow,
    ) {
        return true;
    }

    assign_single_item_haul_to_mixer(
        mixer_entity,
        item_type,
        task_pos,
        already_commanded,
        ctx,
        queries,
        shadow,
    )
}

fn assign_single_item_haul_to_mixer(
    mixer_entity: Entity,
    item_type: ResourceType,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    let Some((source_item, source_pos)) =
        source_selector::find_nearest_mixer_source_item(item_type, task_pos, queries, shadow)
    else {
        debug!(
            "ASSIGN: HaulToMixer request {:?} has no available {:?} source",
            ctx.task_entity, item_type
        );
        return false;
    };

    if !mixer_can_accept_item(mixer_entity, item_type, false, queries, shadow) {
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
