use bevy::prelude::*;
use hw_core::logistics::ResourceType;

use super::super::super::builders::{
    issue_haul_to_stockpile_with_source, issue_haul_with_wheelbarrow,
};
use super::super::super::validator::resolve_haul_to_wall_construction_inputs;
use super::construction_mud::{ConstructionMudInput, select_construction_mud_haul};
use super::demand;
use super::source_selector;
use crate::familiar_ai::decide::task_management::context::ConstructionSitePositions;
use crate::familiar_ai::decide::task_management::{
    AssignTaskContext, FamiliarTaskAssignmentQueries, ReservationShadow,
};

pub fn assign_haul_to_wall_construction(
    _task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    construction_sites: &impl ConstructionSitePositions,
    shadow: &mut ReservationShadow,
) -> bool {
    let Some((site_entity, resource_type)) =
        resolve_haul_to_wall_construction_inputs(ctx.task_entity, queries)
    else {
        return false;
    };

    let site_pos = match construction_sites.wall_site_pos(site_entity) {
        Some(pos) => pos,
        None => {
            debug!(
                "ASSIGN: Wall request {:?} site {:?} not found",
                ctx.task_entity, site_entity
            );
            return false;
        }
    };
    let demand_context =
        demand::DemandReadContext::new(queries, shadow, ctx.tile_site_index, ctx.incoming_snapshot);

    let remaining_needed = match resource_type {
        ResourceType::Wood => demand::compute_remaining_wall_wood(site_entity, &demand_context),
        ResourceType::StasisMud => demand::compute_remaining_wall_mud(site_entity, &demand_context),
        _ => 0,
    };
    if remaining_needed == 0 {
        return false;
    }

    if resource_type == ResourceType::StasisMud {
        let Some(spec) = select_construction_mud_haul(
            ConstructionMudInput {
                site: site_entity,
                site_pos,
                remaining_needed,
            },
            queries,
            shadow,
            ctx.resource_grid,
        ) else {
            debug!(
                "ASSIGN: Wall request {:?} has no available source or wheelbarrow for {:?}",
                ctx.task_entity, resource_type
            );
            return false;
        };
        issue_haul_with_wheelbarrow(spec, site_pos, already_commanded, ctx, queries, shadow);
        return true;
    }

    if let Some((source_item, source_pos)) = source_selector::find_nearest_blueprint_source_item(
        resource_type,
        site_pos,
        queries,
        shadow,
        ctx.resource_grid,
    ) {
        issue_haul_to_stockpile_with_source(
            source_item,
            site_entity,
            source_pos,
            already_commanded,
            ctx,
            queries,
            shadow,
        );
        return true;
    }

    debug!(
        "ASSIGN: Wall request {:?} has no available {:?} source",
        ctx.task_entity, resource_type
    );
    false
}
