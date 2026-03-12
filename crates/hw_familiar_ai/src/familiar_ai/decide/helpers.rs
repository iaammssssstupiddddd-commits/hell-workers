//! 使い魔AIのデシジョン共通ヘルパー
//!
//! 分隊管理コンテキスト、状態遷移の最終確定など、
//! 複数のモジュールから参照される純粋ロジックを提供します。

use bevy::prelude::*;
use hw_core::familiar::{FamiliarAiState, FamiliarOperation};
use hw_core::relationships::Commanding;

use super::query_types::SoulSquadQuery;
use super::squad::SquadManager;

/// 分隊管理に必要なコンテキスト
pub struct FamiliarSquadContext<'a, 'w, 's> {
    pub fam_entity: Entity,
    pub familiar_op: &'a FamiliarOperation,
    pub commanding: Option<&'a Commanding>,
    pub q_souls: &'a SoulSquadQuery<'w, 's>,
}

pub struct SquadManagementOutcome {
    pub squad_entities: Vec<Entity>,
    pub released_entities: Vec<Entity>,
}

/// 分隊管理を実行
pub fn process_squad_management(
    ctx: &mut FamiliarSquadContext<'_, '_, '_>,
) -> SquadManagementOutcome {
    let initial_squad = SquadManager::build_squad(ctx.commanding);

    // 分隊を検証（無効なメンバーを除外）
    let (mut squad_entities, invalid_members) =
        SquadManager::validate_squad(initial_squad, ctx.fam_entity, ctx.q_souls);

    // 疲労・崩壊したメンバーをリリース要求
    let released_entities = SquadManager::release_fatigued(
        &squad_entities,
        ctx.fam_entity,
        ctx.familiar_op.fatigue_threshold,
        ctx.q_souls,
    );

    // リリースされたメンバーを分隊から除外
    if !released_entities.is_empty() {
        squad_entities.retain(|e| !released_entities.contains(e));
    }

    // 無効なメンバーも分隊から除外
    if !invalid_members.is_empty() {
        squad_entities.retain(|e| !invalid_members.contains(e));
    }

    SquadManagementOutcome {
        squad_entities,
        released_entities,
    }
}

/// 状態遷移の最終確定
pub fn finalize_state_transitions(
    ai_state: &mut FamiliarAiState,
    squad_entities: &[Entity],
    fam_entity: Entity,
    max_workers: usize,
) -> bool {
    let mut state_changed = false;

    // 分隊が空になった場合の処理
    if squad_entities.is_empty() {
        if !matches!(
            *ai_state,
            FamiliarAiState::SearchingTask
                | FamiliarAiState::Idle
                | FamiliarAiState::Scouting { .. }
        ) {
            let prev_state = ai_state.clone();
            *ai_state = FamiliarAiState::SearchingTask;
            state_changed = true;
            info!(
                "FAM_AI: {:?} squad is empty. Transitioning to SearchingTask from {:?}",
                fam_entity, prev_state
            );
        }
    } else {
        // メンバーがいる場合
        let is_squad_full = squad_entities.len() >= max_workers;

        if !matches!(*ai_state, FamiliarAiState::Scouting { .. }) {
            // 枠に空きがあるなら、監視を中断して探索へ戻れるようにする
            if !is_squad_full && matches!(*ai_state, FamiliarAiState::Supervising { .. }) {
                *ai_state = FamiliarAiState::SearchingTask;
                state_changed = true;
                info!(
                    "FAM_AI: {:?} squad has open slots ({}/{}). Switching to SearchingTask",
                    fam_entity,
                    squad_entities.len(),
                    max_workers
                );
            } else if is_squad_full && !matches!(*ai_state, FamiliarAiState::Supervising { .. }) {
                // 枠がいっぱいで、かつ監視モード以外なら監視へ
                *ai_state = FamiliarAiState::Supervising {
                    target: None,
                    timer: 0.0,
                };
                state_changed = true;
                info!("FAM_AI: {:?} squad full. -> Supervising", fam_entity);
            }
        }
    }

    state_changed
}
