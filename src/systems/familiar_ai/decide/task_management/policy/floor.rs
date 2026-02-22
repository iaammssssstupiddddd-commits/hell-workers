//! Floor construction task assignment policy

use crate::systems::familiar_ai::decide::task_management::{AssignTaskContext, ReservationShadow};
use bevy::prelude::*;

use super::super::builders::{
    issue_coat_wall, issue_frame_wall, issue_pour_floor, issue_reinforce_floor,
};
use super::super::validator::can_reserve_source;

pub(super) fn assign_reinforce_floor(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    // Validate tile is in correct state (already checked in filter, but double-check)
    if let Ok(tile) = queries.storage.floor_tiles.get(ctx.task_entity) {
        if !matches!(
            tile.state,
            crate::systems::jobs::floor_construction::FloorTileState::ReinforcingReady
        ) {
            debug!(
                "ASSIGN: ReinforceFloorTile target {:?} not in ReinforcingReady state",
                ctx.task_entity
            );
            return false;
        }
    } else {
        debug!(
            "ASSIGN: ReinforceFloorTile target {:?} is not a FloorTileBlueprint",
            ctx.task_entity
        );
        return false;
    }

    if !can_reserve_source(ctx.task_entity, queries, shadow) {
        return false;
    }
    issue_reinforce_floor(task_pos, already_commanded, ctx, queries, shadow);
    true
}

pub(super) fn assign_pour_floor(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    // Validate tile is in correct state (already checked in filter, but double-check)
    if let Ok(tile) = queries.storage.floor_tiles.get(ctx.task_entity) {
        if !matches!(
            tile.state,
            crate::systems::jobs::floor_construction::FloorTileState::PouringReady
        ) {
            debug!(
                "ASSIGN: PourFloorTile target {:?} not in PouringReady state",
                ctx.task_entity
            );
            return false;
        }
    } else {
        debug!(
            "ASSIGN: PourFloorTile target {:?} is not a FloorTileBlueprint",
            ctx.task_entity
        );
        return false;
    }

    if !can_reserve_source(ctx.task_entity, queries, shadow) {
        return false;
    }
    issue_pour_floor(task_pos, already_commanded, ctx, queries, shadow);
    true
}

pub(super) fn assign_coat_wall(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    let is_ready = if let Ok(tile) = queries.storage.wall_tiles.get(ctx.task_entity) {
        matches!(
            tile.state,
            crate::systems::jobs::wall_construction::WallTileState::CoatingReady
        ) && tile.spawned_wall.is_some()
    } else if let Ok((_, building, provisional_opt)) =
        queries.storage.buildings.get(ctx.task_entity)
    {
        building.kind == crate::systems::jobs::BuildingType::Wall
            && building.is_provisional
            && provisional_opt.is_some_and(|provisional| provisional.mud_delivered)
    } else {
        debug!(
            "ASSIGN: CoatWall target {:?} is not coatable",
            ctx.task_entity
        );
        return false;
    };
    if !is_ready {
        debug!("ASSIGN: CoatWall target {:?} is not ready", ctx.task_entity);
        return false;
    }

    if !can_reserve_source(ctx.task_entity, queries, shadow) {
        return false;
    }
    issue_coat_wall(task_pos, already_commanded, ctx, queries, shadow);
    true
}

pub(super) fn assign_frame_wall(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    if let Ok(tile) = queries.storage.wall_tiles.get(ctx.task_entity) {
        if !matches!(
            tile.state,
            crate::systems::jobs::wall_construction::WallTileState::FramingReady
        ) {
            debug!(
                "ASSIGN: FrameWallTile target {:?} not in FramingReady state",
                ctx.task_entity
            );
            return false;
        }
    } else {
        debug!(
            "ASSIGN: FrameWallTile target {:?} is not a WallTileBlueprint",
            ctx.task_entity
        );
        return false;
    }

    if !can_reserve_source(ctx.task_entity, queries, shadow) {
        return false;
    }
    issue_frame_wall(task_pos, already_commanded, ctx, queries, shadow);
    true
}
