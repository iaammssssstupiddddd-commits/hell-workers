//! 使い魔AIの状態ハンドラーモジュール
//!
//! 各状態（Idle, SearchingTask, Scouting, Supervising）ごとに
//! 独立したハンドラー関数を提供します。

pub mod idle;
pub mod scouting;
pub mod searching;
pub mod supervising;

use crate::systems::familiar_ai::FamiliarAiState;

/// 状態遷移の結果
#[derive(Debug, Clone)]
pub enum StateTransitionResult {
    /// 状態を維持
    Stay,
    /// 状態を変更
    Transition(FamiliarAiState),
}

impl StateTransitionResult {
    pub fn apply_to(self, current_state: &mut FamiliarAiState) -> bool {
        match self {
            StateTransitionResult::Stay => false,
            StateTransitionResult::Transition(new_state) => {
                *current_state = new_state;
                true
            }
        }
    }
}
