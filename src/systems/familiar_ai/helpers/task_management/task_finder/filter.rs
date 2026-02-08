use crate::relationships::ManagedTasks;
use crate::systems::command::TaskArea;
use crate::systems::jobs::WorkType;
use crate::systems::spatial::DesignationSpatialGrid;
use crate::world::map::WorldMap;
use crate::world::pathfinding::{self, PathfindingContext};
use bevy::prelude::*;

pub(super) fn collect_candidate_entities(
    task_area_opt: Option<&TaskArea>,
    managed_tasks: &ManagedTasks,
    designation_grid: &DesignationSpatialGrid,
) -> Vec<Entity> {
    if let Some(area) = task_area_opt {
        let mut ents = designation_grid.get_in_area(area.min, area.max);

        for &managed_entity in managed_tasks.iter() {
            if !ents.contains(&managed_entity) {
                ents.push(managed_entity);
            }
        }

        ents
    } else {
        managed_tasks.iter().copied().collect::<Vec<_>>()
    }
}

pub(super) fn candidate_snapshot(
    fam_entity: Entity,
    entity: Entity,
    task_area_opt: Option<&TaskArea>,
    managed_tasks: &ManagedTasks,
    worker_pos: Vec2,
    world_map: &WorldMap,
    pf_context: &mut PathfindingContext,
    queries: &crate::systems::soul_ai::task_execution::context::TaskAssignmentQueries,
) -> Option<(Vec2, WorkType, i32, bool)> {
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

    if !is_managed_by_me && !is_unassigned && !is_issued_by_me {
        return None;
    }

    let current_workers = workers.map(|w| w.len()).unwrap_or(0);
    let max_slots = slots.map(|s| s.max).unwrap_or(1) as usize;
    if current_workers >= max_slots {
        return None;
    }

    let pos = transform.translation.truncate();
    let is_mixer_task = queries.storage.target_mixers.get(entity).is_ok();

    if let Some(area) = task_area_opt {
        if !area.contains(pos) {
            if !is_managed_by_me && !is_mixer_task {
                return None;
            }
        }
    } else if !is_managed_by_me && !is_mixer_task {
        return None;
    }

    let worker_grid = world_map.get_nearest_walkable_grid(worker_pos)?;
    let target_grid = WorldMap::world_to_grid(pos);

    let is_reachable = if world_map.is_walkable(target_grid.0, target_grid.1) {
        if pathfinding::find_path(world_map, pf_context, target_grid, worker_grid).is_some() {
            true
        } else {
            pathfinding::find_path_to_adjacent(world_map, pf_context, worker_grid, target_grid)
                .is_some()
        }
    } else {
        pathfinding::find_path_to_adjacent(world_map, pf_context, worker_grid, target_grid)
            .is_some()
    };

    if !is_reachable {
        return None;
    }

    let is_valid = match designation.work_type {
        WorkType::Chop
        | WorkType::Mine
        | WorkType::Haul
        | WorkType::HaulToMixer
        | WorkType::GatherWater
        | WorkType::CollectSand
        | WorkType::Refine
        | WorkType::HaulWaterToMixer => true,
        WorkType::Build => {
            if let Ok((_, bp, _)) = queries.storage.blueprints.get(entity) {
                bp.materials_complete()
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
    Some((pos, designation.work_type, base_priority, in_stockpile_none))
}
