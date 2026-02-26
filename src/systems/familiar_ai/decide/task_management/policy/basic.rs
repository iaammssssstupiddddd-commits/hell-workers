use crate::systems::familiar_ai::decide::task_management::{AssignTaskContext, ReservationShadow};
use crate::systems::jobs::WorkType;
use crate::systems::logistics::ResourceType;
use crate::systems::logistics::transport_request::TransportRequestKind;
use bevy::prelude::*;

use super::super::builders::{
    issue_build, issue_collect_bone, issue_collect_sand, issue_gather, issue_refine,
};
use super::super::validator::can_reserve_source;

pub(super) fn assign_gather(
    work_type: WorkType,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries,
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
    queries: &mut crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    if let Ok((_, bp, _)) = queries.storage.blueprints.get(ctx.task_entity) {
        if !bp.materials_complete() {
            debug!(
                "ASSIGN: Build target {:?} materials not complete",
                ctx.task_entity
            );
            return false;
        }
    }

    if !can_reserve_source(ctx.task_entity, queries, shadow) {
        return false;
    }
    issue_build(task_pos, already_commanded, ctx, queries, shadow);
    true
}

pub(super) fn assign_collect_sand(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries,
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

fn has_collect_sand_demand(
    fam_entity: Entity,
    queries: &crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries,
) -> bool {
    queries
        .designation
        .designations
        .iter()
        .any(|(entity, _, designation, managed_by_opt, _, workers_opt, _, _)| {
            if managed_by_opt.map(|managed_by| managed_by.0) != Some(fam_entity) {
                return false;
            }

            // 明示的な Refine 指定は精製需要として扱う。
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
            let workers = workers_opt.map(|task_workers| task_workers.len() as u32).unwrap_or(0);
            if desired_slots == 0 && workers == 0 {
                return false;
            }

            matches!(
                (request.kind, request.resource_type),
                (TransportRequestKind::DeliverToBlueprint, ResourceType::Sand)
                    | (TransportRequestKind::DeliverToBlueprint, ResourceType::StasisMud)
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
        })
}

pub(super) fn assign_refine(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries,
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
    queries: &mut crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    if !can_reserve_source(ctx.task_entity, queries, shadow) {
        return false;
    }
    issue_collect_bone(task_pos, already_commanded, ctx, queries, shadow);
    true
}
