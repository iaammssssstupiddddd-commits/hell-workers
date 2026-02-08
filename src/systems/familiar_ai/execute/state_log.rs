use crate::events::FamiliarAiStateChangedEvent;
use bevy::prelude::*;

/// 状態遷移イベントを処理するシステム（Execute Phase）
pub fn handle_state_changed_system(
    mut ev_state_changed: MessageReader<FamiliarAiStateChangedEvent>,
) {
    for event in ev_state_changed.read() {
        debug!(
            "FAM_AI: {:?} state changed: {:?} -> {:?} (reason: {:?})",
            event.familiar_entity, event.from, event.to, event.reason
        );
    }
}
