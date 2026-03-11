use bevy::prelude::*;
use hw_core::logistics::{ResourceType, WheelbarrowDestination};
use hw_jobs::BuildingType;

use crate::familiar_ai::decide::task_management::{AssignTaskContext, FamiliarTaskAssignmentQueries, ReservationShadow};
use super::super::super::builders::{
    issue_haul_to_stockpile_with_source, issue_haul_with_wheelbarrow,
};
use super::super::super::validator::resolve_haul_to_provisional_wall_inputs;
use super::demand;
use super::source_selector;
use super::wheelbarrow;

pub fn assign_haul_to_provisional_wall(
    _task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
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

    if building.kind != BuildingType::Wall
        || !building.is_provisional
        || provisional_opt.is_none_or(|provisional| provisional.mud_delivered)
    {
        return false;
    }
    let demand_context =
        demand::DemandReadContext::new(queries, shadow, ctx.tile_site_index, ctx.incoming_snapshot);
    if demand::compute_remaining_provisional_wall_mud(wall_entity, &demand_context) == 0 {
        return false;
    }

    let wall_pos = wall_transform.translation.truncate();
    let Some((source_item, source_pos)) = source_selector::find_nearest_blueprint_source_item(
        resource_type,
        wall_pos,
        queries,
        shadow,
        ctx.resource_grid,
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
            WheelbarrowDestination::Stockpile(wall_entity),
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
