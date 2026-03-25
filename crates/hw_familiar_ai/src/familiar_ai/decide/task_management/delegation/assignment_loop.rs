use bevy::prelude::*;
use hw_core::constants::TILE_SIZE;
use hw_world::WorldMap;
use hw_world::pathfinding::{self, PathfindingContext};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering as AtomicOrdering};

use super::{DelegationEnvCtx, PathfindingCtxMut};
use crate::familiar_ai::decide::task_management::context::ConstructionSitePositions;
use crate::familiar_ai::decide::task_management::{
    AssignTaskContext, DelegationCandidate, FamiliarSearchContext, FamiliarSoulQuery,
    FamiliarTaskAssignmentQueries, ReservationShadow, ScoredDelegationCandidate,
    assign_task_to_worker, collect_scored_candidates,
};

pub type ReachabilityCacheKey = ((i32, i32), (i32, i32));

const TASK_DELEGATION_TOP_K: usize = 24;
const MAX_ASSIGNMENT_DIST_SQ: f32 = (TILE_SIZE * 60.0) * (TILE_SIZE * 60.0);
const WORKER_SCORE_MAX_DIST_SQ: f32 = (TILE_SIZE * 80.0) * (TILE_SIZE * 80.0);
const WORKER_PRIORITY_WEIGHT: f32 = 0.65;
const WORKER_DISTANCE_WEIGHT: f32 = 0.35;

static REACHABLE_WITH_CACHE_CALLS: AtomicU32 = AtomicU32::new(0);

pub fn take_reachable_with_cache_calls() -> u32 {
    REACHABLE_WITH_CACHE_CALLS.swap(0, AtomicOrdering::Relaxed)
}

fn evaluate_reachability(
    worker_grid: (i32, i32),
    candidate: DelegationCandidate,
    world_map: &WorldMap,
    pf_context: &mut PathfindingContext,
) -> bool {
    pathfinding::can_reach_target(
        world_map,
        pf_context,
        worker_grid,
        candidate.target_grid,
        candidate.target_walkable,
    )
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
    task_virtual_workers: &HashMap<Entity, usize>,
    queries: &FamiliarTaskAssignmentQueries,
) -> (Vec<DelegationCandidate>, Vec<(DelegationCandidate, f32)>) {
    let mut ranked: Vec<(DelegationCandidate, f32)> = scored_candidates
        .iter()
        .filter(|entry| {
            let virtual_count = task_virtual_workers
                .get(&entry.candidate.entity)
                .copied()
                .unwrap_or(0);
            if virtual_count == 0 {
                return true;
            }
            // virtual割り当てがある場合、ECSのmax_slotsと比較して残スロットを確認
            let Ok((_, _, _, _, slots, workers, _, _)) =
                queries.designation.designations.get(entry.candidate.entity)
            else {
                return false;
            };
            let current_workers = workers.map(|w| w.len()).unwrap_or(0);
            let max_slots = slots.map(|s| s.max).unwrap_or(1) as usize;
            current_workers + virtual_count < max_slots
        })
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

/// `try_assign_from_candidates` の per-worker データをまとめた構造体。
struct WorkerAssignCtx<'a> {
    worker_entity: Entity,
    worker_grid: (i32, i32),
    candidates: &'a [DelegationCandidate],
    task_virtual_workers: &'a HashMap<Entity, usize>,
}

fn try_assign_from_candidates(
    worker_ctx: WorkerAssignCtx<'_>,
    env: &DelegationEnvCtx<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    construction_sites: &impl ConstructionSitePositions,
    q_souls: &mut FamiliarSoulQuery,
    pf_ctx: &mut PathfindingCtxMut<'_>,
    reservation_shadow: &mut ReservationShadow,
) -> Option<Entity> {
    for candidate in worker_ctx.candidates.iter().copied() {
        let Ok((_, _, _, _, slots, workers, _, _)) =
            queries.designation.designations.get(candidate.entity)
        else {
            continue;
        };
        let current_workers = workers.map(|w| w.len()).unwrap_or(0);
        let max_slots = slots.map(|s| s.max).unwrap_or(1) as usize;
        let virtual_workers = worker_ctx.task_virtual_workers
            .get(&candidate.entity)
            .copied()
            .unwrap_or(0);
        if current_workers + virtual_workers >= max_slots {
            continue;
        }

        if !candidate.skip_reachability_check
            && !reachable_with_cache(
                worker_ctx.worker_grid,
                candidate,
                env.world_map,
                pf_ctx.pf_context,
                pf_ctx.reachability_cache,
            ) {
                continue;
            }

        if assign_task_to_worker(
            AssignTaskContext {
                fam_entity: env.fam_entity,
                task_entity: candidate.entity,
                worker_entity: worker_ctx.worker_entity,
                fatigue_threshold: env.fatigue_threshold,
                task_area_opt: env.task_area_opt,
                resource_grid: env.resource_grid,
                tile_site_index: env.tile_site_index,
                incoming_snapshot: env.incoming_snapshot,
            },
            queries,
            construction_sites,
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
    env: &DelegationEnvCtx<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    construction_sites: &impl ConstructionSitePositions,
    q_souls: &mut FamiliarSoulQuery,
    pf_ctx: &mut PathfindingCtxMut<'_>,
    reservation_shadow: &mut ReservationShadow,
) -> Option<Entity> {
    let scored_candidates = collect_scored_candidates(
        FamiliarSearchContext { fam_entity: env.fam_entity, fam_pos: env.fam_pos, task_area_opt: env.task_area_opt },
        queries,
        env.designation_grid,
        env.transport_request_grid,
        env.managed_tasks,
        &queries.storage.target_blueprints,
        env.world_map,
    );
    if scored_candidates.is_empty() {
        return None;
    }

    let mut sorted_workers = idle_members.to_vec();
    sorted_workers.sort_by(|(_, a_pos), (_, b_pos)| {
        a_pos
            .distance_squared(env.fam_pos)
            .partial_cmp(&b_pos.distance_squared(env.fam_pos))
            .unwrap_or(Ordering::Equal)
    });

    let mut first_assigned_task: Option<Entity> = None;
    let mut task_virtual_workers: HashMap<Entity, usize> = HashMap::new();

    for (worker_entity, worker_pos) in sorted_workers {
        let Some(worker_grid) = env.world_map.get_nearest_walkable_grid(worker_pos) else {
            continue;
        };

        let (top_candidates, mut fallback_ranked) = build_worker_candidates(
            &scored_candidates,
            worker_pos,
            &task_virtual_workers,
            queries,
        );
        if top_candidates.is_empty() && fallback_ranked.is_empty() {
            continue;
        }

        if let Some(task_entity) = try_assign_from_candidates(
            WorkerAssignCtx {
                worker_entity,
                worker_grid,
                candidates: &top_candidates,
                task_virtual_workers: &task_virtual_workers,
            },
            env,
            queries,
            construction_sites,
            q_souls,
            pf_ctx,
            reservation_shadow,
        ) {
            *task_virtual_workers.entry(task_entity).or_insert(0) += 1;
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
            WorkerAssignCtx {
                worker_entity,
                worker_grid,
                candidates: &fallback_candidates,
                task_virtual_workers: &task_virtual_workers,
            },
            env,
            queries,
            construction_sites,
            q_souls,
            pf_ctx,
            reservation_shadow,
        ) {
            *task_virtual_workers.entry(task_entity).or_insert(0) += 1;
            if first_assigned_task.is_none() {
                first_assigned_task = Some(task_entity);
            }
        }
    }

    first_assigned_task
}
