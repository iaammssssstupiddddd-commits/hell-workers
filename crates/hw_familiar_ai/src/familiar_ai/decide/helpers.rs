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
        ctx.familiar_op.release_fatigue_threshold(),
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
            debug!(
                "FAM_AI: {:?} squad is empty. Transitioning to SearchingTask from {:?}",
                fam_entity, prev_state
            );
        }
    } else if !matches!(
        *ai_state,
        FamiliarAiState::Scouting { .. } | FamiliarAiState::Supervising { .. }
    ) {
        // 既存分隊の判断経路では、この確定処理より先に追加募集を試している。
        // 候補をScouting中でなければ、空き枠があっても既存メンバーを監視する。
        *ai_state = FamiliarAiState::Supervising {
            target: None,
            timer: 0.0,
        };
        state_changed = true;
        debug!(
            "FAM_AI: {:?} has {} squad member(s). -> Supervising",
            fam_entity,
            squad_entities.len()
        );
    }

    state_changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_squad_transitions_to_supervising() {
        let familiar = Entity::from_bits(1);
        let soul = Entity::from_bits(2);
        let mut state = FamiliarAiState::SearchingTask;

        let changed = finalize_state_transitions(&mut state, &[soul], familiar);

        assert!(changed);
        assert!(matches!(state, FamiliarAiState::Supervising { .. }));
    }

    #[test]
    fn partial_squad_keeps_active_scouting() {
        let familiar = Entity::from_bits(1);
        let squad_soul = Entity::from_bits(2);
        let recruit = Entity::from_bits(3);
        let mut state = FamiliarAiState::Scouting {
            target_soul: recruit,
        };

        let changed = finalize_state_transitions(&mut state, &[squad_soul], familiar);

        assert!(!changed);
        assert_eq!(
            state,
            FamiliarAiState::Scouting {
                target_soul: recruit
            }
        );
    }

    #[test]
    fn partial_squad_preserves_existing_supervision_target_and_timer() {
        let familiar = Entity::from_bits(1);
        let soul = Entity::from_bits(2);
        let mut state = FamiliarAiState::Supervising {
            target: Some(soul),
            timer: 1.25,
        };

        let changed = finalize_state_transitions(&mut state, &[soul], familiar);

        assert!(!changed);
        assert_eq!(
            state,
            FamiliarAiState::Supervising {
                target: Some(soul),
                timer: 1.25
            }
        );
    }

    #[test]
    fn empty_squad_leaves_supervising_for_searching() {
        let familiar = Entity::from_bits(1);
        let mut state = FamiliarAiState::Supervising {
            target: None,
            timer: 0.0,
        };

        let changed = finalize_state_transitions(&mut state, &[], familiar);

        assert!(changed);
        assert_eq!(state, FamiliarAiState::SearchingTask);
    }
}
