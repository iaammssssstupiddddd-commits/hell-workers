//! Supervising 状態のハンドラー
//!
//! 配下の魂を監視し、仕事の進捗を管理している状態の処理を行います。

use super::StateTransitionResult;
use crate::systems::familiar_ai::helpers::supervising::FamiliarSupervisingContext;

/// Supervising 状態のハンドラー
pub fn handle_supervising_state(
    ctx: &mut FamiliarSupervisingContext<'_, '_, '_>,
) -> StateTransitionResult {
    // 既存の supervising_logic を呼び出し
    crate::systems::familiar_ai::helpers::supervising::supervising_logic(ctx);

    StateTransitionResult::Stay
}
