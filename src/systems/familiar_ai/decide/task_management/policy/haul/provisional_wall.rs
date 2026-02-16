//! 仮設壁向け運搬タスク

use crate::systems::familiar_ai::decide::task_management::{AssignTaskContext, ReservationShadow};
use crate::systems::logistics::ResourceType;
use bevy::prelude::*;

use super::source_selector;
use super::wheelbarrow;
use super::super::super::builders::{issue_haul_to_stockpile_with_source, issue_haul_with_wheelbarrow};
use super::super::super::validator::resolve_haul_to_provisional_wall_inputs;

pub fn assign_haul_to_provisional_wall(
    _task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    let Some((wall_entity, resource_type)) =
        resolve_haul_to_provisional_wall_inputs(ctx.task_entity, queries)
    else {
        return false;
    };

    let Ok((wall_transform, building, provisional_opt)) =
        queries.storage.buildings.get(wall_entity)
    else {
        debug!(
            "ASSIGN: ProvisionalWall request {:?} wall {:?} not found",
            ctx.task_entity, wall_entity
        );
        return false;
    };

    if building.kind != crate::systems::jobs::BuildingType::Wall
        || !building.is_provisional
        || provisional_opt.is_none_or(|provisional| provisional.mud_delivered)
    {
        return false;
    }

    let wall_pos = wall_transform.translation.truncate();
    let Some((source_item, source_pos)) = source_selector::find_nearest_blueprint_source_item(
        resource_type,
        wall_pos,
        queries,
        shadow,
    ) else {
        debug!(
            "ASSIGN: ProvisionalWall request {:?} has no available {:?} source",
            ctx.task_entity, resource_type
        );
        return false;
    };

    if resource_type == ResourceType::StasisMud {
        let Some(wheelbarrow) = wheelbarrow::find_nearest_wheelbarrow(source_pos, queries, shadow)
        else {
            debug!(
                "ASSIGN: ProvisionalWall request {:?} has no available wheelbarrow for {:?}",
                ctx.task_entity, resource_type
            );
            return false;
        };
        issue_haul_with_wheelbarrow(
            wheelbarrow,
            source_pos,
            crate::systems::logistics::transport_request::WheelbarrowDestination::Stockpile(
                wall_entity,
            ),
            vec![source_item],
            wall_pos,
            already_commanded,
            ctx,
            queries,
            shadow,
        );
        return true;
    }

    issue_haul_to_stockpile_with_source(
        source_item,
        wall_entity,
        source_pos,
        already_commanded,
        ctx,
        queries,
        shadow,
    );
    true
}
