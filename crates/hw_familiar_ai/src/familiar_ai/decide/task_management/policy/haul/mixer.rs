use bevy::prelude::*;
use hw_core::constants::{MUD_MIXER_CAPACITY, WHEELBARROW_CAPACITY};
use hw_core::logistics::ResourceType;
use hw_logistics::transport_request::can_complete_pick_drop_to_point;

use super::super::super::builders::{
    issue_collect_sand_with_wheelbarrow_to_mixer, issue_haul_to_mixer,
};
use super::super::super::validator::resolve_haul_to_mixer_inputs;
use super::{direct_collect, lease_validation, source_selector, wheelbarrow};
use crate::familiar_ai::decide::task_management::{
    AssignTaskContext, FamiliarTaskAssignmentQueries, ReservationShadow,
};

fn mixer_can_accept_item(
    mixer_entity: Entity,
    item_type: ResourceType,
    mixer_already_reserved: bool,
    queries: &FamiliarTaskAssignmentQueries,
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
    queries: &mut FamiliarTaskAssignmentQueries,
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

    // Sand: use collect_source path (fill wheelbarrow at source, like buckets for water)
    if item_type == ResourceType::Sand
        && try_direct_collect_with_wheelbarrow_to_mixer(
            mixer_entity,
            task_pos,
            already_commanded,
            ctx,
            queries,
            shadow,
        )
    {
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
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    let Some((source_item, source_pos)) = source_selector::find_nearest_mixer_source_item(
        item_type,
        task_pos,
        queries,
        shadow,
        ctx.resource_grid,
    ) else {
        debug!(
            "ASSIGN: HaulToMixer request {:?} has no available {:?} source",
            ctx.task_entity, item_type
        );
        return false;
    };

    // 猫車必須リソース（Sand など）は pick-drop 完結距離外なら待機してリースを待つ
    if item_type.requires_wheelbarrow() && !can_complete_pick_drop_to_point(source_pos, task_pos) {
        debug!(
            "ASSIGN: HaulToMixer request {:?} {:?} requires wheelbarrow but no lease available; waiting",
            ctx.task_entity, item_type
        );
        return false;
    }

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

fn try_direct_collect_with_wheelbarrow_to_mixer(
    mixer_entity: Entity,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    let Ok((_, storage, _)) = queries.storage.mixers.get(mixer_entity) else {
        return false;
    };
    let reserved = queries
        .reservation
        .resource_cache
        .get_mixer_destination_reservation(mixer_entity, ResourceType::Sand)
        + shadow.mixer_reserved(mixer_entity, ResourceType::Sand);
    let available =
        MUD_MIXER_CAPACITY.saturating_sub(storage.sand + reserved as u32);
    if available == 0 {
        return false;
    }
    let amount = available.min(WHEELBARROW_CAPACITY as u32);

    let Some((source_entity, source_pos)) =
        direct_collect::find_collect_sand_source(task_pos, ctx.task_area_opt, queries, shadow)
    else {
        return false;
    };

    let Some(wb_entity) = wheelbarrow::find_nearest_wheelbarrow(source_pos, queries, shadow) else {
        return false;
    };

    issue_collect_sand_with_wheelbarrow_to_mixer(
        wb_entity,
        source_entity,
        source_pos,
        mixer_entity,
        amount,
        task_pos,
        already_commanded,
        ctx,
        queries,
        shadow,
    );
    true
}
