use super::FamiliarAiState;
use crate::constants::TILE_SIZE;
use crate::entities::damned_soul::{
    DamnedSoul, Destination, IdleBehavior, IdleState, Path, StressBreakdown,
};
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
    _max_workers: usize,
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
            &mut crate::systems::logistics::Inventory,
            Option<&UnderCommand>,
        ),
        Without<crate::entities::familiar::Familiar>,
    >,
    q_breakdown: &Query<&StressBreakdown>,
    commands: &mut Commands,
) -> bool {
    // 変更があった場合は true を返す
    if let Ok((_soul_entity, target_transform, soul, task, _, _, idle, _, uc)) =
        q_souls.get(target_soul)
    {
        // Gathering状態（回復中）なら疲労チェックをスキップ（helpers.rs と条件を統一）
        let is_gathering = idle.behavior == IdleBehavior::Gathering;
        let fatigue_ok = is_gathering || soul.fatigue < fatigue_threshold;
        let stress_ok = q_breakdown.get(target_soul).is_err();

        // 依然としてリクルート可能かチェック
        if uc.is_none() || matches!(uc, Some(u) if u.0 == fam_entity) {
            if !fatigue_ok || !stress_ok || !matches!(*task, AssignedTask::None) {
                // 条件を満たさなくなった
                *ai_state = FamiliarAiState::SearchingTask;
                return true;
            }

            let target_pos = target_transform.translation.truncate();
            if fam_pos.distance_squared(target_pos) < (TILE_SIZE * 1.5).powi(2) {
                // リクルート成功
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
                if squad.len() >= _max_workers {
                    *ai_state = FamiliarAiState::Supervising {
                        target: Some(target_soul),
                        timer: 2.0,
                    };
                } else {
                    *ai_state = FamiliarAiState::SearchingTask;
                }

                return true;
            } else {
                // まだ距離があるなら接近を継続 (ガードを 0.5 タイルに緩和して反応性を向上)
                let is_path_finished = fam_path.current_index >= fam_path.waypoints.len();
                let dest_lag_sq = fam_dest.0.distance_squared(target_pos);

                if is_path_finished || dest_lag_sq > (TILE_SIZE * 0.5).powi(2) {
                    fam_dest.0 = target_pos;
                    fam_path.waypoints = vec![target_pos];
                    fam_path.current_index = 0;
                }
                return false;
            }
        } else {
            // 他の使い魔に取られた
            *ai_state = FamiliarAiState::SearchingTask;
            return true;
        }
    } else {
        // ターゲット消失
        *ai_state = FamiliarAiState::SearchingTask;
        return true;
    }
}
