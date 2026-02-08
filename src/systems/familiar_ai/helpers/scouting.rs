use crate::constants::TILE_SIZE;
use crate::entities::damned_soul::{DamnedSoul, Destination, IdleState, Path, StressBreakdown};
use crate::events::SquadManagementRequest;
use crate::relationships::CommandedBy;
// use crate::events::OnSoulRecruited;
use crate::systems::familiar_ai::FamiliarAiState;
use crate::systems::familiar_ai::FamiliarSoulQuery;
use crate::systems::soul_ai::gathering::ParticipatingIn;
use crate::systems::soul_ai::task_execution::AssignedTask;
use bevy::prelude::*;

/// スカウト状態の判定/適用に必要なコンテキスト
pub struct FamiliarScoutingContext<'a, 'w, 's> {
    pub fam_entity: Entity,
    pub fam_pos: Vec2,
    pub target_soul: Entity,
    pub fatigue_threshold: f32,
    pub max_workers: usize,
    pub squad: &'a mut Vec<Entity>,
    pub ai_state: &'a mut FamiliarAiState,
    pub fam_dest: &'a mut Destination,
    pub fam_path: &'a mut Path,
    pub q_souls: &'a mut FamiliarSoulQuery<'w, 's>,
    pub q_breakdown: &'a Query<'w, 's, &'static StressBreakdown>,
    pub request_writer: &'a mut MessageWriter<'w, SquadManagementRequest>,
}

/// スカウト（Scouting）状態のロジック
/// ターゲットに接近し、近づいたらリクルートする
pub fn scouting_logic(ctx: &mut FamiliarScoutingContext<'_, '_, '_>) -> bool {
    // 早期退出: 分隊が既に満員なら監視モードへ
    if ctx.squad.len() >= ctx.max_workers {
        info!(
            "FAM_AI: {:?} scouting cancelled (squad full: {}/{}), switching to Supervising",
            ctx.fam_entity,
            ctx.squad.len(),
            ctx.max_workers
        );
        *ctx.ai_state = FamiliarAiState::Supervising {
            target: None,
            timer: 0.0,
        };
        return true;
    }

    // 変更があった場合は true を返す
    if let Ok((
        _soul_entity,
        target_transform,
        soul,
        soul_task,
        dest,
        path,
        idle,
        mut _inv,
        uc,
        participating,
    )) = ctx.q_souls.get_mut(ctx.target_soul)
    {
        let (
            _soul_entity,
            target_transform,
            soul,
            soul_task,
            _dest,
            _path,
            _idle,
            _inv,
            uc,
            _participating,
        ): (
            Entity,
            &Transform,
            &DamnedSoul,
            &AssignedTask,
            &Destination,
            &Path,
            &IdleState,
            Option<&mut crate::systems::logistics::Inventory>,
            Option<&CommandedBy>,
            Option<&ParticipatingIn>,
        ) = (
            _soul_entity,
            target_transform,
            soul,
            &soul_task,
            &*dest,
            &*path,
            idle,
            _inv.as_deref_mut(),
            uc,
            participating,
        );
        // リクルート閾値 = リリース閾値 - 0.2（余裕を持ってリクルート）
        let recruit_threshold = ctx.fatigue_threshold - 0.2;
        let fatigue_ok = soul.fatigue < recruit_threshold;
        let stress_ok = ctx.q_breakdown.get(ctx.target_soul).is_err();

        // 依然としてリクルート可能かチェック
        if uc.is_none() || matches!(uc, Some(u) if u.0 == ctx.fam_entity) {
            if !fatigue_ok || !stress_ok || !matches!(soul_task, AssignedTask::None) {
                // 条件を満たさなくなった
                info!(
                    "FAM_AI: {:?} scouting cancelled for {:?} (FatigueOK: {}, StressOK: {}, Task: {:?})",
                    ctx.fam_entity, ctx.target_soul, fatigue_ok, stress_ok, soul_task
                );
                *ctx.ai_state = FamiliarAiState::SearchingTask;
                return true;
            }

            let target_pos = target_transform.translation.truncate();
            let dist_sq = ctx.fam_pos.distance_squared(target_pos);

            // リクルート半径を少し広げて確実に成功させる (1.5 -> 2.5)
            if dist_sq < (TILE_SIZE * 2.5).powi(2) {
                // リクルート成功
                info!(
                    "FAM_AI: {:?} reached target {:?} (dist: {:.2}), recruiting...",
                    ctx.fam_entity,
                    ctx.target_soul,
                    dist_sq.sqrt()
                );
                if uc.is_none() {
                    ctx.request_writer
                        .write(crate::events::SquadManagementRequest {
                            familiar_entity: ctx.fam_entity,
                            operation: crate::events::SquadManagementOperation::AddMember {
                                soul_entity: ctx.target_soul,
                            },
                        });

                    // 分隊リストを即座に更新 (Decideフェーズ内の後続処理のため)
                    ctx.squad.push(ctx.target_soul);
                }

                // 次のステートを決定 (元々のロジック: 満員でないなら探索に戻る)
                if ctx.squad.len() >= ctx.max_workers {
                    info!(
                        "FAM_AI: {:?} squad full after recruit, switching to Supervising",
                        ctx.fam_entity
                    );
                    *ctx.ai_state = FamiliarAiState::Supervising {
                        target: Some(ctx.target_soul),
                        timer: 2.0,
                    };
                } else {
                    info!(
                        "FAM_AI: {:?} squad has room ({}/{}), returning to Searching",
                        ctx.fam_entity,
                        ctx.squad.len(),
                        ctx.max_workers
                    );
                    *ctx.ai_state = FamiliarAiState::SearchingTask;
                }

                return true;
            } else {
                // まだ距離があるなら接近を継続 (ガードを 0.5 タイルに緩和して反応性を向上)
                let is_path_finished = ctx.fam_path.current_index >= ctx.fam_path.waypoints.len();
                let dest_lag_sq = ctx.fam_dest.0.distance_squared(target_pos);
                let dist = dist_sq.sqrt();

                if is_path_finished || dest_lag_sq > (TILE_SIZE * 0.5).powi(2) {
                    debug!(
                        "FAM_AI: {:?} approaching {:?} (dist: {:.2}, path_fin: {})",
                        ctx.fam_entity, ctx.target_soul, dist, is_path_finished
                    );
                    ctx.fam_dest.0 = target_pos;
                    ctx.fam_path.waypoints = vec![target_pos];
                    ctx.fam_path.current_index = 0;
                }
                return false;
            }
        } else {
            // 他の使い魔に取られた
            info!(
                "FAM_AI: {:?} scouting target {:?} taken by another familiar",
                ctx.fam_entity, ctx.target_soul
            );
            *ctx.ai_state = FamiliarAiState::SearchingTask;
            return true;
        }
    } else {
        // ターゲット消失
        info!(
            "FAM_AI: {:?} scouting target {:?} disappeared from world",
            ctx.fam_entity, ctx.target_soul
        );
        *ctx.ai_state = FamiliarAiState::SearchingTask;
        return true;
    }
}
