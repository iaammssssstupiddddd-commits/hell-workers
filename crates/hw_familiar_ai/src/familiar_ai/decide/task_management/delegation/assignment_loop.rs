use bevy::prelude::*;
use hw_core::constants::TILE_SIZE;
use hw_world::{WalkabilityConnectivityCache, WorldMap};
use std::cmp::Ordering;
use std::collections::HashMap;
#[cfg(feature = "profiling")]
use std::sync::atomic::{AtomicU32, Ordering as AtomicOrdering};

use super::DelegationEnvCtx;
use crate::familiar_ai::decide::task_management::context::ConstructionSitePositions;
use crate::familiar_ai::decide::task_management::{
    AssignTaskContext, DelegationCandidate, FamiliarSearchContext, FamiliarSoulQuery,
    FamiliarTaskAssignmentQueries, ReservationShadow, ScoredDelegationCandidate,
    assign_task_to_worker, collect_scored_candidates,
};

const TASK_DELEGATION_TOP_K: usize = 24;
const MAX_ASSIGNMENT_DIST_SQ: f32 = (TILE_SIZE * 60.0) * (TILE_SIZE * 60.0);
const WORKER_SCORE_MAX_DIST_SQ: f32 = (TILE_SIZE * 80.0) * (TILE_SIZE * 80.0);
const WORKER_PRIORITY_WEIGHT: f32 = 0.65;
const WORKER_DISTANCE_WEIGHT: f32 = 0.35;

#[cfg(feature = "profiling")]
static REACHABLE_WITH_CACHE_CALLS: AtomicU32 = AtomicU32::new(0);

#[cfg(feature = "profiling")]
pub fn take_reachable_with_cache_calls() -> u32 {
    REACHABLE_WITH_CACHE_CALLS.swap(0, AtomicOrdering::Relaxed)
}

#[cfg(not(feature = "profiling"))]
pub fn take_reachable_with_cache_calls() -> u32 {
    0
}

fn reachable_with_cache(
    worker_grid: (i32, i32),
    candidate: DelegationCandidate,
    world_map: &WorldMap,
    connectivity_cache: &mut WalkabilityConnectivityCache,
) -> bool {
    #[cfg(feature = "profiling")]
    REACHABLE_WITH_CACHE_CALLS.fetch_add(1, AtomicOrdering::Relaxed);
    connectivity_cache.can_reach_target(
        world_map,
        worker_grid,
        candidate.target_grid,
        candidate.target_walkable,
    )
}

fn score_for_worker(candidate: &ScoredDelegationCandidate, worker_pos: Vec2) -> f32 {
    let worker_dist_sq = worker_pos.distance_squared(candidate.pos);
    let priority_norm = ((candidate.priority as f32 + 20.0) / 40.0).clamp(0.0, 1.0);
    let dist_norm = 1.0 - (worker_dist_sq / WORKER_SCORE_MAX_DIST_SQ).min(1.0);
    priority_norm * WORKER_PRIORITY_WEIGHT + dist_norm * WORKER_DISTANCE_WEIGHT
}

/// 同点の候補をEntity IDで一意に順序付ける。
///
/// task finderの候補集合は複数のspatial gridとHashSetを経由するため、入力順を
/// assignmentの意味にしてはいけない。scoreだけの比較では同点候補の優先順位が
/// HashSetのhash seedに依存し、fixed-step auditで異なるSoulへ同じtaskが割り当て
/// られる。通常実行でも同じtie-breakを使い、比較の全順序を保つ。
fn compare_ranked_candidates(
    left: &(DelegationCandidate, f32),
    right: &(DelegationCandidate, f32),
) -> Ordering {
    right
        .1
        .total_cmp(&left.1)
        .then_with(|| compare_entity_keys(left.0.entity, right.0.entity))
}

