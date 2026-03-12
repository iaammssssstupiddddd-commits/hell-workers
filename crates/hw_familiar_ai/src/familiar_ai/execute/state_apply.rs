use bevy::prelude::*;

use hw_core::events::FamiliarStateRequest;
use hw_core::familiar::FamiliarAiState;

/// FamiliarStateRequest を適用する（Execute Phase）
pub fn familiar_state_apply_system(
    mut request_reader: MessageReader<FamiliarStateRequest>,
    mut q_ai_state: Query<&mut FamiliarAiState>,
) {
    for request in request_reader.read() {
        if let Ok(mut ai_state) = q_ai_state.get_mut(request.familiar_entity) {
            *ai_state = request.new_state.clone();
        }
    }
}
