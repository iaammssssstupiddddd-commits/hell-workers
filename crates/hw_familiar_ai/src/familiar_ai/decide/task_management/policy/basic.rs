use bevy::prelude::*;
use hw_core::logistics::ResourceType;
use hw_jobs::WorkType;
use hw_logistics::transport_request::TransportRequestKind;

use super::super::builders::{
    issue_build, issue_collect_bone, issue_collect_sand, issue_gather, issue_move, issue_refine,
};
use super::super::validator::can_reserve_source;
use crate::familiar_ai::decide::task_management::{
    AssignTaskContext, FamiliarTaskAssignmentQueries, ReservationShadow,
};

pub(super) fn assign_gather(
    work_type: WorkType,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    if !can_reserve_source(ctx.task_entity, queries, shadow) {
        return false;
    }
    issue_gather(work_type, task_pos, already_commanded, ctx, queries, shadow);
    true
}

pub(super) fn assign_build(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    if let Ok((_, bp, _)) = queries.storage.blueprints.get(ctx.task_entity)
        && !bp.materials_complete()
    {
        debug!(
            "ASSIGN: Build target {:?} materials not complete",
            ctx.task_entity
        );
        return false;
    }
    issue_build(task_pos, already_commanded, ctx, queries, shadow);
    true
}

pub(super) fn assign_move(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    issue_move(task_pos, already_commanded, ctx, queries, shadow);
    true
}

pub(super) fn assign_collect_sand(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    if !has_collect_sand_demand(ctx.fam_entity, queries) {
        debug!(
            "ASSIGN: Skip CollectSand target {:?} (no refine/build demand for familiar {:?})",
            ctx.task_entity, ctx.fam_entity
        );
        return false;
    }

    if !can_reserve_source(ctx.task_entity, queries, shadow) {
        return false;
    }
    issue_collect_sand(task_pos, already_commanded, ctx, queries, shadow);
    true
}

fn has_collect_sand_demand(fam_entity: Entity, queries: &FamiliarTaskAssignmentQueries) -> bool {
    queries.designation.designations.iter().any(
        |(entity, transform, designation, managed_by_opt, _, workers_opt, _, _)| {
            let task_pos = transform.translation.truncate();
            let in_any_yard = queries.yards.iter().any(|yard| yard.contains(task_pos));
            let managed_by_me = managed_by_opt.map(|managed_by| managed_by.0) == Some(fam_entity);
            if !managed_by_me && !in_any_yard {
                return false;
            }

            if designation.work_type == WorkType::Refine {
                return true;
            }

            let Ok(request) = queries.transport_requests.get(entity) else {
                return false;
            };

            let desired_slots = queries
                .transport_demands
                .get(entity)
                .map(|demand| demand.desired_slots)
                .unwrap_or(0);
            let workers = workers_opt
                .map(|task_workers| task_workers.len() as u32)
                .unwrap_or(0);
            if desired_slots == 0 && workers == 0 {
                return false;
            }

            matches!(
                (request.kind, request.resource_type),
                (
                    TransportRequestKind::DeliverToMixerSolid,
                    ResourceType::Sand
                ) | (TransportRequestKind::DeliverToBlueprint, ResourceType::Sand)
                    | (
                        TransportRequestKind::DeliverToBlueprint,
                        ResourceType::StasisMud
                    )
                    | (
                        TransportRequestKind::DeliverToFloorConstruction,
                        ResourceType::StasisMud
                    )
                    | (
                        TransportRequestKind::DeliverToWallConstruction,
                        ResourceType::StasisMud
                    )
                    | (
                        TransportRequestKind::DeliverToProvisionalWall,
                        ResourceType::StasisMud
                    )
            )
        },
    )
}

pub(super) fn assign_refine(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    if !can_reserve_source(ctx.task_entity, queries, shadow) {
        return false;
    }
    issue_refine(task_pos, already_commanded, ctx, queries, shadow);
    true
}

pub(super) fn assign_collect_bone(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    if !can_reserve_source(ctx.task_entity, queries, shadow) {
        return false;
    }
    issue_collect_bone(task_pos, already_commanded, ctx, queries, shadow);
    true
}
