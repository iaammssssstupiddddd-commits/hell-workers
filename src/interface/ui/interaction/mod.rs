//! UIインタラクションモジュール
//!
//! ツールチップ、モードテキスト、タスクサマリー、およびボタン操作を管理します。

mod common;
mod dialog;
mod menu_actions;
mod mode;
mod status_display;
mod tooltip;

pub(crate) use common::despawn_context_menus;

pub use status_display::{
    task_summary_ui_system, update_area_edit_preview_ui_system, update_fps_display_system,
    update_mode_text_system,
};
pub(crate) use tooltip::hover_tooltip_system;

use crate::entities::familiar::{Familiar, FamiliarOperation};
use crate::game_state::{BuildContext, CompanionPlacementState, PlayMode, TaskContext, ZoneContext};
use crate::interface::ui::components::*;
use crate::interface::ui::theme::UiTheme;
use crate::systems::command::{TaskArea, TaskMode};
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

pub fn update_ui_input_state_system(
    mut ui_input_state: ResMut<UiInputState>,
    q_blockers: Query<&RelativeCursorPosition, With<UiInputBlocker>>,
    q_buttons: Query<&Interaction, With<Button>>,
) {
    let pointer_over_blocker = q_blockers.iter().any(RelativeCursorPosition::cursor_over);
    let pointer_over_button = q_buttons
        .iter()
        .any(|interaction| matches!(*interaction, Interaction::Hovered | Interaction::Pressed));
    ui_input_state.pointer_over_ui = pointer_over_blocker || pointer_over_button;
}

pub fn ui_keyboard_shortcuts_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut menu_state: ResMut<MenuState>,
    mut next_play_mode: ResMut<NextState<PlayMode>>,
    mut build_context: ResMut<BuildContext>,
    mut zone_context: ResMut<ZoneContext>,
    mut task_context: ResMut<TaskContext>,
    mut time: ResMut<Time<Virtual>>,
    play_mode: Res<State<PlayMode>>,
    mut companion_state: ResMut<CompanionPlacementState>,
) {
    // メニュートグル
    if keyboard.just_pressed(KeyCode::KeyB) {
        mode::toggle_menu_and_reset_mode(
            &mut menu_state,
            MenuState::Architect,
            &mut next_play_mode,
            &mut build_context,
            &mut zone_context,
            &mut task_context,
            false,
        );
    }

    if keyboard.just_pressed(KeyCode::KeyZ) {
        mode::toggle_menu_and_reset_mode(
            &mut menu_state,
            MenuState::Zones,
            &mut next_play_mode,
            &mut build_context,
            &mut zone_context,
            &mut task_context,
            true,
        );
    }

    // 時間制御
    if keyboard.just_pressed(KeyCode::Space) {
        if time.is_paused() {
            time.unpause();
        } else {
            time.pause();
        }
    }
    if keyboard.just_pressed(KeyCode::Digit1) {
        time.pause();
    }
    if keyboard.just_pressed(KeyCode::Digit2) {
        time.unpause();
        time.set_relative_speed(1.0);
    }
    if keyboard.just_pressed(KeyCode::Digit3) {
        time.unpause();
        time.set_relative_speed(2.0);
    }
    if keyboard.just_pressed(KeyCode::Digit4) {
        time.unpause();
        time.set_relative_speed(4.0);
    }

    // モードキャンセル (Escape)
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

/// UI ボタンの操作を管理する統合システム
pub fn ui_interaction_system(
    mut interaction_query: Query<
        (&Interaction, &MenuButton, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
    mut menu_state: ResMut<MenuState>,
    mut next_play_mode: ResMut<NextState<PlayMode>>,
    mut build_context: ResMut<BuildContext>,
    mut zone_context: ResMut<ZoneContext>,
    mut task_context: ResMut<TaskContext>,
    mut selected_entity: ResMut<crate::interface::selection::SelectedEntity>,
    mut info_panel_pin: ResMut<crate::interface::ui::InfoPanelPinState>,
    mut q_familiar_ops: Query<&mut FamiliarOperation>,
    q_familiars_for_area: Query<(Entity, Option<&TaskArea>), With<Familiar>>,
    mut q_dialog: Query<&mut Node, With<OperationDialog>>,
    q_context_menu: Query<Entity, With<ContextMenu>>,
    mut commands: Commands,
    mut ev_max_soul_changed: MessageWriter<crate::events::FamiliarOperationMaxSoulChangedEvent>,
    theme: Res<UiTheme>,
    mut time: ResMut<Time<Virtual>>,
) {
    for (interaction, menu_button, mut color) in interaction_query.iter_mut() {
        common::update_interaction_color(*interaction, &mut color, &theme);
        if *interaction != Interaction::Pressed {
            continue;
        }

        common::despawn_context_menus(&mut commands, &q_context_menu);
        menu_actions::handle_pressed_action(
            menu_button.0,
            &mut menu_state,
            &mut next_play_mode,
            &mut build_context,
            &mut zone_context,
            &mut task_context,
            &mut selected_entity,
            &mut info_panel_pin,
            &mut q_familiar_ops,
            &q_familiars_for_area,
            &mut q_dialog,
            &mut ev_max_soul_changed,
            &mut time,
        );
    }
}

/// Operation Dialog のテキスト表示を更新するシステム
pub fn update_operation_dialog_system(
    selected_entity: Res<crate::interface::selection::SelectedEntity>,
    ui_nodes: Res<UiNodeRegistry>,
    q_familiars: Query<(&Familiar, &FamiliarOperation)>,
    mut q_dialog: Query<&mut Node, With<OperationDialog>>,
    mut q_text: Query<&mut Text>,
) {
    if let Some(selected) = selected_entity.0 {
        if let Ok((familiar, op)) = q_familiars.get(selected) {
            if let Some(entity) = ui_nodes.get_slot(UiSlot::DialogFamiliarName) {
                if let Ok(mut text) = q_text.get_mut(entity) {
                    text.0 = format!("Editing: {}", familiar.name);
                }
            }
            if let Some(entity) = ui_nodes.get_slot(UiSlot::DialogThresholdText) {
                if let Ok(mut text) = q_text.get_mut(entity) {
                    let val_str = format!("{:.0}%", op.fatigue_threshold * 100.0);
                    if text.0 != val_str {
                        text.0 = val_str;
                    }
                }
            }
            if let Some(entity) = ui_nodes.get_slot(UiSlot::DialogMaxSoulText) {
                if let Ok(mut text) = q_text.get_mut(entity) {
                    let val_str = format!("{}", op.max_controlled_soul);
                    if text.0 != val_str {
                        text.0 = val_str;
                    }
                }
            }
        } else {
            dialog::close_operation_dialog(&mut q_dialog);
        }
    } else {
        dialog::close_operation_dialog(&mut q_dialog);
    }
}
