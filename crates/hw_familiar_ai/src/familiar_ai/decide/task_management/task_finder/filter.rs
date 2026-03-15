use bevy::prelude::*;
use hw_core::area::TaskArea;
use hw_core::relationships::ManagedTasks;
use hw_jobs::BuildingType;
use hw_jobs::WorkType;
use hw_jobs::construction::{FloorTileState, WallTileState};
use hw_spatial::{DesignationSpatialGrid, TransportRequestSpatialGrid};
use hw_world::{WorldMap, Yard};
use std::collections::HashSet;

use crate::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries;

pub(super) struct CandidateSnapshot {
    pub pos: Vec2,
    pub target_grid: (i32, i32),
    pub target_walkable: bool,
    pub skip_reachability_check: bool,
    pub work_type: WorkType,
    pub base_priority: i32,
    pub in_stockpile_none: bool,
}

pub(super) fn collect_candidate_entities(
    task_area_opt: Option<&TaskArea>,
    yards: &[Yard],
    managed_tasks: &ManagedTasks,
    designation_grid: &DesignationSpatialGrid,
    transport_request_grid: &TransportRequestSpatialGrid,
) -> Vec<Entity> {
    let mut seen = HashSet::new();

    if let Some(area) = task_area_opt {
        for &e in designation_grid.get_in_area(area.min(), area.max()).iter() {
            seen.insert(e);
        }
        for &e in transport_request_grid
            .get_in_area(area.min(), area.max())
            .iter()
        {
            seen.insert(e);
        }
    }
    for yard in yards {
        for &e in designation_grid.get_in_area(yard.min, yard.max).iter() {
            seen.insert(e);
        }
        for &e in transport_request_grid
            .get_in_area(yard.min, yard.max)
            .iter()
        {
            seen.insert(e);
        }
    }

    for &managed_entity in managed_tasks.iter() {
        seen.insert(managed_entity);
    }

    seen.into_iter().collect()
}

