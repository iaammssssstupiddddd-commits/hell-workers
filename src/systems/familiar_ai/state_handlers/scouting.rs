//! Scouting 状態のハンドラー
//!
//! 遠方の魂をリクルートするために接近している状態の処理を行います。

use super::StateTransitionResult;
use crate::entities::damned_soul::{Destination, Path, StressBreakdown};
use crate::systems::familiar_ai::FamiliarAiState;
use bevy::prelude::*;
use crate::systems::familiar_ai::FamiliarSoulQuery;

/// Scouting 状態のハンドラー
pub fn handle_scouting_state(
    fam_entity: Entity,
    fam_pos: Vec2,
    target_soul: Entity,
    fatigue_threshold: f32,
    max_workers: usize,
    squad: &mut Vec<Entity>,
    ai_state: &mut FamiliarAiState,
    fam_dest: &mut Destination,
    fam_path: &mut Path,
    q_souls: &mut FamiliarSoulQuery,
q_breakdown: &Query<&StressBreakdown>,
request_writer: &mut MessageWriter<crate::events::SquadManagementRequest>,
) -> StateTransitionResult {
    // 既存の scouting_logic を呼び出し
    let state_changed = crate::systems::familiar_ai::scouting::scouting_logic(
        fam_entity,
        fam_pos,
        target_soul,
        fatigue_threshold,
        max_workers,
        squad,
        ai_state,
        fam_dest,
        fam_path,
        q_souls,
        q_breakdown,
        request_writer,
    );

    if state_changed {
        // 状態が変更された場合は、新しい状態を返す
        StateTransitionResult::Transition(ai_state.clone())
    } else {
        StateTransitionResult::Stay
    }
}
