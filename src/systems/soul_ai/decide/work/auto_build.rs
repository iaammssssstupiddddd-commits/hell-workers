use bevy::prelude::*;

use crate::entities::damned_soul::StressBreakdown;
use crate::entities::familiar::ActiveCommand;
use crate::events::TaskAssignmentRequest;
use crate::relationships::{Commanding, ManagedBy, TaskWorkers};
use crate::systems::command::TaskArea;
use crate::systems::jobs::{Blueprint, Designation, Priority, TaskSlots, WorkType};
use crate::systems::logistics::InStockpile;
use crate::systems::soul_ai::execute::task_execution::types::{AssignedTask, BuildData, BuildPhase};
use crate::systems::soul_ai::helpers::query_types::AutoBuildSoulQuery;
use crate::systems::soul_ai::helpers::work as helpers;
use crate::systems::spatial::BlueprintSpatialGrid;

/// 資材が揃った建築タスクの自動割り当てシステム
pub fn blueprint_auto_build_system(
    mut assignment_writer: MessageWriter<TaskAssignmentRequest>,
    blueprint_grid: Res<BlueprintSpatialGrid>,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea, Option<&Commanding>)>,
    q_blueprints: Query<(Entity, &Transform, &Blueprint, Option<&TaskWorkers>)>,
    q_designations: Query<(
        Entity,
        &Transform,
        &Designation,
        Option<&ManagedBy>,
        Option<&TaskSlots>,
        Option<&TaskWorkers>,
        Option<&InStockpile>,
        Option<&Priority>,
    )>,
    mut q_souls: AutoBuildSoulQuery,
    q_breakdown: Query<&StressBreakdown>,
) {
    for (fam_entity, _active_command, task_area, commanding_opt) in q_familiars.iter() {
        let Some(commanding) = commanding_opt else {
            continue;
        };
        let mut already_requested_workers = std::collections::HashSet::new();

        // 最適化: タスクエリア内のブループリントのみを取得
        let blueprints_in_area = blueprint_grid.get_in_area(task_area.min, task_area.max);

        for bp_entity in blueprints_in_area {
            // クエリで詳細データを取得
            let Ok((_, bp_transform, blueprint, workers_opt)) = q_blueprints.get(bp_entity) else {
                continue;
            };
            let bp_pos = bp_transform.translation.truncate();

            // 既に作業員が割り当てられている場合はスキップ（建築中）
            if workers_opt.map(|w| w.len()).unwrap_or(0) > 0 {
                continue;
            }

            // 資材が揃っていて、まだManagedByが付与されていない場合のみ処理
            if !blueprint.materials_complete() {
                continue;
            }

            // Designationが存在し、ManagedByが付与されていないか確認
            if let Ok((_, _, designation, managed_by_opt, _, _, _, _)) =
                q_designations.get(bp_entity)
            {
                if designation.work_type != WorkType::Build {
                    continue;
                }

                // 既に割り当てられている場合はスキップ
                if managed_by_opt.is_some() {
                    continue;
                }

                // 使い魔の部下から待機中の魂を探す
                let fatigue_threshold = 0.8; // デフォルトの疲労閾値

                // 近くの待機中の魂を探す
                let mut best_worker = None;
                let mut min_dist_sq = f32::MAX;

                for &soul_entity in commanding.iter() {
                    if already_requested_workers.contains(&soul_entity) {
                        continue;
                    }
                    let Ok((_, soul_transform, soul, task, _, _, idle, uc_opt)) =
                        q_souls.get_mut(soul_entity)
                    else {
                        continue;
                    };

                    if uc_opt.map(|uc| uc.0) != Some(fam_entity) {
                        continue;
                    }

                    // 待機中・健康チェックをヘルパーに委譲
                        if !helpers::is_soul_available_for_work(
                            soul,
                            &task,
                            idle,
                            q_breakdown.get(soul_entity).is_ok(),
                            fatigue_threshold,
                    ) {
                        continue;
                    }

                    // 最も近い魂を選択
                    let dist_sq = soul_transform
                        .translation
                        .truncate()
                        .distance_squared(bp_pos);
                    if dist_sq < min_dist_sq {
                        min_dist_sq = dist_sq;
                        best_worker = Some(soul_entity);
                    }
                }

                // 見つかった魂に建築タスクを割り当て
                if let Some(worker_entity) = best_worker {
                    if let Ok((_, _, soul, assigned_task, _, _, idle, _)) =
                        q_souls.get_mut(worker_entity)
                    {
                        if !helpers::is_soul_available_for_work(
                            soul,
                            &assigned_task,
                            idle,
                            false, // ここでは既に前のチェックを通っているが、念のため
                            fatigue_threshold,
                        ) {
                            continue;
                        }

                        assignment_writer.write(TaskAssignmentRequest {
                            familiar_entity: fam_entity,
                            worker_entity,
                            task_entity: bp_entity,
                            work_type: WorkType::Build,
                            task_pos: bp_pos,
                            assigned_task: AssignedTask::Build(BuildData {
                                blueprint: bp_entity,
                                phase: BuildPhase::GoingToBlueprint,
                            }),
                            reservation_ops: vec![],
                            already_commanded: true,
                        });
                        already_requested_workers.insert(worker_entity);

                        info!(
                            "AUTO_BUILD: Assigned build task {:?} to worker {:?}",
                            bp_entity, worker_entity
                        );
                    }
                }
            }
        }
    }
}