/// Familiarからの距離が同じworkerをEntity IDで一意に順序付ける。
fn compare_workers(
    familiar_position: Vec2,
    left: &(Entity, Vec2),
    right: &(Entity, Vec2),
) -> Ordering {
    left.1
        .distance_squared(familiar_position)
        .total_cmp(&right.1.distance_squared(familiar_position))
        .then_with(|| compare_entity_keys(left.0, right.0))
}

/// Entityの内部表現順ではなく、fixtureで安定したindex/generation順を使う。
fn compare_entity_keys(left: Entity, right: Entity) -> Ordering {
    left.index_u32().cmp(&right.index_u32()).then_with(|| {
        left.generation()
            .to_bits()
            .cmp(&right.generation().to_bits())
    })
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
        ranked.sort_unstable_by(compare_ranked_candidates);
        return (
            ranked.into_iter().map(|(candidate, _)| candidate).collect(),
            Vec::new(),
        );
    }

    ranked.select_nth_unstable_by(top_k, compare_ranked_candidates);
    ranked[..top_k].sort_unstable_by(compare_ranked_candidates);
    let top: Vec<DelegationCandidate> = ranked[..top_k].iter().map(|(c, _)| *c).collect();
    let fallback: Vec<(DelegationCandidate, f32)> = ranked[top_k..].to_vec();

    (top, fallback)
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
    connectivity_cache: &mut WalkabilityConnectivityCache,
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
        let virtual_workers = worker_ctx
            .task_virtual_workers
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
                connectivity_cache,
            )
        {
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
    connectivity_cache: &mut WalkabilityConnectivityCache,
    reservation_shadow: &mut ReservationShadow,
) -> Option<Entity> {
    let scored_candidates = collect_scored_candidates(
        FamiliarSearchContext {
            fam_entity: env.fam_entity,
            fam_pos: env.fam_pos,
            task_area_opt: env.task_area_opt,
        },
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
    sorted_workers.sort_unstable_by(|left, right| compare_workers(env.fam_pos, left, right));

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
            connectivity_cache,
            reservation_shadow,
        ) {
            *task_virtual_workers.entry(task_entity).or_insert(0) += 1;
            if first_assigned_task.is_none() {
                first_assigned_task = Some(task_entity);
            }
            continue;
        }

        fallback_ranked.sort_unstable_by(compare_ranked_candidates);
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
            connectivity_cache,
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

#[cfg(test)]
mod tests {
    use super::{DelegationCandidate, compare_ranked_candidates, compare_workers};
    use bevy::prelude::{Entity, Vec2};

    fn entity(index: u32) -> Entity {
        Entity::from_raw_u32(index).expect("test entity index is valid")
    }

    fn candidate(index: u32) -> DelegationCandidate {
        DelegationCandidate {
            entity: entity(index),
            target_grid: (0, 0),
            target_walkable: true,
            skip_reachability_check: false,
        }
    }

    #[test]
    fn equal_score_candidates_use_entity_id_as_a_total_order() {
        let mut candidates = [
            (candidate(9), 0.5),
            (candidate(2), 0.5),
            (candidate(5), 0.5),
        ];

        candidates.sort_unstable_by(compare_ranked_candidates);

        assert_eq!(
            candidates
                .iter()
                .map(|(candidate, _)| candidate.entity)
                .collect::<Vec<_>>(),
            vec![entity(2), entity(5), entity(9)]
        );
    }

    #[test]
    fn equal_distance_workers_use_entity_id_as_a_total_order() {
        let familiar_position = Vec2::ZERO;
        let mut workers = vec![
            (entity(8), Vec2::new(3.0, 4.0)),
            (entity(1), Vec2::new(0.0, 5.0)),
            (entity(4), Vec2::new(-3.0, 4.0)),
        ];

        workers.sort_unstable_by(|left, right| compare_workers(familiar_position, left, right));

        assert_eq!(
            workers
                .into_iter()
                .map(|(entity, _)| entity)
                .collect::<Vec<_>>(),
            vec![entity(1), entity(4), entity(8)]
        );
    }
}
