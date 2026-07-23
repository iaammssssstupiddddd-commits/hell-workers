use bevy::prelude::*;
use hw_core::constants::TILE_SIZE;
use hw_logistics::transport_request::{
    TransportRequest, WheelbarrowArbitrationHeader, WheelbarrowArbitrationOutcome,
    is_wheelbarrow_arbitration_applicable,
};
use hw_world::{WalkabilityConnectivityCache, WorldMap};
use std::cmp::Ordering;
use std::collections::HashMap;
#[cfg(feature = "profiling")]
use std::sync::atomic::{AtomicU32, Ordering as AtomicOrdering};

use super::{DelegationDiagnosticsCtx, DelegationEnvCtx, DelegationScratchCtx};
use crate::familiar_ai::decide::task_management::CandidateRejectReason;
use crate::familiar_ai::decide::task_management::context::ConstructionSitePositions;
use crate::familiar_ai::decide::task_management::policy_score::{
    WORKER_DISTANCE_WEIGHT, WORKER_PRIORITY_WEIGHT, compose_worker_score,
};
use crate::familiar_ai::decide::task_management::task_finder::{
    FamiliarCandidateSources, collect_scored_candidates_with_diagnostics,
};
use crate::familiar_ai::decide::task_management::{
    AssignTaskContext, DelegationCandidate, FamiliarEvaluatorDiagnostics, FamiliarSearchContext,
    FamiliarSoulQuery, FamiliarTaskAssignmentQueries, ScoredDelegationCandidate,
    TaskAssignmentAttempt, assign_task_to_worker,
};

const TASK_DELEGATION_TOP_K: usize = 24;
const MAX_ASSIGNMENT_DIST_SQ: f32 = (TILE_SIZE * 60.0) * (TILE_SIZE * 60.0);
const WORKER_SCORE_MAX_DIST_SQ: f32 = (TILE_SIZE * 80.0) * (TILE_SIZE * 80.0);

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
    let base_score = priority_norm * WORKER_PRIORITY_WEIGHT + dist_norm * WORKER_DISTANCE_WEIGHT;
    compose_worker_score(base_score, candidate.policy_contributions)
}

fn worker_distance_rejection(
    worker_pos: Vec2,
    candidate_pos: Vec2,
) -> Option<CandidateRejectReason> {
    (worker_pos.distance_squared(candidate_pos) > MAX_ASSIGNMENT_DIST_SQ)
        .then_some(CandidateRejectReason::NoEligibleFamiliar)
}

const fn connectivity_rejection(reachable: bool) -> Option<CandidateRejectReason> {
    if reachable {
        None
    } else {
        Some(CandidateRejectReason::Unreachable)
    }
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
    let ranked: Vec<(DelegationCandidate, f32)> = scored_candidates
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

    partition_ranked_candidates(ranked)
}

fn partition_ranked_candidates(
    mut ranked: Vec<(DelegationCandidate, f32)>,
) -> (Vec<DelegationCandidate>, Vec<(DelegationCandidate, f32)>) {
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
    scratch: &mut DelegationScratchCtx<'_>,
    diagnostics: &mut FamiliarEvaluatorDiagnostics,
) -> Option<Entity> {
    for candidate in worker_ctx.candidates.iter().copied() {
        let arbitration_reason = wheelbarrow_arbitration_reason(candidate, queries);
        let Ok((_, _, _, _, slots, workers, _, _)) =
            queries.designation.designations.get(candidate.entity)
        else {
            diagnostics.mark_partial(candidate.entity);
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
            diagnostics.reject(candidate.entity, CandidateRejectReason::NoEligibleFamiliar);
            continue;
        }

        if !candidate.skip_reachability_check
            && let Some(reason) = connectivity_rejection(reachable_with_cache(
                worker_ctx.worker_grid,
                candidate,
                env.world_map,
                scratch.connectivity_cache,
            ))
        {
            diagnostics.reject(candidate.entity, reason);
            continue;
        }

        let assignment = assign_task_to_worker(
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
            scratch.reservation_shadow,
        );
        match assignment {
            TaskAssignmentAttempt::Submitted => {
                diagnostics.mark_submitted(candidate.entity);
                return Some(candidate.entity);
            }
            TaskAssignmentAttempt::Rejected(reason) => {
                diagnostics.reject(candidate.entity, arbitration_reason.unwrap_or(reason));
            }
        }
    }

    None
}

