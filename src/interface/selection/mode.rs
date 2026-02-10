use crate::game_state::{
    BuildContext, CompanionPlacementState, PlayMode, TaskContext, ZoneContext,
};
use crate::interface::ui::MenuState;
use crate::systems::command::TaskMode;
use bevy::prelude::*;

pub fn clear_companion_state_outside_build_mode(
    play_mode: Res<State<PlayMode>>,
    mut companion_state: ResMut<CompanionPlacementState>,
) {
    if *play_mode.get() != PlayMode::BuildingPlace && companion_state.0.is_some() {
        companion_state.0 = None;
    }
}

/// Escキーでビルド/ゾーン/タスクモードを解除し、PlayMode::Normalに戻す
/// 共通仕様: Normalに戻る際はMenuStateもHiddenに戻す
pub fn build_mode_cancel_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    play_mode: Res<State<PlayMode>>,
    mut next_play_mode: ResMut<NextState<PlayMode>>,
    mut build_context: ResMut<BuildContext>,
    mut zone_context: ResMut<ZoneContext>,
    mut task_context: ResMut<TaskContext>,
    mut companion_state: ResMut<CompanionPlacementState>,
    mut menu_state: ResMut<MenuState>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        let current_mode = play_mode.get();
        if *current_mode == PlayMode::BuildingPlace {
            companion_state.0 = None;
            build_context.0 = None;
            next_play_mode.set(PlayMode::Normal);
            *menu_state = MenuState::Hidden;
            info!("STATE: Cancelled BuildingPlace -> Normal, Menu hidden");
        } else if *current_mode == PlayMode::ZonePlace {
            zone_context.0 = None;
            next_play_mode.set(PlayMode::Normal);
            *menu_state = MenuState::Hidden;
            info!("STATE: Cancelled ZonePlace -> Normal, Menu hidden");
        } else if *current_mode == PlayMode::TaskDesignation {
            task_context.0 = TaskMode::None;
            next_play_mode.set(PlayMode::Normal);
            *menu_state = MenuState::Hidden;
            info!("STATE: Cancelled TaskDesignation -> Normal, Menu hidden");
        }
    }
}
