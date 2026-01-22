use bevy::prelude::*;

use crate::entities::damned_soul::{DamnedSoul, Destination, IdleState, Path, StressBreakdown};
use crate::entities::familiar::{ActiveCommand, Familiar, UnderCommand};
use crate::relationships::{TaskWorkers, WorkingOn};
use crate::systems::command::TaskArea;
use crate::systems::jobs::{Blueprint, Designation, IssuedBy, TaskSlots, WorkType};
use crate::systems::soul_ai::task_execution::AssignedTask;
use crate::systems::soul_ai::task_execution::types::BuildPhase;
use crate::systems::soul_ai::work::helpers;

/// 資材が揃った建築タスクの自動割り当てシステム
pub fn blueprint_auto_build_system(
    mut commands: Commands,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea)>,
    q_blueprints: Query<(Entity, &Transform, &Blueprint, Option<&TaskWorkers>)>,
    q_designations: Query<(
        Entity,
        &Transform,
        &Designation,
        Option<&IssuedBy>,
        Option<&TaskSlots>,
        Option<&TaskWorkers>,
    )>,
    mut q_souls: Query<
        (
            Entity,
            &Transform,
            &DamnedSoul,
            &mut AssignedTask,
            &mut Destination,
            &mut Path,
            &IdleState,
            Option<&crate::relationships::Holding>,
            Option<&UnderCommand>,
        ),
        Without<Familiar>,
    >,
    q_breakdown: Query<&StressBreakdown>,
) {
    for (fam_entity, _active_command, task_area) in q_familiars.iter() {
        // エリア内の Blueprint を探す
        for (bp_entity, bp_transform, blueprint, workers_opt) in q_blueprints.iter() {
            let bp_pos = bp_transform.translation.truncate();
            if !task_area.contains(bp_pos) {
                continue;
            }

            // 既に作業員が割り当てられている場合はスキップ（建築中）
            if workers_opt.map(|w| w.len()).unwrap_or(0) > 0 {
                continue;
            }

            // 資材が揃っていて、まだIssuedByが付与されていない場合のみ処理
            if !blueprint.materials_complete() {
                continue;
            }

            // Designationが存在し、IssuedByが付与されていないか確認
            if let Ok((_, _, designation, issued_by_opt, _, _)) = q_designations.get(bp_entity) {
                if designation.work_type != WorkType::Build {
                    continue;
                }

                // 既に割り当てられている場合はスキップ
                if issued_by_opt.is_some() {
                    continue;
                }

                // 使い魔の部下から待機中の魂を探す
                let fatigue_threshold = 0.8; // デフォルトの疲労閾値

                // 近くの待機中の魂を探す
                let mut best_worker = None;
                let mut min_dist_sq = f32::MAX;

                for (soul_entity, soul_transform, soul, task, _, _, idle, _, uc_opt) in
                    q_souls.iter()
                {
                    // この使い魔の部下か確認
                    if let Some(uc) = uc_opt {
                        if uc.0 != fam_entity {
                            continue;
                        }
                    } else {
                        continue;
                    }

                    // 待機中・健康チェックをヘルパーに委譲
                    if !helpers::is_soul_available_for_work(
                        soul,
                        task,
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
                    if let Ok((_, _, soul, mut assigned_task, mut dest, mut path, idle, _, _)) =
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

                        // 建築タスクを割り当て
                        *assigned_task = AssignedTask::Build {
                            blueprint: bp_entity,
                            phase: BuildPhase::GoingToBlueprint,
                        };
                        dest.0 = bp_pos;
                        path.waypoints = vec![bp_pos];
                        path.current_index = 0;

                        commands
                            .entity(worker_entity)
                            .insert((UnderCommand(fam_entity), WorkingOn(bp_entity)));
                        commands.entity(bp_entity).insert(IssuedBy(fam_entity));

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