fn wheelbarrow_arbitration_reason(
    candidate: DelegationCandidate,
    queries: &FamiliarTaskAssignmentQueries,
) -> Option<CandidateRejectReason> {
    let request = queries.transport_requests.get(candidate.entity).ok()?;
    let diagnostics = &queries.wheelbarrow_arbitration_diagnostics;
    wheelbarrow_arbitration_reason_from_evidence(
        request,
        queries.wheelbarrow_leases.get(candidate.entity).is_ok(),
        diagnostics.header(),
        diagnostics.outcome(candidate.entity),
        queries.reservation.resource_cache.semantic_generation(),
    )
}

fn wheelbarrow_arbitration_reason_from_evidence(
    request: &TransportRequest,
    has_lease: bool,
    header: Option<&WheelbarrowArbitrationHeader>,
    outcome: Option<WheelbarrowArbitrationOutcome>,
    availability_generation: u64,
) -> Option<CandidateRejectReason> {
    if !is_wheelbarrow_arbitration_applicable(request) || has_lease {
        return None;
    }

    let Some(header) = header else {
        return Some(CandidateRejectReason::Unevaluated);
    };
    if header.availability_generation != availability_generation {
        return Some(CandidateRejectReason::StaleInput);
    }
    if header.available_vehicle_count == 0 {
        return Some(if header.any_vehicle_exists {
            CandidateRejectReason::TemporaryContention
        } else {
            CandidateRejectReason::MissingResourceOrSource
        });
    }

    match outcome {
        // A granted outcome without a visible lease means Commands have not
        // become observable yet. It is not evidence for a terminal blocker.
        Some(WheelbarrowArbitrationOutcome::LeaseGranted) => {
            Some(CandidateRejectReason::Unevaluated)
        }
        Some(WheelbarrowArbitrationOutcome::NoAvailableWheelbarrow) => {
            Some(if header.any_vehicle_exists {
                CandidateRejectReason::TemporaryContention
            } else {
                CandidateRejectReason::MissingResourceOrSource
            })
        }
        Some(
            WheelbarrowArbitrationOutcome::NoSourceItems
            | WheelbarrowArbitrationOutcome::NoDestinationCapacity,
        ) => Some(CandidateRejectReason::MissingResourceOrSource),
        Some(
            WheelbarrowArbitrationOutcome::SourceReserved
            | WheelbarrowArbitrationOutcome::CapacityReserved
            | WheelbarrowArbitrationOutcome::PreferredBatchWaiting
            | WheelbarrowArbitrationOutcome::ArbitrationContention,
        ) => Some(CandidateRejectReason::TemporaryContention),
        Some(
            WheelbarrowArbitrationOutcome::NotApplicable
            | WheelbarrowArbitrationOutcome::DemandGone
            | WheelbarrowArbitrationOutcome::StaleInput,
        ) => Some(CandidateRejectReason::Unevaluated),
        None => Some(CandidateRejectReason::Unevaluated),
    }
}

