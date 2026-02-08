//! 使い魔AIの状態遷移検知とイベント発火
//!
//! Bevy の `Changed<T>` フィルタを使用して状態遷移を検知し、
//! イベントを発火するシステムを提供します。

use crate::entities::familiar::{ActiveCommand, FamiliarCommand};
use crate::events::{FamiliarAiStateChangedEvent, FamiliarAiStateTransitionReason};
use crate::systems::familiar_ai::FamiliarAiState;
use bevy::prelude::*;

/// 前フレームの状態を保存するコンポーネント
#[derive(Component, Default)]
pub struct FamiliarAiStateHistory {
    pub last_state: FamiliarAiState,
}

/// 状態が変更された時のみ処理するシステム
/// Bevy の `Changed<FamiliarAiState>` フィルタを使用
pub fn detect_state_changes_system(
    mut q_familiars: Query<
        (Entity, &FamiliarAiState, &mut FamiliarAiStateHistory),
        Changed<FamiliarAiState>,
    >,
    mut ev_state_changed: MessageWriter<FamiliarAiStateChangedEvent>,
) {
    for (entity, new_state, mut history) in q_familiars.iter_mut() {
        let from_state = history.last_state.clone();

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
            history.last_state = new_state.clone();
        }
    }
}

/// コマンドが変更された時のみ処理するシステム
/// Bevy の `Changed<ActiveCommand>` フィルタを使用
pub fn detect_command_changes_system(
    mut q_familiars: Query<
        (
            Entity,
            &ActiveCommand,
            &FamiliarAiState,
            &mut FamiliarAiStateHistory,
        ),
        Changed<ActiveCommand>,
    >,
    mut ev_state_changed: MessageWriter<FamiliarAiStateChangedEvent>,
) {
    for (entity, active_command, current_state, mut history) in q_familiars.iter_mut() {
        // コマンドが Idle に変更された場合、状態も Idle に遷移する可能性が高い
        if matches!(active_command.command, FamiliarCommand::Idle) {
            if !matches!(current_state, FamiliarAiState::Idle) {
                let from_state = history.last_state.clone();

                // イベントを発火
                ev_state_changed.write(FamiliarAiStateChangedEvent {
                    familiar_entity: entity,
                    from: from_state,
                    to: FamiliarAiState::Idle,
                    reason: FamiliarAiStateTransitionReason::CommandChanged,
                });

                history.last_state = FamiliarAiState::Idle;
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
