//! タスク検索モジュール

mod filter;
mod score;

use bevy::prelude::*;
use hw_core::area::TaskArea;
use hw_core::relationships::ManagedTasks;
use hw_jobs::{TargetBlueprint, WorkType};
use hw_spatial::{DesignationSpatialGrid, TransportRequestSpatialGrid};
use hw_world::{WorldMap, Yard};
use std::collections::HashSet;

use crate::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries;
use filter::{candidate_snapshot, collect_candidate_entities};
use score::score_candidate;

#[derive(Clone, Copy, Debug)]
pub struct DelegationCandidate {
    pub entity: Entity,
    pub target_grid: (i32, i32),
    pub target_walkable: bool,
    pub skip_reachability_check: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct ScoredDelegationCandidate {
    pub candidate: DelegationCandidate,
    pub priority: i32,
    pub pos: Vec2,
    pub dist_sq: f32,
}

/// `collect_scored_candidates` に渡す Familiar 固有の検索コンテキスト。
pub struct FamiliarSearchContext<'a> {
    pub fam_entity: Entity,
    pub fam_pos: Vec2,
    pub task_area_opt: Option<&'a TaskArea>,
}

/// Familiar単位で委譲候補を収集し、スコア情報付きで返す
pub fn collect_scored_candidates(
    ctx: FamiliarSearchContext<'_>,
    queries: &FamiliarTaskAssignmentQueries,
    designation_grid: &DesignationSpatialGrid,
    transport_request_grid: &TransportRequestSpatialGrid,
    managed_tasks: &ManagedTasks,
    q_target_blueprints: &Query<&TargetBlueprint>,
    world_map: &WorldMap,
) -> Vec<ScoredDelegationCandidate> {
    let all_yards: Vec<Yard> = queries.yards.iter().cloned().collect();

    let mut candidates = collect_candidate_entities(
        ctx.task_area_opt,
        &all_yards,
        managed_tasks,
        designation_grid,
        transport_request_grid,
    );

    let mut seen: HashSet<Entity> = candidates.iter().copied().collect();
    for (entity, _, designation, managed_by_opt, _, _, _, _) in
        queries.designation.designations.iter()
    {
        let is_build = designation.work_type == WorkType::Build;
        let is_remote_yard_collect = matches!(
            designation.work_type,
            WorkType::CollectBone
        ) && managed_by_opt
            .is_some_and(|managed_by| queries.yards.get(managed_by.0).is_ok());

        if (is_build || is_remote_yard_collect) && seen.insert(entity) {
            candidates.push(entity);
        }
    }

    let valid_candidates: Vec<ScoredDelegationCandidate> = candidates
        .into_iter()
        .filter_map(|entity| {
            let snapshot = candidate_snapshot(
                ctx.fam_entity,
                entity,
                ctx.task_area_opt,
                &all_yards,
                managed_tasks,
                world_map,
                queries,
            )?;

            let work_type = snapshot.work_type;
            if work_type == WorkType::HaulWaterToMixer {
                debug!("TASK_FINDER: HaulWaterToMixer {:?} passed filter", entity);
            }

            let priority = score_candidate(
                entity,
                work_type,
                snapshot.base_priority,
                snapshot.in_stockpile_none,
                queries,
                q_target_blueprints,
            )?;

            let dist_sq = snapshot.pos.distance_squared(ctx.fam_pos);

            if work_type == WorkType::HaulWaterToMixer {
                debug!(
                    "TASK_FINDER: HaulWaterToMixer {:?} scored priority={} dist_sq={}",
                    entity, priority, dist_sq
                );
            }

            Some(ScoredDelegationCandidate {
                candidate: DelegationCandidate {
                    entity,
                    target_grid: snapshot.target_grid,
                    target_walkable: snapshot.target_walkable,
                    skip_reachability_check: snapshot.skip_reachability_check,
                },
                priority,
                pos: snapshot.pos,
                dist_sq,
            })
        })
        .collect();

    if valid_candidates.is_empty() {
        debug!("TASK_FINDER: {:?} has no candidates", ctx.fam_entity);
    }

    valid_candidates
}