pub(super) fn try_assign_for_workers(
    idle_members: &[(Entity, Vec2)],
    env: &DelegationEnvCtx<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    construction_sites: &impl ConstructionSitePositions,
    q_souls: &mut FamiliarSoulQuery,
    scratch: &mut DelegationScratchCtx<'_>,
    diagnostics: &mut DelegationDiagnosticsCtx<'_>,
) -> Option<Entity> {
    let scored_candidates = collect_scored_candidates_with_diagnostics(
        FamiliarSearchContext {
            fam_entity: env.fam_entity,
            fam_pos: env.fam_pos,
            task_area_opt: env.task_area_opt,
        },
        queries,
        FamiliarCandidateSources {
            designation_grid: env.designation_grid,
            transport_request_grid: env.transport_request_grid,
            managed_tasks: env.managed_tasks,
            world_map: env.world_map,
        },
        &queries.storage.target_blueprints,
        diagnostics.evaluator,
        diagnostics.revisions,
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
            for candidate in &scored_candidates {
                diagnostics.evaluator.reject(
                    candidate.candidate.entity,
                    CandidateRejectReason::NoEligibleFamiliar,
                );
            }
            continue;
        };

        for candidate in &scored_candidates {
            if let Some(reason) = worker_distance_rejection(worker_pos, candidate.pos) {
                diagnostics
                    .evaluator
                    .reject(candidate.candidate.entity, reason);
            }
        }

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
            scratch,
            diagnostics.evaluator,
        ) {
            *task_virtual_workers.entry(task_entity).or_insert(0) += 1;
            if first_assigned_task.is_none() {
                first_assigned_task = Some(task_entity);
            }
            for candidate in &scored_candidates {
                if candidate.candidate.entity != task_entity {
                    diagnostics
                        .evaluator
                        .mark_partial(candidate.candidate.entity);
                }
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
            scratch,
            diagnostics.evaluator,
        ) {
            *task_virtual_workers.entry(task_entity).or_insert(0) += 1;
            if first_assigned_task.is_none() {
                first_assigned_task = Some(task_entity);
            }
            for candidate in &scored_candidates {
                if candidate.candidate.entity != task_entity {
                    diagnostics
                        .evaluator
                        .mark_partial(candidate.candidate.entity);
                }
            }
        }
    }

    first_assigned_task
}

#[cfg(test)]
mod tests {
    use super::{
        DelegationCandidate, compare_ranked_candidates, compare_workers, connectivity_rejection,
        partition_ranked_candidates, score_for_worker,
        wheelbarrow_arbitration_reason_from_evidence, worker_distance_rejection,
    };
    use bevy::prelude::{Entity, Vec2};
    use hw_jobs::WorkType;
    use hw_logistics::ResourceType;
    use hw_logistics::transport_request::{
        TransportPriority, TransportRequest, TransportRequestKind, WheelbarrowArbitrationHeader,
        WheelbarrowArbitrationOutcome, is_wheelbarrow_arbitration_applicable,
    };

    use crate::familiar_ai::decide::task_management::policy_score::{
        POLICY_SCORE_UNIT, PolicyScoreContributions, transport_policy_units,
    };
    use crate::familiar_ai::decide::task_management::{
        CandidateRejectReason, ScoredDelegationCandidate,
    };

    fn entity(index: u32) -> Entity {
        Entity::from_raw_u32(index).expect("test entity index is valid")
    }

    fn candidate(index: u32) -> DelegationCandidate {
        DelegationCandidate {
            entity: entity(index),
            work_type: WorkType::Chop,
            target_grid: (0, 0),
            target_walkable: true,
            skip_reachability_check: false,
        }
    }

    fn scored_candidate(
        index: u32,
        transport_priority: TransportPriority,
    ) -> ScoredDelegationCandidate {
        ScoredDelegationCandidate {
            candidate: candidate(index),
            priority: 20,
            pos: Vec2::ZERO,
            dist_sq: 0.0,
            policy_contributions: PolicyScoreContributions::new(
                transport_policy_units(transport_priority),
                0,
            ),
        }
    }

    fn scored_candidate_with_base_priority(
        index: u32,
        base_priority: i32,
        transport_priority: TransportPriority,
    ) -> ScoredDelegationCandidate {
        ScoredDelegationCandidate {
            priority: base_priority,
            ..scored_candidate(index, transport_priority)
        }
    }

