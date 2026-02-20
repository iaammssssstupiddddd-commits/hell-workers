//! タスク検索モジュール
//!
//! 未割り当てのタスクを検索するロジックを提供します。

mod filter;
mod score;

use crate::relationships::ManagedTasks;
use crate::systems::command::TaskArea;
use crate::systems::jobs::{TargetBlueprint, WorkType};
use crate::systems::spatial::{DesignationSpatialGrid, TransportRequestSpatialGrid};
use crate::world::map::WorldMap;
use bevy::prelude::*;

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

/// Familiar単位で委譲候補を収集し、スコア情報付きで返す
#[allow(clippy::too_many_arguments)]
pub fn collect_scored_candidates(
    fam_entity: Entity,
    fam_pos: Vec2,
    task_area_opt: Option<&TaskArea>,
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    designation_grid: &DesignationSpatialGrid,
    transport_request_grid: &TransportRequestSpatialGrid,
    managed_tasks: &ManagedTasks,
    q_target_blueprints: &Query<&TargetBlueprint>,
    world_map: &WorldMap,
) -> Vec<ScoredDelegationCandidate> {
    let candidates = collect_candidate_entities(
        task_area_opt,
        managed_tasks,
        designation_grid,
        transport_request_grid,
    );

    let valid_candidates: Vec<ScoredDelegationCandidate> = candidates
        .into_iter()
        .filter_map(|entity| {
            let snapshot = candidate_snapshot(
                fam_entity,
                entity,
                task_area_opt,
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

            let dist_sq = snapshot.pos.distance_squared(fam_pos);

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
        debug!("TASK_FINDER: {:?} has no candidates", fam_entity);
    }

    valid_candidates
}