pub(super) fn candidate_snapshot(
    fam_entity: Entity,
    entity: Entity,
    task_area_opt: Option<&TaskArea>,
    yards: &[Yard],
    managed_tasks: &ManagedTasks,
    world_map: &WorldMap,
    queries: &FamiliarTaskAssignmentQueries,
) -> Option<CandidateSnapshot> {
    let (
        _entity,
        transform,
        designation,
        issued_by,
        slots,
        workers,
        in_stockpile_opt,
        priority_opt,
    ) = queries.designation.designations.get(entity).ok()?;

    let is_managed_by_me = managed_tasks.contains(entity);
    let is_unassigned = issued_by.is_none();
    let is_issued_by_me = issued_by.map(|ib| ib.0) == Some(fam_entity);
    let is_issued_by_yard = issued_by.is_some_and(|issuer| queries.yards.get(issuer.0).is_ok());
    let pos = transform.translation.truncate();
    let in_yard = yards.iter().any(|yard| yard.contains(pos));
    let current_workers = workers.map(|w| w.len()).unwrap_or(0);
    let is_transport_request = queries.transport_requests.get(entity).is_ok();
    let requires_transport_request = matches!(
        designation.work_type,
        WorkType::Haul
            | WorkType::HaulToMixer
            | WorkType::GatherWater
            | WorkType::HaulWaterToMixer
            | WorkType::WheelbarrowHaul
    );
    if requires_transport_request && !is_transport_request {
        return None;
    }
    let can_take_over_from_overlapping_owner = issued_by
        .filter(|issuer| issuer.0 != fam_entity)
        .is_some_and(|issuer| {
            current_workers == 0
                && task_area_opt.is_some_and(|my_area| {
                    let Ok(owner_area) = queries.familiar_task_areas.get(issuer.0) else {
                        return false;
                    };
                    if !my_area.contains(pos) || !owner_area.contains(pos) {
                        return false;
                    }
                    let overlap_w = (my_area.max().x.min(owner_area.max().x)
                        - my_area.min().x.max(owner_area.min().x))
                    .max(0.0);
                    let overlap_h = (my_area.max().y.min(owner_area.max().y)
                        - my_area.min().y.max(owner_area.min().y))
                    .max(0.0);
                    overlap_w > f32::EPSILON && overlap_h > f32::EPSILON
                })
        });

    if !is_managed_by_me
        && !is_unassigned
        && !is_issued_by_me
        && !is_issued_by_yard
        && !in_yard
        && !can_take_over_from_overlapping_owner
    {
        return None;
    }

    let max_slots = slots.map(|s| s.max).unwrap_or(1) as usize;
    if current_workers >= max_slots {
        return None;
    }

    let is_mixer_task = queries.storage.target_mixers.get(entity).is_ok();
    let is_build_task = designation.work_type == WorkType::Build;

    if let Some(area) = task_area_opt {
        if !area.contains(pos) {
            if !is_managed_by_me
                && !is_issued_by_yard
                && !is_mixer_task
                && !in_yard
                && !is_build_task
            {
                return None;
            }
        }
    } else if !is_managed_by_me
        && !is_issued_by_yard
        && !is_mixer_task
        && !in_yard
        && !is_build_task
    {
        return None;
    }

    let mut target_grid = WorldMap::world_to_grid(pos);
    let mut target_walkable = world_map.is_walkable(target_grid.0, target_grid.1);

    let is_valid = match designation.work_type {
        WorkType::Chop
        | WorkType::Mine
        | WorkType::Move
        | WorkType::Haul
        | WorkType::HaulToMixer
        | WorkType::GatherWater
        | WorkType::CollectSand
        | WorkType::CollectBone
        | WorkType::Refine
        | WorkType::HaulWaterToMixer
        | WorkType::WheelbarrowHaul => true,
        WorkType::Build => {
            if let Ok((_, bp, _)) = queries.storage.blueprints.get(entity) {
                if !bp.materials_complete() {
                    false
                } else {
                    if !target_walkable {
                        let approach_grid = bp
                            .occupied_grids
                            .iter()
                            .copied()
                            .find(|&(gx, gy)| {
                                [
                                    (0, 1),
                                    (0, -1),
                                    (1, 0),
                                    (-1, 0),
                                    (1, 1),
                                    (1, -1),
                                    (-1, 1),
                                    (-1, -1),
                                ]
                                .iter()
                                .any(|&(dx, dy)| world_map.is_walkable(gx + dx, gy + dy))
                            })
                            .or_else(|| bp.occupied_grids.first().copied());

                        if let Some(grid) = approach_grid {
                            target_grid = grid;
                            target_walkable = world_map.is_walkable(grid.0, grid.1);
                        }
                    }
                    true
                }
            } else {
                false
            }
        }
        WorkType::ReinforceFloorTile => {
            if let Ok(tile) = queries.storage.floor_tiles.get(entity) {
                matches!(tile.state, FloorTileState::ReinforcingReady)
            } else {
                false
            }
        }
        WorkType::PourFloorTile => {
            if let Ok(tile) = queries.storage.floor_tiles.get(entity) {
                matches!(tile.state, FloorTileState::PouringReady)
            } else {
                false
            }
        }
        WorkType::FrameWallTile => {
            if let Ok(tile) = queries.storage.wall_tiles.get(entity) {
                matches!(tile.state, WallTileState::FramingReady)
            } else {
                false
            }
        }
        WorkType::CoatWall => {
            if let Ok(tile) = queries.storage.wall_tiles.get(entity) {
                matches!(tile.state, WallTileState::CoatingReady) && tile.spawned_wall.is_some()
            } else if let Ok((_, building, provisional_opt)) = queries.storage.buildings.get(entity)
            {
                building.kind == BuildingType::Wall
                    && building.is_provisional
                    && provisional_opt.is_some_and(|provisional| provisional.mud_delivered)
            } else {
                false
            }
        }
    };

    if !is_valid {
        return None;
    }

    let base_priority = priority_opt.map(|p| p.0).unwrap_or(0) as i32;
    let in_stockpile_none = in_stockpile_opt.is_none();
    Some(CandidateSnapshot {
        pos,
        target_grid,
        target_walkable,
        skip_reachability_check: is_transport_request
            || matches!(designation.work_type, WorkType::Refine | WorkType::Build),
        work_type: designation.work_type,
        base_priority,
        in_stockpile_none,
    })
}
