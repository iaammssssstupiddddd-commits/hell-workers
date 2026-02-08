//! タスク検索モジュール
//!
//! 未割り当てのタスクを検索するロジックを提供します。

mod filter;
mod score;

use crate::relationships::ManagedTasks;
use crate::systems::command::TaskArea;
use crate::systems::jobs::{TargetBlueprint, WorkType};
use crate::systems::spatial::DesignationSpatialGrid;
use crate::world::map::WorldMap;
use crate::world::pathfinding::PathfindingContext;
use bevy::prelude::*;

use filter::{candidate_snapshot, collect_candidate_entities};
use score::score_candidate;

/// 指定ワーカーの位置から到達可能な未割り当てタスクを探す
#[allow(clippy::too_many_arguments)]
pub fn find_unassigned_task_in_area(
    fam_entity: Entity,
    fam_pos: Vec2,
    worker_pos: Vec2,
    task_area_opt: Option<&TaskArea>,
    queries: &crate::systems::soul_ai::task_execution::context::TaskAssignmentQueries,
    designation_grid: &DesignationSpatialGrid,
    managed_tasks: &ManagedTasks,
    q_target_blueprints: &Query<&TargetBlueprint>,
    world_map: &WorldMap,
    pf_context: &mut PathfindingContext,
) -> Vec<Entity> {
    let candidates = collect_candidate_entities(task_area_opt, managed_tasks, designation_grid);

    let mut valid_candidates: Vec<(Entity, i32, f32)> = candidates
        .into_iter()
        .filter_map(|entity| {
            let (pos, work_type, base_priority, in_stockpile_none) = candidate_snapshot(
                fam_entity,
                entity,
                task_area_opt,
                managed_tasks,
                worker_pos,
                world_map,
                pf_context,
                queries,
            )?;

            if work_type == WorkType::HaulWaterToMixer {
                debug!("TASK_FINDER: HaulWaterToMixer {:?} passed filter", entity);
            }

            let priority = score_candidate(
                entity,
                work_type,
                base_priority,
                in_stockpile_none,
                queries,
                q_target_blueprints,
            )?;

            let dist_sq = pos.distance_squared(fam_pos);

            if work_type == WorkType::HaulWaterToMixer {
                debug!(
                    "TASK_FINDER: HaulWaterToMixer {:?} scored priority={} dist_sq={}",
                    entity, priority, dist_sq
                );
            }

            Some((entity, priority, dist_sq))
        })
        .collect();

    valid_candidates.sort_by(|(_, p1, d1), (_, p2, d2)| match p2.cmp(p1) {
        std::cmp::Ordering::Equal => d1.partial_cmp(d2).unwrap_or(std::cmp::Ordering::Equal),
        other => other,
    });

    if valid_candidates.is_empty() {
        debug!("TASK_FINDER: {:?} has no candidates", fam_entity);
    }

    valid_candidates
        .into_iter()
        .map(|(entity, _, _)| entity)
        .collect()
}
