use crate::game_state::{CompanionPlacementState, PlayMode};
use bevy::prelude::*;

pub fn clear_companion_state_outside_build_mode(
    play_mode: Res<State<PlayMode>>,
    mut companion_state: ResMut<CompanionPlacementState>,
) {
    if *play_mode.get() != PlayMode::BuildingPlace && companion_state.0.is_some() {
        companion_state.0 = None;
    }
}
