use bevy::prelude::*;
use hw_jobs::WorkType;
use hw_jobs::{
    AssignedTask, BuildData, BuildPhase, BuildingType, CoatWallData, CoatWallPhase,
    CollectBoneData, CollectBonePhase, CollectSandData, CollectSandPhase, FrameWallPhase,
    FrameWallTileData, GatherData, GatherPhase, GeneratePowerData, GeneratePowerPhase,
    MovePlantData, MovePlantPhase, PourFloorPhase, PourFloorTileData, RefineData, RefinePhase,
    ReinforceFloorPhase, ReinforceFloorTileData,
};

use super::{
    TaskTarget, submit_assignment_with_reservation_ops, submit_assignment_with_source_entities,
};
use crate::familiar_ai::decide::task_management::{
    AssignTaskContext, FamiliarTaskAssignmentQueries, ReservationShadow,
};

pub fn issue_gather(
    work_type: WorkType,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let assigned_task = AssignedTask::Gather(GatherData {
        target: ctx.task_entity,
        work_type,
        phase: GatherPhase::GoingToResource,
    });
    submit_assignment_with_source_entities(
        ctx,
        queries,
        shadow,
        TaskTarget {
            work_type,
            task_pos,
        },
        assigned_task,
        &[ctx.task_entity],
        already_commanded,
    );
}

pub fn issue_build(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let assigned_task = AssignedTask::Build(BuildData {
        blueprint: ctx.task_entity,
        phase: BuildPhase::GoingToBlueprint,
    });
    submit_assignment_with_source_entities(
        ctx,
        queries,
        shadow,
        TaskTarget {
            work_type: WorkType::Build,
            task_pos,
        },
        assigned_task,
        &[ctx.task_entity],
        already_commanded,
    );
}

pub fn issue_collect_sand(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let assigned_task = AssignedTask::CollectSand(CollectSandData {
        target: ctx.task_entity,
        phase: CollectSandPhase::GoingToSand,
    });
    submit_assignment_with_source_entities(
        ctx,
        queries,
        shadow,
        TaskTarget {
            work_type: WorkType::CollectSand,
            task_pos,
        },
        assigned_task,
        &[ctx.task_entity],
        already_commanded,
    );
}

pub fn issue_refine(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let assigned_task = AssignedTask::Refine(RefineData {
        mixer: ctx.task_entity,
        phase: RefinePhase::GoingToMixer,
    });
    submit_assignment_with_source_entities(
        ctx,
        queries,
        shadow,
        TaskTarget {
            work_type: WorkType::Refine,
            task_pos,
        },
        assigned_task,
        &[ctx.task_entity],
        already_commanded,
    );
}

pub fn issue_collect_bone(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let assigned_task = AssignedTask::CollectBone(CollectBoneData {
        target: ctx.task_entity,
        phase: CollectBonePhase::GoingToBone,
    });
    submit_assignment_with_source_entities(
        ctx,
        queries,
        shadow,
        TaskTarget {
            work_type: WorkType::CollectBone,
            task_pos,
        },
        assigned_task,
        &[ctx.task_entity],
        already_commanded,
    );
}

pub fn issue_reinforce_floor(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let site_entity = if let Ok(tile) = queries.storage.floor_tiles.get(ctx.task_entity) {
        tile.parent_site
    } else {
        error!(
            "issue_reinforce_floor: Task entity {:?} is not a FloorTileBlueprint",
            ctx.task_entity
        );
        return;
    };

    let assigned_task = AssignedTask::ReinforceFloorTile(ReinforceFloorTileData {
        tile: ctx.task_entity,
        site: site_entity,
        phase: ReinforceFloorPhase::GoingToMaterialCenter,
    });
    submit_assignment_with_source_entities(
        ctx,
        queries,
        shadow,
        TaskTarget {
            work_type: WorkType::ReinforceFloorTile,
            task_pos,
        },
        assigned_task,
        &[ctx.task_entity],
        already_commanded,
    );
}

pub fn issue_pour_floor(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let site_entity = if let Ok(tile) = queries.storage.floor_tiles.get(ctx.task_entity) {
        tile.parent_site
    } else {
        error!(
            "issue_pour_floor: Task entity {:?} is not a FloorTileBlueprint",
            ctx.task_entity
        );
        return;
    };

    let assigned_task = AssignedTask::PourFloorTile(PourFloorTileData {
        tile: ctx.task_entity,
        site: site_entity,
        phase: PourFloorPhase::GoingToMaterialCenter,
    });
    submit_assignment_with_source_entities(
        ctx,
        queries,
        shadow,
        TaskTarget {
            work_type: WorkType::PourFloorTile,
            task_pos,
        },
        assigned_task,
        &[ctx.task_entity],
        already_commanded,
    );
}

pub fn issue_coat_wall(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
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
        } else if let Ok((_, building, provisional_opt)) =
            queries.storage.buildings.get(ctx.task_entity)
        {
            if building.kind != BuildingType::Wall
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

    let assigned_task = AssignedTask::CoatWall(CoatWallData {
        tile: tile_entity,
        site: site_entity,
        wall: wall_entity,
        phase: CoatWallPhase::GoingToMaterialCenter,
    });
    submit_assignment_with_source_entities(
        ctx,
        queries,
        shadow,
        TaskTarget {
            work_type: WorkType::CoatWall,
            task_pos,
        },
        assigned_task,
        &[ctx.task_entity],
        already_commanded,
    );
}

pub fn issue_frame_wall(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
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

    let assigned_task = AssignedTask::FrameWallTile(FrameWallTileData {
        tile: ctx.task_entity,
        site: site_entity,
        phase: FrameWallPhase::GoingToMaterialCenter,
    });
    submit_assignment_with_source_entities(
        ctx,
        queries,
        shadow,
        TaskTarget {
            work_type: WorkType::FrameWallTile,
            task_pos,
        },
        assigned_task,
        &[ctx.task_entity],
        already_commanded,
    );
}

pub fn issue_move(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let Ok(task_template) = queries.move_plant_tasks.get(ctx.task_entity) else {
        warn!(
            "issue_move: Missing task template for {:?}",
            ctx.task_entity
        );
        return;
    };

    let assigned_task = AssignedTask::MovePlant(MovePlantData {
        task_entity: ctx.task_entity,
        building: task_template.building,
        destination_grid: task_template.destination_grid,
        destination_pos: task_template.destination_pos,
        companion_anchor: task_template.companion_anchor,
        phase: MovePlantPhase::GoToBuilding,
    });

    submit_assignment_with_reservation_ops(
        ctx,
        queries,
        shadow,
        TaskTarget {
            work_type: WorkType::Move,
            task_pos,
        },
        assigned_task,
        Vec::new(),
        already_commanded,
    );
}

pub fn issue_generate_power(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let assigned_task = AssignedTask::GeneratePower(GeneratePowerData {
        tile: ctx.task_entity,
        tile_pos: task_pos,
        phase: GeneratePowerPhase::GoingToTile,
    });
    submit_assignment_with_source_entities(
        ctx,
        queries,
        shadow,
        TaskTarget {
            work_type: WorkType::GeneratePower,
            task_pos,
        },
        assigned_task,
        &[ctx.task_entity],
        already_commanded,
    );
}
