use bevy::prelude::*;
use hw_jobs::WorkType;

use super::super::builders::{
    issue_build, issue_collect_bone, issue_gather, issue_generate_power, issue_move, issue_refine,
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

pub(super) fn assign_generate_power(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    // タイルの parent_site を引き、active_slots ゲートを確認
    let Ok((tile, _)) = queries.soul_spa_tiles.get(ctx.task_entity) else {
        return false;
    };
    let parent_site = tile.parent_site;
    let Ok(site) = queries.soul_spa_sites.get(parent_site) else {
        return false;
    };
    let occupied = queries
        .soul_spa_tiles
        .iter()
        .filter(|(t, w)| t.parent_site == parent_site && w.map(|w| !w.is_empty()).unwrap_or(false))
        .count() as u32;
    if !site.has_available_slot(occupied) {
        return false;
    }
    issue_generate_power(task_pos, already_commanded, ctx, queries, shadow);
    true
}