    #[test]
    fn worker_score_applies_candidate_policy_once_without_final_clamp() {
        let normal = score_for_worker(&scored_candidate(1, TransportPriority::Normal), Vec2::ZERO);
        let low = score_for_worker(&scored_candidate(2, TransportPriority::Low), Vec2::ZERO);
        let high = score_for_worker(&scored_candidate(3, TransportPriority::High), Vec2::ZERO);
        let critical = score_for_worker(
            &scored_candidate(4, TransportPriority::Critical),
            Vec2::ZERO,
        );

        assert_eq!(normal.to_bits(), 1.0f32.to_bits());
        assert!(low < normal && normal < high && high < critical);
        assert!((critical - (1.0 + 20.0 * POLICY_SCORE_UNIT)).abs() < f32::EPSILON * 2.0);
        assert!(critical > 1.0);
    }

    #[test]
    fn policy_score_changes_the_twenty_four_candidate_top_k_boundary() {
        let mut scored: Vec<_> = (1..=24)
            .map(|index| scored_candidate(index, TransportPriority::Normal))
            .collect();
        scored.push(scored_candidate_with_base_priority(
            25,
            19,
            TransportPriority::Critical,
        ));
        let ranked = scored
            .iter()
            .map(|candidate| (candidate.candidate, score_for_worker(candidate, Vec2::ZERO)))
            .collect();

        let (top, fallback) = partition_ranked_candidates(ranked);

        assert_eq!(top.len(), 24);
        assert!(top.iter().any(|candidate| candidate.entity == entity(25)));
        assert_eq!(fallback.len(), 1);
        assert_eq!(fallback[0].0.entity, entity(24));
    }

