use crate::events::ResourceReservationOp;
use crate::systems::familiar_ai::decide::task_management::{AssignTaskContext, ReservationShadow};
use crate::systems::jobs::WorkType;
use crate::systems::soul_ai::execute::task_execution::types::{
    BuildPhase, CoatWallPhase, FrameWallPhase, GatherPhase, PourFloorPhase, ReinforceFloorPhase,
};
use bevy::prelude::*;

use super::submit_assignment;

pub fn issue_gather(
    work_type: WorkType,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let assigned_task =
        crate::systems::soul_ai::execute::task_execution::types::AssignedTask::Gather(
            crate::systems::soul_ai::execute::task_execution::types::GatherData {
                target: ctx.task_entity,
                work_type,
                phase: GatherPhase::GoingToResource,
            },
        );
    let reservation_ops = vec![ResourceReservationOp::ReserveSource {
        source: ctx.task_entity,
        amount: 1,
    }];
    submit_assignment(
        ctx,
        queries,
        shadow,
        work_type,
        task_pos,
        assigned_task,
        reservation_ops,
        already_commanded,
    );
}

pub fn issue_build(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let assigned_task =
        crate::systems::soul_ai::execute::task_execution::types::AssignedTask::Build(
            crate::systems::soul_ai::execute::task_execution::types::BuildData {
                blueprint: ctx.task_entity,
                phase: BuildPhase::GoingToBlueprint,
            },
        );
    let reservation_ops = vec![ResourceReservationOp::ReserveSource {
        source: ctx.task_entity,
        amount: 1,
    }];
    submit_assignment(
        ctx,
        queries,
        shadow,
        WorkType::Build,
        task_pos,
        assigned_task,
        reservation_ops,
        already_commanded,
    );
}

pub fn issue_collect_sand(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let assigned_task = crate::systems::soul_ai::execute::task_execution::types::AssignedTask::CollectSand(
        crate::systems::soul_ai::execute::task_execution::types::CollectSandData {
            target: ctx.task_entity,
            phase: crate::systems::soul_ai::execute::task_execution::types::CollectSandPhase::GoingToSand,
        },
    );
    let reservation_ops = vec![ResourceReservationOp::ReserveSource {
        source: ctx.task_entity,
        amount: 1,
    }];
    submit_assignment(
        ctx,
        queries,
        shadow,
        WorkType::CollectSand,
        task_pos,
        assigned_task,
        reservation_ops,
        already_commanded,
    );
}

pub fn issue_refine(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let assigned_task = crate::systems::soul_ai::execute::task_execution::types::AssignedTask::Refine(
        crate::systems::soul_ai::execute::task_execution::types::RefineData {
            mixer: ctx.task_entity,
            phase: crate::systems::soul_ai::execute::task_execution::types::RefinePhase::GoingToMixer,
        },
    );
    let reservation_ops = vec![ResourceReservationOp::ReserveSource {
        source: ctx.task_entity,
        amount: 1,
    }];
    submit_assignment(
        ctx,
        queries,
        shadow,
        WorkType::Refine,
        task_pos,
        assigned_task,
        reservation_ops,
        already_commanded,
    );
}

pub fn issue_collect_bone(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let assigned_task = crate::systems::soul_ai::execute::task_execution::types::AssignedTask::CollectBone(
        crate::systems::soul_ai::execute::task_execution::types::CollectBoneData {
            target: ctx.task_entity,
            phase: crate::systems::soul_ai::execute::task_execution::types::CollectBonePhase::GoingToBone,
        },
    );
    let reservation_ops = vec![ResourceReservationOp::ReserveSource {
        source: ctx.task_entity,
        amount: 1,
    }];
    submit_assignment(
        ctx,
        queries,
        shadow,
        WorkType::CollectBone,
        task_pos,
        assigned_task,
        reservation_ops,
        already_commanded,
    );
}

