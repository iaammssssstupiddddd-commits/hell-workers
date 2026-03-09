//! Scouting 状態のハンドラー
//!
//! 遠方の魂をリクルートするために接近している状態の処理を行います。

use super::StateTransitionResult;
use crate::familiar_ai::decide::scouting::{FamiliarScoutingContext, ScoutingOutcome};

pub struct ScoutingStateTransition {
    pub transition: StateTransitionResult,
    pub recruited_entity: Option<bevy::prelude::Entity>,
}

/// Scouting 状態のハンドラー
pub fn handle_scouting_state(
    ctx: &mut FamiliarScoutingContext<'_, '_, '_>,
) -> ScoutingStateTransition {
    let ScoutingOutcome {
        state_changed,
        recruited_entity,
    } = crate::familiar_ai::decide::scouting::scouting_logic(ctx);

    if state_changed {
        // 状態が変更された場合は、新しい状態を返す
        ScoutingStateTransition {
            transition: StateTransitionResult::Transition(ctx.ai_state.clone()),
            recruited_entity,
        }
    } else {
        ScoutingStateTransition {
            transition: StateTransitionResult::Stay,
            recruited_entity,
        }
    }
}
