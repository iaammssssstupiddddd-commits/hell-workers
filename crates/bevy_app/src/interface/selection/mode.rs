use crate::app_contexts::CompanionPlacementState;
use bevy::prelude::*;
use hw_core::game_state::PlayMode;

pub fn clear_companion_state_outside_build_mode(
    play_mode: Res<State<PlayMode>>,
    mut companion_state: ResMut<CompanionPlacementState>,
) {
    if *play_mode.get() != PlayMode::BuildingPlace
        && *play_mode.get() != PlayMode::BuildingMove
        && companion_state.0.is_some()
    {
        companion_state.0 = None;
    }
}
