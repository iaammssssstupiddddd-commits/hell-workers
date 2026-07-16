use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use std::time::Duration;

use hw_core::area::TaskArea;
use hw_core::familiar::{ActiveCommand, FamiliarCommand};
use hw_core::jobs::WorkType;
use hw_core::relationships::{Commanding, ManagedBy, TaskWorkers};
use hw_core::soul::StressBreakdown;
use hw_jobs::events::TaskAssignmentRequest;
use hw_jobs::{AssignedTask, Blueprint, BuildData, BuildPhase, Designation, Priority, TaskSlots};
use hw_spatial::BlueprintSpatialGrid;

use crate::soul_ai::helpers::query_types::AutoBuildSoulQuery;
use crate::soul_ai::helpers::work as helpers;

type DesignationsQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static Designation,
        Option<&'static ManagedBy>,
        Option<&'static TaskSlots>,
        Option<&'static TaskWorkers>,
        Option<&'static Priority>,
    ),
>;

#[derive(SystemParam)]
pub(crate) struct BlueprintAutoBuildParams<'w, 's> {
    assignment_writer: MessageWriter<'w, TaskAssignmentRequest>,
    blueprint_grid: Res<'w, BlueprintSpatialGrid>,
    q_familiars: Query<
        'w,
        's,
        (
            Entity,
            &'static ActiveCommand,
            &'static TaskArea,
            Option<&'static Commanding>,
        ),
    >,
    q_blueprints: Query<
        'w,
        's,
        (
            Entity,
            &'static Transform,
            &'static Blueprint,
            Option<&'static TaskWorkers>,
        ),
    >,
    q_designations: DesignationsQuery<'w, 's>,
    q_souls: AutoBuildSoulQuery<'w, 's>,
    q_breakdown: Query<'w, 's, &'static StressBreakdown>,
}

/// Existing build producer cadence while ownership remains intentionally
/// separate from Familiar task delegation.
///
/// The producer has different TaskArea/Idle/ManagedBy semantics from the
/// general delegator, so M3 does not merge the two paths without a product
/// contract. It does, however, avoid scanning them at render-frame cadence.
#[derive(Resource)]
pub(crate) struct BlueprintAutoBuildTimer {
    timer: Timer,
    first_run_done: bool,
}

impl Default for BlueprintAutoBuildTimer {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(0.5, TimerMode::Repeating),
            first_run_done: false,
        }
    }
}

impl BlueprintAutoBuildTimer {
    fn advance(&mut self, delta: Duration) -> bool {
        let first_cycle = !self.first_run_done;
        let elapsed = self.timer.tick(delta).just_finished();
        self.first_run_done = true;
        first_cycle || elapsed
    }
}

/// 資材が揃った建築タスクの自動割り当てシステム
pub(crate) fn blueprint_auto_build_system(
    time: Res<Time>,
    mut cadence: ResMut<BlueprintAutoBuildTimer>,
    mut params: BlueprintAutoBuildParams,
) {
    if !cadence.advance(time.delta()) {
        return;
    }

    for (fam_entity, active_command, task_area, commanding_opt) in params.q_familiars.iter() {
        if matches!(active_command.command, FamiliarCommand::Idle) {
            continue;
        }

        let Some(commanding) = commanding_opt else {
            continue;
        };
        let mut already_requested_workers = std::collections::HashSet::new();

        // 最適化: タスクエリア内のブループリントのみを取得
        let blueprints_in_area = params
            .blueprint_grid
            .get_in_area(task_area.min(), task_area.max());

        for bp_entity in blueprints_in_area {
            // クエリで詳細データを取得
            let Ok((_, bp_transform, blueprint, workers_opt)) = params.q_blueprints.get(bp_entity)
            else {
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
            if let Ok((_, _, designation, managed_by_opt, _, _, _)) =
                params.q_designations.get(bp_entity)
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
                        params.q_souls.get_mut(soul_entity)
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
                        params.q_breakdown.get(soul_entity).is_ok(),
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
                if let Some(worker_entity) = best_worker
                    && let Ok((_, _, soul, assigned_task, _, _, idle, _)) =
                        params.q_souls.get_mut(worker_entity)
                {
                    if !helpers::is_soul_available_for_work(
                        soul,
                        &assigned_task,
                        idle,
                        false,
                        fatigue_threshold,
                    ) {
                        continue;
                    }

                    params.assignment_writer.write(TaskAssignmentRequest {
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

#[cfg(test)]
mod tests {
    use super::BlueprintAutoBuildTimer;
    use std::time::Duration;

    #[test]
    fn producer_runs_immediately_then_at_half_second_cadence() {
        let mut timer = BlueprintAutoBuildTimer::default();

        assert!(timer.advance(Duration::ZERO));
        assert!(!timer.advance(Duration::from_millis(499)));
        assert!(timer.advance(Duration::from_millis(1)));
    }
}
