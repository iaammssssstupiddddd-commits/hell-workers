//! Scouting 状態のハンドラー
//!
//! 遠方の魂をリクルートするために接近している状態の処理を行います。

use super::StateTransitionResult;
use crate::systems::familiar_ai::scouting::FamiliarScoutingContext;

/// Scouting 状態のハンドラー
pub fn handle_scouting_state(
    ctx: &mut FamiliarScoutingContext<'_, '_, '_>,
) -> StateTransitionResult {
    // 既存の scouting_logic を呼び出し
    let state_changed = crate::systems::familiar_ai::scouting::scouting_logic(ctx);

    if state_changed {
        // 状態が変更された場合は、新しい状態を返す
        StateTransitionResult::Transition(ctx.ai_state.clone())
    } else {
        StateTransitionResult::Stay
    }
}
