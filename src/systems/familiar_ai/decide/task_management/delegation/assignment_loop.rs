use crate::constants::TILE_SIZE;
use crate::relationships::ManagedTasks;
use crate::systems::command::TaskArea;
use crate::systems::familiar_ai::decide::task_delegation::ReachabilityCacheKey;
use crate::systems::familiar_ai::decide::task_management::{
    AssignTaskContext, DelegationCandidate, ReservationShadow, ScoredDelegationCandidate,
    assign_task_to_worker, collect_scored_candidates,
};
use crate::systems::spatial::{DesignationSpatialGrid, TransportRequestSpatialGrid};
use crate::world::map::WorldMap;
use crate::world::pathfinding::{self, PathfindingContext};
use bevy::prelude::*;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU32, Ordering as AtomicOrdering};

use crate::systems::familiar_ai::FamiliarSoulQuery;

const TASK_DELEGATION_TOP_K: usize = 24;
const MAX_ASSIGNMENT_DIST_SQ: f32 = (TILE_SIZE * 60.0) * (TILE_SIZE * 60.0);
const WORKER_SCORE_MAX_DIST_SQ: f32 = (TILE_SIZE * 80.0) * (TILE_SIZE * 80.0);
const WORKER_PRIORITY_WEIGHT: f32 = 0.65;
const WORKER_DISTANCE_WEIGHT: f32 = 0.35;

static REACHABLE_WITH_CACHE_CALLS: AtomicU32 = AtomicU32::new(0);

pub(crate) fn take_reachable_with_cache_calls() -> u32 {
    REACHABLE_WITH_CACHE_CALLS.swap(0, AtomicOrdering::Relaxed)
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
    cache: &mut HashMap<ReachabilityCacheKey, bool>,
) -> bool {
    REACHABLE_WITH_CACHE_CALLS.fetch_add(1, AtomicOrdering::Relaxed);
    let key = (worker_grid, candidate.target_grid);
    if let Some(reachable) = cache.get(&key) {
        return *reachable;
    }

    let reachable = evaluate_reachability(worker_grid, candidate, world_map, pf_context);
    cache.insert(key, reachable);
    reachable
}

fn score_for_worker(candidate: &ScoredDelegationCandidate, worker_pos: Vec2) -> f32 {
    let worker_dist_sq = worker_pos.distance_squared(candidate.pos);
    let priority_norm = ((candidate.priority as f32 + 20.0) / 40.0).clamp(0.0, 1.0);
    let dist_norm = 1.0 - (worker_dist_sq / WORKER_SCORE_MAX_DIST_SQ).min(1.0);
    priority_norm * WORKER_PRIORITY_WEIGHT + dist_norm * WORKER_DISTANCE_WEIGHT
}

fn build_worker_candidates(
    scored_candidates: &[ScoredDelegationCandidate],
    worker_pos: Vec2,
    assigned_tasks: &HashSet<Entity>,
) -> (Vec<DelegationCandidate>, Vec<(DelegationCandidate, f32)>) {
    let mut ranked: Vec<(DelegationCandidate, f32)> = scored_candidates
        .iter()
        .filter(|entry| !assigned_tasks.contains(&entry.candidate.entity))
        .filter(|entry| worker_pos.distance_squared(entry.pos) <= MAX_ASSIGNMENT_DIST_SQ)
        .map(|entry| (entry.candidate, score_for_worker(entry, worker_pos)))
        .collect();

    if ranked.is_empty() {
        return (Vec::new(), Vec::new());
    }

    let top_k = ranked.len().min(TASK_DELEGATION_TOP_K);
    if ranked.len() <= top_k {
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));
        return (
            ranked.into_iter().map(|(candidate, _)| candidate).collect(),
            Vec::new(),
        );
    }

    ranked.select_nth_unstable_by(top_k, |a, b| {
        b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal)
    });
    let mut top_ranked = ranked[..top_k].to_vec();
    top_ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));
    let fallback_ranked = ranked[top_k..].to_vec();

    (
        top_ranked
            .into_iter()
            .map(|(candidate, _)| candidate)
            .collect(),
        fallback_ranked,
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
    queries: &mut crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries,
    q_souls: &mut FamiliarSoulQuery,
    world_map: &WorldMap,
    pf_context: &mut PathfindingContext,
    reservation_shadow: &mut ReservationShadow,
    assigned_tasks: &HashSet<Entity>,
    reachability_cache: &mut HashMap<ReachabilityCacheKey, bool>,
) -> Option<Entity> {
    for candidate in candidates.iter().copied() {
        if assigned_tasks.contains(&candidate.entity) {
            continue;
        }

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
    queries: &mut crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries,
    q_souls: &mut FamiliarSoulQuery,
    designation_grid: &DesignationSpatialGrid,
    transport_request_grid: &TransportRequestSpatialGrid,
    managed_tasks: &ManagedTasks,
    world_map: &WorldMap,
    pf_context: &mut PathfindingContext,
    reservation_shadow: &mut ReservationShadow,
    reachability_cache: &mut HashMap<ReachabilityCacheKey, bool>,
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

    let mut sorted_workers = idle_members.to_vec();
    sorted_workers.sort_by(|(_, a_pos), (_, b_pos)| {
        a_pos
            .distance_squared(fam_pos)
            .partial_cmp(&b_pos.distance_squared(fam_pos))
            .unwrap_or(Ordering::Equal)
    });

    let mut first_assigned_task: Option<Entity> = None;
    let mut assigned_tasks: HashSet<Entity> = HashSet::new();

    for (worker_entity, worker_pos) in sorted_workers {
        let Some(worker_grid) = world_map.get_nearest_walkable_grid(worker_pos) else {
            continue;
        };

        let (top_candidates, mut fallback_ranked) =
            build_worker_candidates(&scored_candidates, worker_pos, &assigned_tasks);
        if top_candidates.is_empty() && fallback_ranked.is_empty() {
            continue;
        }

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
            &assigned_tasks,
            reachability_cache,
        ) {
            assigned_tasks.insert(task_entity);
            if first_assigned_task.is_none() {
                first_assigned_task = Some(task_entity);
            }
            continue;
        }

        fallback_ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));
        let fallback_candidates: Vec<DelegationCandidate> = fallback_ranked
            .into_iter()
            .map(|(candidate, _)| candidate)
            .collect();
        if let Some(task_entity) = try_assign_from_candidates(
            worker_entity,
            worker_grid,
            &fallback_candidates,
            fam_entity,
            fatigue_threshold,
            task_area_opt,
            queries,
            q_souls,
            world_map,
            pf_context,
            reservation_shadow,
            &assigned_tasks,
            reachability_cache,
        ) {
            assigned_tasks.insert(task_entity);
            if first_assigned_task.is_none() {
                first_assigned_task = Some(task_entity);
            }
        }
    }

    first_assigned_task
}
