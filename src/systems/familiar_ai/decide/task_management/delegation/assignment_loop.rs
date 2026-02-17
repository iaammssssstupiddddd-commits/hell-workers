use crate::relationships::ManagedTasks;
use crate::systems::command::TaskArea;
use crate::systems::familiar_ai::decide::task_management::{
    AssignTaskContext, DelegationCandidate, ReservationShadow, ScoredDelegationCandidate,
    assign_task_to_worker, collect_scored_candidates,
};
use crate::systems::spatial::{DesignationSpatialGrid, TransportRequestSpatialGrid};
use crate::world::map::WorldMap;
use crate::world::pathfinding::{self, PathfindingContext};
use bevy::prelude::*;
use std::cmp::Ordering;
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
        pathfinding::find_path_to_adjacent(
            world_map,
            pf_context,
            worker_grid,
            candidate.target_grid,
        )
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

fn compare_scored_candidates(
    a: &ScoredDelegationCandidate,
    b: &ScoredDelegationCandidate,
) -> Ordering {
    match b.priority.cmp(&a.priority) {
        Ordering::Equal => a.dist_sq.partial_cmp(&b.dist_sq).unwrap_or(Ordering::Equal),
        other => other,
    }
}

fn split_top_candidates(
    mut candidates: Vec<ScoredDelegationCandidate>,
    top_k: usize,
) -> (Vec<DelegationCandidate>, Vec<ScoredDelegationCandidate>) {
    if candidates.is_empty() {
        return (Vec::new(), Vec::new());
    }

    if top_k == 0 || candidates.len() <= top_k {
        candidates.sort_by(compare_scored_candidates);
        let top = candidates
            .into_iter()
            .map(|entry| entry.candidate)
            .collect();
        return (top, Vec::new());
    }

    let nth = top_k - 1;
    candidates.select_nth_unstable_by(nth, compare_scored_candidates);
    let remaining = candidates.split_off(top_k);
    candidates.sort_by(compare_scored_candidates);

    (
        candidates
            .into_iter()
            .map(|entry| entry.candidate)
            .collect(),
        remaining,
    )
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

        if !candidate.skip_reachability_check {
            if !reachable_with_cache(
                worker_grid,
                candidate,
                world_map,
                pf_context,
                reachability_cache,
            ) {
                continue;
            }
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
    let scored_candidates = collect_scored_candidates(
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
    if scored_candidates.is_empty() {
        return None;
    }

    let top_k = scored_candidates.len().min(TASK_DELEGATION_TOP_K);
    let (top_candidates, mut remaining_scored) = split_top_candidates(scored_candidates, top_k);
    let mut fallback_candidates: Option<Vec<DelegationCandidate>> = None;
    let mut reachability_cache: HashMap<ReachabilityKey, bool> = HashMap::new();

    for (worker_entity, pos) in idle_members.iter().copied() {
        let Some(worker_grid) = world_map.get_nearest_walkable_grid(pos) else {
            continue;
        };

        if let Some(task_entity) = try_assign_from_candidates(
            worker_entity,
            worker_grid,
            &top_candidates,
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

        if !remaining_scored.is_empty() {
            if fallback_candidates.is_none() {
                remaining_scored.sort_by(compare_scored_candidates);
                fallback_candidates = Some(
                    remaining_scored
                        .iter()
                        .map(|entry| entry.candidate)
                        .collect(),
                );
            }

            if let Some(fallback) = fallback_candidates.as_deref()
                && let Some(task_entity) = try_assign_from_candidates(
                    worker_entity,
                    worker_grid,
                    fallback,
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
    }

    None
}