pub fn issue_reinforce_floor(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    // Get site entity from tile
    let site_entity = if let Ok(tile) = queries.storage.floor_tiles.get(ctx.task_entity) {
        tile.parent_site
    } else {
        error!(
            "issue_reinforce_floor: Task entity {:?} is not a FloorTileBlueprint",
            ctx.task_entity
        );
        return;
    };

    let assigned_task =
        crate::systems::soul_ai::execute::task_execution::types::AssignedTask::ReinforceFloorTile(
            crate::systems::soul_ai::execute::task_execution::types::ReinforceFloorTileData {
                tile: ctx.task_entity,
                site: site_entity,
                phase: ReinforceFloorPhase::GoingToMaterialCenter,
            },
        );
    let reservation_ops = vec![ResourceReservationOp::ReserveSource {
        source: ctx.task_entity,
        amount: 1,
    }];
    submit_assignment(
        ctx,
        queries,
        shadow,
        WorkType::ReinforceFloorTile,
        task_pos,
        assigned_task,
        reservation_ops,
        already_commanded,
    );
}

pub fn issue_pour_floor(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    // Get site entity from tile
    let site_entity = if let Ok(tile) = queries.storage.floor_tiles.get(ctx.task_entity) {
        tile.parent_site
    } else {
        error!(
            "issue_pour_floor: Task entity {:?} is not a FloorTileBlueprint",
            ctx.task_entity
        );
        return;
    };

    let assigned_task =
        crate::systems::soul_ai::execute::task_execution::types::AssignedTask::PourFloorTile(
            crate::systems::soul_ai::execute::task_execution::types::PourFloorTileData {
                tile: ctx.task_entity,
                site: site_entity,
                phase: PourFloorPhase::GoingToMaterialCenter,
            },
        );
    let reservation_ops = vec![ResourceReservationOp::ReserveSource {
        source: ctx.task_entity,
        amount: 1,
    }];
    submit_assignment(
        ctx,
        queries,
        shadow,
        WorkType::PourFloorTile,
        task_pos,
        assigned_task,
        reservation_ops,
        already_commanded,
    );
}

pub fn issue_coat_wall(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let (tile_entity, site_entity, wall_entity) =
        if let Ok(tile) = queries.storage.wall_tiles.get(ctx.task_entity) {
            let Some(wall_entity) = tile.spawned_wall else {
                error!(
                    "issue_coat_wall: Tile {:?} has no spawned wall",
                    ctx.task_entity
                );
                return;
            };
            (ctx.task_entity, tile.parent_site, wall_entity)
        } else if let Ok((_, building, provisional_opt)) = queries.storage.buildings.get(ctx.task_entity)
        {
            if building.kind != crate::systems::jobs::BuildingType::Wall
                || !building.is_provisional
                || provisional_opt.is_none_or(|provisional| !provisional.mud_delivered)
            {
                error!(
                    "issue_coat_wall: Legacy wall {:?} is not ready",
                    ctx.task_entity
                );
                return;
            }
            (ctx.task_entity, Entity::PLACEHOLDER, ctx.task_entity)
        } else {
            error!(
                "issue_coat_wall: Task entity {:?} is not coatable",
                ctx.task_entity
            );
            return;
    };

    let assigned_task =
        crate::systems::soul_ai::execute::task_execution::types::AssignedTask::CoatWall(
            crate::systems::soul_ai::execute::task_execution::types::CoatWallData {
                tile: tile_entity,
                site: site_entity,
                wall: wall_entity,
                phase: CoatWallPhase::GoingToMaterialCenter,
            },
        );
    let reservation_ops = vec![ResourceReservationOp::ReserveSource {
        source: ctx.task_entity,
        amount: 1,
    }];
    submit_assignment(
        ctx,
        queries,
        shadow,
        WorkType::CoatWall,
        task_pos,
        assigned_task,
        reservation_ops,
        already_commanded,
    );
}

pub fn issue_frame_wall(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let site_entity = if let Ok(tile) = queries.storage.wall_tiles.get(ctx.task_entity) {
        tile.parent_site
    } else {
        error!(
            "issue_frame_wall: Task entity {:?} is not a WallTileBlueprint",
            ctx.task_entity
        );
        return;
    };

    let assigned_task =
        crate::systems::soul_ai::execute::task_execution::types::AssignedTask::FrameWallTile(
            crate::systems::soul_ai::execute::task_execution::types::FrameWallTileData {
                tile: ctx.task_entity,
                site: site_entity,
                phase: FrameWallPhase::GoingToMaterialCenter,
            },
        );
    let reservation_ops = vec![ResourceReservationOp::ReserveSource {
        source: ctx.task_entity,
        amount: 1,
    }];
    submit_assignment(
        ctx,
        queries,
        shadow,
        WorkType::FrameWallTile,
        task_pos,
        assigned_task,
        reservation_ops,
        already_commanded,
    );
}
