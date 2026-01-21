use super::FamiliarAiState;
use crate::entities::damned_soul::{DamnedSoul, Destination, IdleState, Path};
use crate::entities::familiar::UnderCommand;
use crate::systems::soul_ai::gathering::ParticipatingIn;
use crate::systems::soul_ai::task_execution::AssignedTask;
use bevy::prelude::*;

use crate::constants::TILE_SIZE;
use crate::systems::command::TaskArea;

/// 監視（Supervising）状態のロジック
pub fn supervising_logic(
    fam_entity: Entity,
    fam_pos: Vec2,
    active_members: &[Entity],
    task_area_opt: Option<&TaskArea>,
    time: &Res<Time>,
    ai_state: &mut FamiliarAiState,
    fam_dest: &mut Destination,
    fam_path: &mut Path,
    q_souls: &Query<
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
            Option<&ParticipatingIn>,
        ),
        Without<crate::entities::familiar::Familiar>,
    >,
    _has_available_task: bool,
) {
    if active_members.is_empty() {
        if let Some(area) = task_area_opt {
            let area_center = area.center();
            move_to_center(fam_entity, fam_pos, area_center, fam_dest, fam_path);
        }
        return;
    }

    // ステートから現在のターゲットとタイマーを取得
    let (mut current_target, mut timer) = match ai_state {
        FamiliarAiState::Supervising { target, timer } => (*target, *timer),
        _ => (None, 0.0),
    };

    // タイマー更新
    timer = (timer - time.delta_secs()).max(0.0);

    // 現在のターゲットが有効かチェック
    let mut target_valid = false;
    if let Some(target_ent) = current_target {
        if active_members.contains(&target_ent) {
            target_valid = true;
        }
    }

    // ターゲットが無効、またはタイマー終了時に新しいターゲットを選定
    if !target_valid || timer <= 0.0 {
        let mut best_worker = None;
        let mut max_worker_dist_sq = -1.0;
        let mut best_idle = None;
        let mut max_idle_dist_sq = -1.0;

        for &member_ent in active_members {
            if let Ok((_, transform, _, task, _, _, _, _, _, _)) = q_souls.get(member_ent) {
                let dist_sq = fam_pos.distance_squared(transform.translation.truncate());
                if !matches!(*task, AssignedTask::None) {
                    if dist_sq > max_worker_dist_sq {
                        max_worker_dist_sq = dist_sq;
                        best_worker = Some(member_ent);
                    }
                } else {
                    if dist_sq > max_idle_dist_sq {
                        max_idle_dist_sq = dist_sq;
                        best_idle = Some(member_ent);
                    }
                }
            }
        }

        let next_target = best_worker.or(best_idle);
        if let Some(new_target) = next_target {
            current_target = Some(new_target);
            timer = 2.0;
            debug!(
                "FAM_AI: {:?} New target selected: {:?}",
                fam_entity, new_target
            );
        }
    }

    // ステートを更新
    if let FamiliarAiState::Supervising { target, timer: t } = ai_state {
        *target = current_target;
        *t = timer;
    }

    // 移動制御
    let all_members_idle = active_members.iter().all(|&member_ent| {
        if let Ok((_, _, _, task, _, _, _, _, _, _)) = q_souls.get(member_ent) {
            matches!(*task, AssignedTask::None)
        } else {
            false
        }
    });

    if all_members_idle {
        if let Some(area) = task_area_opt {
            let area_center = area.center();
            // メンバー全員待機中なら、エリア中心に移動して終了（以降の追従ロジックをスキップ）
            move_to_center(fam_entity, fam_pos, area_center, fam_dest, fam_path);
            return;
        }
    }

    if let Some(target_ent) = current_target {
        if let Ok((_, transform, _, task, _, _, _, _, _, _)) = q_souls.get(target_ent) {
            let target_pos = transform.translation.truncate();
            let is_working = !matches!(*task, AssignedTask::None);

            // 監視のしきい値 (遠めから見守る設定)
            let follow_threshold = (TILE_SIZE * 5.0).powi(2);
            let stop_threshold = (TILE_SIZE * 3.0).powi(2);
            let dist_sq = fam_pos.distance_squared(target_pos);
            let is_path_finished = fam_path.current_index >= fam_path.waypoints.len();

            // 1. 誘導判定 (ターゲットが待機中、かつエリアから遠い場合)
            if !is_working {
                if let Some(area) = task_area_opt {
                    let area_center = area.center();
                    if fam_pos.distance_squared(area_center) > (TILE_SIZE * 1.5).powi(2)
                        || target_pos.distance_squared(area_center) > (TILE_SIZE * 5.0).powi(2)
                    {
                        if move_to_center(fam_entity, fam_pos, area_center, fam_dest, fam_path) {
                            return;
                        }
                    }
                }
            }

            // 2. 追従判定 (離れた時だけ近づく)
            let mut should_follow = dist_sq > follow_threshold;
            if !is_working {
                if let Some(area) = task_area_opt {
                    if !area.contains(target_pos)
                        && fam_pos.distance_squared(area.center()) < (TILE_SIZE * 3.0).powi(2)
                    {
                        should_follow = false;
                    }
                }
            }

            if should_follow {
                let dest_lag_sq = fam_dest.0.distance_squared(target_pos);
                if is_path_finished || dest_lag_sq > (TILE_SIZE * 1.0).powi(2) {
                    fam_dest.0 = target_pos;
                    fam_path.waypoints = vec![target_pos];
                    fam_path.current_index = 0;
                }
            } else if dist_sq < stop_threshold || !is_working {
                if !is_path_finished {
                    fam_path.waypoints.clear();
                    fam_path.current_index = 0;
                }
            }
        }
    }
}

/// 指定位置（エリア中心など）への移動を制御するヘルパー
/// 到着していれば false, 移動中なら true を返す
pub fn move_to_center(
    fam_entity: Entity,
    fam_pos: Vec2,
    center: Vec2,
    fam_dest: &mut Destination,
    fam_path: &mut Path,
) -> bool {
    let dist_sq = fam_pos.distance_squared(center);
    let is_path_finished = fam_path.current_index >= fam_path.waypoints.len();
    let threshold_sq = (TILE_SIZE * 1.5).powi(2);

    if dist_sq > threshold_sq {
        let is_moving_to_center = fam_dest.0.distance_squared(center) < (TILE_SIZE * 0.5).powi(2);
        if is_path_finished || !is_moving_to_center {
            debug!(
                "FAM_AI: {:?} setting return path to center {:?}",
                fam_entity, center
            );
            fam_dest.0 = center;
            fam_path.waypoints = vec![center];
            fam_path.current_index = 0;
        }
        true
    } else {
        if !is_path_finished {
            debug!("FAM_AI: {:?} reached center, clearing path", fam_entity);
            fam_path.waypoints.clear();
            fam_path.current_index = 0;
        }
        false
    }
}