    #[test]
    fn fallback_keeps_the_same_composed_policy_rank() {
        let mut scored: Vec<_> = (1..=24)
            .map(|index| scored_candidate(index, TransportPriority::Critical))
            .collect();
        scored.extend([
            scored_candidate_with_base_priority(25, -20, TransportPriority::Low),
            scored_candidate_with_base_priority(26, -20, TransportPriority::Critical),
        ]);
        let ranked = scored
            .iter()
            .map(|candidate| (candidate.candidate, score_for_worker(candidate, Vec2::ZERO)))
            .collect();

        let (_, mut fallback) = partition_ranked_candidates(ranked);
        fallback.sort_unstable_by(compare_ranked_candidates);

        assert_eq!(fallback.len(), 2);
        assert_eq!(fallback[0].0.entity, entity(26));
        assert_eq!(fallback[1].0.entity, entity(25));
        assert!(fallback[0].1 > fallback[1].1);
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

    #[test]
    fn distance_limit_and_connectivity_use_distinct_rejection_classes() {
        assert_eq!(
            worker_distance_rejection(Vec2::ZERO, Vec2::splat(f32::MAX)),
            Some(CandidateRejectReason::NoEligibleFamiliar)
        );
        assert_eq!(
            connectivity_rejection(false),
            Some(CandidateRejectReason::Unreachable)
        );
        assert_eq!(connectivity_rejection(true), None);
    }

    #[test]
    fn haul_work_types_use_transport_request_arbitration_evidence() {
        let header = WheelbarrowArbitrationHeader {
            generation: 1,
            availability_generation: 7,
            any_vehicle_exists: true,
            available_vehicle_count: 1,
            leased_vehicle_count: 0,
        };
        let cases = [
            (
                WorkType::Haul,
                TransportRequestKind::DeliverToBlueprint,
                ResourceType::Sand,
            ),
            (
                WorkType::HaulToMixer,
                TransportRequestKind::DeliverToMixerSolid,
                ResourceType::Sand,
            ),
        ];

        for (work_type, kind, resource_type) in cases {
            assert!(matches!(work_type, WorkType::Haul | WorkType::HaulToMixer));
            let request = TransportRequest {
                kind,
                anchor: entity(20),
                resource_type,
                issued_by: entity(21),
                priority: TransportPriority::Normal,
                stockpile_group: Vec::new(),
            };
            assert!(is_wheelbarrow_arbitration_applicable(&request));
            assert_eq!(
                wheelbarrow_arbitration_reason_from_evidence(
                    &request,
                    false,
                    Some(&header),
                    Some(WheelbarrowArbitrationOutcome::SourceReserved),
                    7,
                ),
                Some(CandidateRejectReason::TemporaryContention)
            );
        }
    }

    #[test]
    fn missing_or_stale_arbitration_evidence_remains_partial() {
        let request = TransportRequest {
            kind: TransportRequestKind::DeliverToBlueprint,
            anchor: entity(20),
            resource_type: ResourceType::Sand,
            issued_by: entity(21),
            priority: TransportPriority::Normal,
            stockpile_group: Vec::new(),
        };

        assert_eq!(
            wheelbarrow_arbitration_reason_from_evidence(&request, false, None, None, 0),
            Some(CandidateRejectReason::Unevaluated)
        );
    }

    #[test]
    fn missing_wheelbarrow_inputs_are_distinct_from_temporary_contention() {
        let request = TransportRequest {
            kind: TransportRequestKind::DeliverToBlueprint,
            anchor: entity(20),
            resource_type: ResourceType::Sand,
            issued_by: entity(21),
            priority: TransportPriority::Normal,
            stockpile_group: Vec::new(),
        };
        let available_header = WheelbarrowArbitrationHeader {
            generation: 1,
            availability_generation: 7,
            any_vehicle_exists: true,
            available_vehicle_count: 1,
            leased_vehicle_count: 0,
        };
        let cases = [
            (
                WheelbarrowArbitrationHeader {
                    any_vehicle_exists: false,
                    available_vehicle_count: 0,
                    ..available_header
                },
                None,
                CandidateRejectReason::MissingResourceOrSource,
            ),
            (
                WheelbarrowArbitrationHeader {
                    available_vehicle_count: 0,
                    ..available_header
                },
                None,
                CandidateRejectReason::TemporaryContention,
            ),
            (
                available_header,
                Some(WheelbarrowArbitrationOutcome::NoSourceItems),
                CandidateRejectReason::MissingResourceOrSource,
            ),
            (
                available_header,
                Some(WheelbarrowArbitrationOutcome::NoDestinationCapacity),
                CandidateRejectReason::MissingResourceOrSource,
            ),
        ];

        for (header, outcome, expected) in cases {
            assert_eq!(
                wheelbarrow_arbitration_reason_from_evidence(
                    &request,
                    false,
                    Some(&header),
                    outcome,
                    7,
                ),
                Some(expected)
            );
        }
    }

    #[test]
    fn released_arbitration_contention_clears_on_the_next_evidence() {
        let request = TransportRequest {
            kind: TransportRequestKind::DeliverToBlueprint,
            anchor: entity(20),
            resource_type: ResourceType::Sand,
            issued_by: entity(21),
            priority: TransportPriority::Normal,
            stockpile_group: Vec::new(),
        };
        let header = WheelbarrowArbitrationHeader {
            generation: 1,
            availability_generation: 7,
            any_vehicle_exists: true,
            available_vehicle_count: 1,
            leased_vehicle_count: 0,
        };

        for contention in [
            WheelbarrowArbitrationOutcome::SourceReserved,
            WheelbarrowArbitrationOutcome::CapacityReserved,
        ] {
            assert_eq!(
                wheelbarrow_arbitration_reason_from_evidence(
                    &request,
                    false,
                    Some(&header),
                    Some(contention),
                    7,
                ),
                Some(CandidateRejectReason::TemporaryContention)
            );
            assert_eq!(
                wheelbarrow_arbitration_reason_from_evidence(
                    &request,
                    true,
                    Some(&WheelbarrowArbitrationHeader {
                        generation: 2,
                        ..header
                    }),
                    Some(WheelbarrowArbitrationOutcome::LeaseGranted),
                    7,
                ),
                None
            );
        }
    }
}
