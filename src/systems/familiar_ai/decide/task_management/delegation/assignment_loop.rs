use crate::relationships::ManagedTasks;
use crate::systems::command::TaskArea;
use crate::systems::familiar_ai::decide::task_management::{
    AssignTaskContext, DelegationCandidate, ReservationShadow, assign_task_to_worker,
    collect_scored_candidates,
};
use crate::systems::spatial::{DesignationSpatialGrid, TransportRequestSpatialGrid};
use crate::world::map::WorldMap;
use crate::world::pathfinding::{self, PathfindingContext};
use bevy::prelude::*;
use std::collections::HashMap;

use crate::systems::familiar_ai::FamiliarSoulQuery;

const TASK_DELEGATION_TOP_K: usize = 24;

#[derive(Clone, Copy, Eq, PartialEq, Hash)]
struct ReachabilityKey {
    worker_grid: (i32, i32),
    target_grid: (i32, i32),
}

fn evaluate_reachability(
    worker_grid: (i32, i32),
    candidate: DelegationCandidate,
    world_map: &WorldMap,
    pf_context: &mut PathfindingContext,
) -> bool {
    if candidate.target_walkable {
        pathfinding::find_path(world_map, pf_context, candidate.target_grid, worker_grid).is_some()
            || pathfinding::find_path_to_adjacent(
                world_map,
                pf_context,
                worker_grid,
                candidate.target_grid,
            )
            .is_some()
    } else {
        pathfinding::find_path_to_adjacent(world_map, pf_context, worker_grid, candidate.target_grid)
            .is_some()
    }
}

fn reachable_with_cache(
    worker_grid: (i32, i32),
    candidate: DelegationCandidate,
    world_map: &WorldMap,
    pf_context: &mut PathfindingContext,
    cache: &mut HashMap<ReachabilityKey, bool>,
) -> bool {
    let key = ReachabilityKey {
        worker_grid,
        target_grid: candidate.target_grid,
    };
    if let Some(reachable) = cache.get(&key) {
        return *reachable;
    }

    let reachable = evaluate_reachability(worker_grid, candidate, world_map, pf_context);
    cache.insert(key, reachable);
    reachable
}

#[allow(clippy::too_many_arguments)]
fn try_assign_from_candidates(
    worker_entity: Entity,
    worker_grid: (i32, i32),
    candidates: &[DelegationCandidate],
    fam_entity: Entity,
    fatigue_threshold: f32,
    task_area_opt: Option<&TaskArea>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    q_souls: &mut FamiliarSoulQuery,
    world_map: &WorldMap,
    pf_context: &mut PathfindingContext,
    reservation_shadow: &mut ReservationShadow,
    reachability_cache: &mut HashMap<ReachabilityKey, bool>,
) -> Option<Entity> {
    for candidate in candidates.iter().copied() {
        let Ok((_, _, _, _, slots, workers, _, _)) =
            queries.designation.designations.get(candidate.entity)
        else {
            continue;
        };
        let current_workers = workers.map(|w| w.len()).unwrap_or(0);
        let max_slots = slots.map(|s| s.max).unwrap_or(1) as usize;
        if current_workers >= max_slots {
            continue;
        }

        if !reachable_with_cache(
            worker_grid,
            candidate,
            world_map,
            pf_context,
            reachability_cache,
        ) {
            continue;
        }

        if assign_task_to_worker(
            AssignTaskContext {
                fam_entity,
                task_entity: candidate.entity,
                worker_entity,
                fatigue_threshold,
                task_area_opt,
            },
            queries,
            q_souls,
            reservation_shadow,
        ) {
            return Some(candidate.entity);
        }
    }

    None
}

pub(super) fn try_assign_for_workers(
    idle_members: &[(Entity, Vec2)],
    fam_entity: Entity,
    fam_pos: Vec2,
    task_area_opt: Option<&TaskArea>,
    fatigue_threshold: f32,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    q_souls: &mut FamiliarSoulQuery,
    designation_grid: &DesignationSpatialGrid,
    transport_request_grid: &TransportRequestSpatialGrid,
    managed_tasks: &ManagedTasks,
    world_map: &WorldMap,
    pf_context: &mut PathfindingContext,
    reservation_shadow: &mut ReservationShadow,
) -> Option<Entity> {
    let candidates = collect_scored_candidates(
        fam_entity,
        fam_pos,
        task_area_opt,
        queries,
        designation_grid,
        transport_request_grid,
        managed_tasks,
        &queries.storage.target_blueprints,
        world_map,
    );
    if candidates.is_empty() {
        return None;
    }

    let top_k = candidates.len().min(TASK_DELEGATION_TOP_K);
    let mut reachability_cache: HashMap<ReachabilityKey, bool> = HashMap::new();

    for (worker_entity, pos) in idle_members.iter().copied() {
        let Some(worker_grid) = world_map.get_nearest_walkable_grid(pos) else {
            continue;
        };

        if let Some(task_entity) = try_assign_from_candidates(
            worker_entity,
            worker_grid,
            &candidates[..top_k],
            fam_entity,
            fatigue_threshold,
            task_area_opt,
            queries,
            q_souls,
            world_map,
            pf_context,
            reservation_shadow,
            &mut reachability_cache,
        ) {
            return Some(task_entity);
        }

        if top_k < candidates.len()
            && let Some(task_entity) = try_assign_from_candidates(
                worker_entity,
                worker_grid,
                &candidates[top_k..],
                fam_entity,
                fatigue_threshold,
                task_area_opt,
                queries,
                q_souls,
                world_map,
                pf_context,
                reservation_shadow,
                &mut reachability_cache,
            )
        {
            return Some(task_entity);
        }
    }

    None
}
