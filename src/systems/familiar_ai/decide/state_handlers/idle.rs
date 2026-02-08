//! Idle 状態のハンドラー
//!
//! プレイヤーからの Idle コマンドが発行された際の処理を行います。

use super::StateTransitionResult;
use crate::entities::damned_soul::{Destination, Path};
use crate::entities::familiar::{ActiveCommand, FamiliarCommand};
use crate::systems::familiar_ai::FamiliarAiState;
use bevy::prelude::*;

/// Idle 状態のハンドラー
///
/// Decide フェーズで使用するため、Commands を使わずに
/// 状態遷移判定と自己エンティティの移動停止のみを行う。
pub fn handle_idle_state(
    active_command: &ActiveCommand,
    current_state: &FamiliarAiState,
    fam_pos: Vec2,
    fam_dest: &mut Destination,
    fam_path: &mut Path,
) -> StateTransitionResult {
    if !matches!(active_command.command, FamiliarCommand::Idle) {
        return StateTransitionResult::Stay;
    }

    // Idle命令中は停止状態を維持
    fam_dest.0 = fam_pos;
    fam_path.waypoints.clear();
    fam_path.current_index = 0;

    if *current_state == FamiliarAiState::Idle {
        StateTransitionResult::Stay
    } else {
        StateTransitionResult::Transition(FamiliarAiState::Idle)
    }
}
