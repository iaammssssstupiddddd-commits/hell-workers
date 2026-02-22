//! Blueprint 向け運搬タスクの割り当て

use crate::constants::WHEELBARROW_CAPACITY;
use crate::systems::command::TaskArea;
use crate::systems::familiar_ai::decide::task_management::{AssignTaskContext, ReservationShadow};
use crate::systems::logistics::ResourceType;
use crate::systems::logistics::transport_request::can_complete_pick_drop_to_blueprint;
use bevy::prelude::*;

use super::super::super::builders::{
    issue_collect_bone_with_wheelbarrow_to_blueprint,
    issue_collect_sand_with_wheelbarrow_to_blueprint, issue_haul_to_blueprint_with_source,
};
use super::super::super::validator::resolve_haul_to_blueprint_inputs;
use super::demand;
use super::direct_collect;
use super::lease_validation;
use super::source_selector;
use super::wheelbarrow;

pub fn assign_haul_to_blueprint(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    let Some((blueprint, resource_type)) =
        resolve_haul_to_blueprint_inputs(ctx.task_entity, queries)
    else {
        return false;
    };

    if !resource_type.requires_wheelbarrow() {
        return assign_single_item_haul(
            blueprint,
            resource_type,
            task_pos,
            already_commanded,
            ctx,
            queries,
            shadow,
        );
    }

    let remaining_needed = demand::compute_remaining_blueprint_wheelbarrow_amount(
        blueprint,
        resource_type,
        ctx.task_entity,
        queries,
    );
    if remaining_needed == 0 {
        return false;
    }

    if queries.wheelbarrow_leases.get(ctx.task_entity).is_err()
        && resource_type != ResourceType::StasisMud
    {
        if try_pick_drop_to_blueprint(
            blueprint,
            resource_type,
            already_commanded,
            ctx,
            queries,
            shadow,
        ) {
            return true;
        }
    }

    let max_items = remaining_needed.min(WHEELBARROW_CAPACITY as u32) as usize;
    if lease_validation::try_issue_haul_from_lease(
        ctx.task_entity,
        task_pos,
        already_commanded,
        1,
        max_items,
        |_| true,
        ctx,
        queries,
        shadow,
    ) {
        return true;
    }

    if resource_type == ResourceType::Sand
        && try_direct_collect_with_wheelbarrow_to_blueprint(
            blueprint,
            remaining_needed,
            task_pos,
            already_commanded,
            ctx,
            queries,
            shadow,
            direct_collect::find_collect_sand_source,
            issue_collect_sand_with_wheelbarrow_to_blueprint,
        )
    {
        return true;
    }

    if resource_type == ResourceType::Bone
        && try_direct_collect_with_wheelbarrow_to_blueprint(
            blueprint,
            remaining_needed,
            task_pos,
            already_commanded,
            ctx,
            queries,
            shadow,
            direct_collect::find_collect_bone_source,
            issue_collect_bone_with_wheelbarrow_to_blueprint,
        )
    {
        return true;
    }

    false
}

fn try_pick_drop_to_blueprint(
    blueprint: Entity,
    resource_type: ResourceType,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    let Ok((bp_transform, bp, _)) = queries.storage.blueprints.get(blueprint) else {
        return false;
    };
    let bp_pos = bp_transform.translation.truncate();
    let occupied_grids = bp.occupied_grids.clone();

    let Some((source_item, source_pos)) =
        source_selector::find_nearest_blueprint_source_item(resource_type, bp_pos, queries, shadow)
    else {
        return false;
    };

    if !can_complete_pick_drop_to_blueprint(source_pos, &occupied_grids) {
        return false;
    }

    issue_haul_to_blueprint_with_source(
        source_item,
        blueprint,
        source_pos,
        already_commanded,
        ctx,
        queries,
        shadow,
    );
    true
}

fn assign_single_item_haul(
    blueprint: Entity,
    resource_type: ResourceType,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    let Some((source_item, source_pos)) = source_selector::find_nearest_blueprint_source_item(
        resource_type,
        task_pos,
        queries,
        shadow,
    ) else {
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
    true
}

type FindSourceFn = fn(
    Vec2,
    Option<&TaskArea>,
    &crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries,
    &ReservationShadow,
) -> Option<(Entity, Vec2)>;

/// Sand/Bone 直接採取を猫車で Blueprint へ搬入（共通化）
fn try_direct_collect_with_wheelbarrow_to_blueprint(
    blueprint: Entity,
    remaining_needed: u32,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
    find_source: FindSourceFn,
    issue_fn: fn(
        Entity,
        Entity,
        Vec2,
        Entity,
        u32,
        Vec2,
        bool,
        &AssignTaskContext<'_>,
        &mut crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries,
        &mut ReservationShadow,
    ),
) -> bool {
    let Some((source_entity, source_pos)) =
        find_source(task_pos, ctx.task_area_opt, queries, shadow)
    else {
        return false;
    };

    let Some(wb_entity) = wheelbarrow::find_nearest_wheelbarrow(source_pos, queries, shadow) else {
        return false;
    };

    let amount = remaining_needed.max(1).min(WHEELBARROW_CAPACITY as u32);

    issue_fn(
        wb_entity,
        source_entity,
        source_pos,
        blueprint,
        amount,
        task_pos,
        already_commanded,
        ctx,
        queries,
        shadow,
    );
    true
}
