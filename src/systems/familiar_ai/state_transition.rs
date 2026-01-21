//! 使い魔AIの状態遷移検知とイベント発火
//!
//! Bevy の `Changed<T>` フィルタを使用して状態遷移を検知し、
//! イベントを発火するシステムを提供します。

use crate::entities::familiar::{ActiveCommand, FamiliarCommand};
use crate::events::{FamiliarAiStateChangedEvent, FamiliarAiStateTransitionReason};
use crate::systems::familiar_ai::FamiliarAiState;
use bevy::prelude::*;
use std::collections::HashMap;

/// 前フレームの状態を保存するリソース
#[derive(Resource, Default)]
pub struct PreviousFamiliarAiStates {
    states: HashMap<Entity, FamiliarAiState>,
}

/// 状態が変更された時のみ処理するシステム
/// Bevy の `Changed<FamiliarAiState>` フィルタを使用
pub fn detect_state_changes_system(
    q_familiars: Query<(Entity, &FamiliarAiState), Changed<FamiliarAiState>>,
    mut previous_states: ResMut<PreviousFamiliarAiStates>,
    mut ev_state_changed: MessageWriter<FamiliarAiStateChangedEvent>,
) {
    for (entity, new_state) in q_familiars.iter() {
        let from_state = previous_states
            .states
            .get(&entity)
            .cloned()
            .unwrap_or_else(|| FamiliarAiState::default());

        // 実際に状態が異なっている場合のみ発火
        if from_state != *new_state {
            // 状態遷移の理由を判定
            let reason = determine_transition_reason(&from_state, new_state);

            // イベントを発火
            ev_state_changed.write(FamiliarAiStateChangedEvent {
                familiar_entity: entity,
                from: from_state.clone(),
                to: new_state.clone(),
                reason,
            });

            // 前の状態を更新
            previous_states.states.insert(entity, new_state.clone());
        }
    }
}

/// コマンドが変更された時のみ処理するシステム
/// Bevy の `Changed<ActiveCommand>` フィルタを使用
pub fn detect_command_changes_system(
    q_familiars: Query<(Entity, &ActiveCommand), Changed<ActiveCommand>>,
    q_ai_states: Query<&FamiliarAiState>,
    mut ev_state_changed: MessageWriter<FamiliarAiStateChangedEvent>,
    mut previous_states: ResMut<PreviousFamiliarAiStates>,
) {
    for (entity, active_command) in q_familiars.iter() {
        // コマンドが Idle に変更された場合、状態も Idle に遷移する可能性が高い
        if matches!(active_command.command, FamiliarCommand::Idle) {
            if let Ok(current_state) = q_ai_states.get(entity) {
                if !matches!(current_state, FamiliarAiState::Idle) {
                    let from_state = previous_states
                        .states
                        .get(&entity)
                        .cloned()
                        .unwrap_or_else(|| current_state.clone());

                    // イベントを発火
                    ev_state_changed.write(FamiliarAiStateChangedEvent {
                        familiar_entity: entity,
                        from: from_state.clone(),
                        to: FamiliarAiState::Idle,
                        reason: FamiliarAiStateTransitionReason::CommandChanged,
                    });

                    previous_states.states.insert(entity, FamiliarAiState::Idle);
                }
            }
        }
    }
}

/// 状態遷移の理由を判定する
pub fn determine_transition_reason(
    from: &FamiliarAiState,
    to: &FamiliarAiState,
) -> FamiliarAiStateTransitionReason {
    match (from, to) {
        (_, FamiliarAiState::Idle) => FamiliarAiStateTransitionReason::CommandChanged,
        (FamiliarAiState::Scouting { .. }, FamiliarAiState::Supervising { .. }) => {
            FamiliarAiStateTransitionReason::SquadFull
        }
        (FamiliarAiState::Scouting { .. }, FamiliarAiState::SearchingTask) => {
            FamiliarAiStateTransitionReason::ScoutingCancelled
        }
        (FamiliarAiState::Supervising { .. }, FamiliarAiState::SearchingTask) => {
            FamiliarAiStateTransitionReason::SquadEmpty
        }
        (FamiliarAiState::SearchingTask, FamiliarAiState::Scouting { .. }) => {
            FamiliarAiStateTransitionReason::RecruitSuccess
        }
        _ => FamiliarAiStateTransitionReason::Unknown,
    }
}

/// 状態遷移イベントを処理するシステム
/// ログ出力やその他の副作用を処理
pub fn handle_state_changed_system(
    mut ev_state_changed: MessageReader<FamiliarAiStateChangedEvent>,
) {
    for event in ev_state_changed.read() {
        debug!(
            "FAM_AI: {:?} state changed: {:?} -> {:?} (reason: {:?})",
            event.familiar_entity, event.from, event.to, event.reason
        );
        // ここで状態遷移に応じた処理（アニメーション、音声など）を追加可能
    }
}

/// エンティティが削除された時に前の状態をクリーンアップ
pub fn cleanup_previous_states_system(
    mut removed: RemovedComponents<FamiliarAiState>,
    mut previous_states: ResMut<PreviousFamiliarAiStates>,
) {
    for entity in removed.read() {
        previous_states.states.remove(&entity);
    }
}
