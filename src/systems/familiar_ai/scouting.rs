use super::FamiliarAiState;
use crate::constants::TILE_SIZE;
use crate::entities::damned_soul::{DamnedSoul, Destination, IdleState, Path, StressBreakdown};
use crate::entities::familiar::UnderCommand;
use crate::events::OnSoulRecruited;
use crate::systems::work::AssignedTask;
use bevy::prelude::*;

/// スカウト（Scouting）状態のロジック
/// ターゲットに接近し、近づいたらリクルートする
pub fn scouting_logic(
    fam_entity: Entity,
    fam_pos: Vec2,
    target_soul: Entity,
    fatigue_threshold: f32,
    max_workers: usize,
    squad: &mut Vec<Entity>,
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
        ),
        Without<crate::entities::familiar::Familiar>,
    >,
    q_breakdown: &Query<&StressBreakdown>,
    commands: &mut Commands,
) -> bool {
    // 早期退出: 分隊が既に満員なら監視モードへ
    if squad.len() >= max_workers {
        info!(
            "FAM_AI: {:?} scouting cancelled (squad full: {}/{}), switching to Supervising",
            fam_entity,
            squad.len(),
            max_workers
        );
        *ai_state = FamiliarAiState::Supervising {
            target: None,
            timer: 0.0,
        };
        return true;
    }

    // 変更があった場合は true を返す
    if let Ok((_soul_entity, target_transform, soul, task, _, _, _, _, uc)) =
        q_souls.get(target_soul)
    {
        // リクルート閾値 = リリース閾値 - 0.2（余裕を持ってリクルート）
        let recruit_threshold = fatigue_threshold - 0.2;
        let fatigue_ok = soul.fatigue < recruit_threshold;
        let stress_ok = q_breakdown.get(target_soul).is_err();

        // 依然としてリクルート可能かチェック
        if uc.is_none() || matches!(uc, Some(u) if u.0 == fam_entity) {
            if !fatigue_ok || !stress_ok || !matches!(*task, AssignedTask::None) {
                // 条件を満たさなくなった
                info!(
                    "FAM_AI: {:?} scouting cancelled for {:?} (FatigueOK: {}, StressOK: {}, Task: {:?})",
                    fam_entity, target_soul, fatigue_ok, stress_ok, *task
                );
                *ai_state = FamiliarAiState::SearchingTask;
                return true;
            }

            let target_pos = target_transform.translation.truncate();
            let dist_sq = fam_pos.distance_squared(target_pos);

            // リクルート半径を少し広げて確実に成功させる (1.5 -> 2.5)
            if dist_sq < (TILE_SIZE * 2.5).powi(2) {
                // リクルート成功
                info!(
                    "FAM_AI: {:?} reached target {:?} (dist: {:.2}), recruiting...",
                    fam_entity,
                    target_soul,
                    dist_sq.sqrt()
                );
                if uc.is_none() {
                    commands
                        .entity(target_soul)
                        .insert(UnderCommand(fam_entity));
                    commands.trigger(OnSoulRecruited {
                        entity: target_soul,
                        familiar_entity: fam_entity,
                    });

                    // 分隊リストを即座に更新
                    squad.push(target_soul);
                }

                // 次のステートを決定 (元々のロジック: 満員でないなら探索に戻る)
                if squad.len() >= max_workers {
                    info!(
                        "FAM_AI: {:?} squad full after recruit, switching to Supervising",
                        fam_entity
                    );
                    *ai_state = FamiliarAiState::Supervising {
                        target: Some(target_soul),
                        timer: 2.0,
                    };
                } else {
                    info!(
                        "FAM_AI: {:?} squad has room ({}/{}), returning to Searching",
                        fam_entity,
                        squad.len(),
                        max_workers
                    );
                    *ai_state = FamiliarAiState::SearchingTask;
                }

                return true;
            } else {
                // まだ距離があるなら接近を継続 (ガードを 0.5 タイルに緩和して反応性を向上)
                let is_path_finished = fam_path.current_index >= fam_path.waypoints.len();
                let dest_lag_sq = fam_dest.0.distance_squared(target_pos);
                let dist = dist_sq.sqrt();

                if is_path_finished || dest_lag_sq > (TILE_SIZE * 0.5).powi(2) {
                    debug!(
                        "FAM_AI: {:?} approaching {:?} (dist: {:.2}, path_fin: {})",
                        fam_entity, target_soul, dist, is_path_finished
                    );
                    fam_dest.0 = target_pos;
                    fam_path.waypoints = vec![target_pos];
                    fam_path.current_index = 0;
                }
                return false;
            }
        } else {
            // 他の使い魔に取られた
            info!(
                "FAM_AI: {:?} scouting target {:?} taken by another familiar",
                fam_entity, target_soul
            );
            *ai_state = FamiliarAiState::SearchingTask;
            return true;
        }
    } else {
        // ターゲット消失
        info!(
            "FAM_AI: {:?} scouting target {:?} disappeared from world",
            fam_entity, target_soul
        );
        *ai_state = FamiliarAiState::SearchingTask;
        return true;
    }
}
