//! UIインタラクションモジュール
//!
//! ツールチップ、モードテキスト、タスクサマリー、およびボタン操作を管理します。

mod intent_handler;
mod menu_actions;
mod mode;
mod status_display;
mod tooltip;

pub(crate) use hw_ui::interaction::common::despawn_context_menus;
pub(crate) use intent_handler::handle_ui_intent;

pub use hw_ui::interaction::hover_action::hover_action_button_system;
pub use status_display::{
    task_summary_ui_system, update_area_edit_preview_ui_system, update_dream_loss_popup_ui_system,
    update_dream_pool_display_system, update_fps_display_system, update_mode_text_system,
    update_speed_button_highlight_system,
};
pub(crate) use tooltip::hover_tooltip_system;

use crate::app_contexts::{
    BuildContext, CompanionPlacementState, MoveContext, MovePlacementState, TaskContext,
    ZoneContext,
};
use crate::entities::familiar::{Familiar, FamiliarOperation};
use hw_ui::components::*;
use hw_ui::interaction::common::update_interaction_color;
use hw_ui::interaction::dialog::close_operation_dialog;
use hw_ui::theme::UiTheme;
use crate::systems::command::TaskMode;
use crate::systems::jobs::{Door, DoorState, apply_door_state};
use crate::world::map::WorldMapWrite;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use hw_core::game_state::PlayMode;
use hw_ui::UiIntent;
use hw_world::DoorVisualHandles;

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
        } else if *current_mode == PlayMode::ZonePlace {
            zone_context.0 = None;
            next_play_mode.set(PlayMode::Normal);
            *menu_state = MenuState::Hidden;
        } else if *current_mode == PlayMode::TaskDesignation {
            task_context.0 = TaskMode::None;
            next_play_mode.set(PlayMode::Normal);
            *menu_state = MenuState::Hidden;
        }
    }
}

/// UI ボタンの操作を受け取り、`UiIntent` を発行する統合システム
pub fn ui_interaction_system(
    mut interaction_query: Query<
        (&Interaction, &MenuButton, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
    q_context_menu: Query<Entity, With<ContextMenu>>,
    mut commands: Commands,
    mut ui_intent_writer: MessageWriter<UiIntent>,
    theme: Res<UiTheme>,
) {
    for (interaction, menu_button, mut color) in interaction_query.iter_mut() {
        update_interaction_color(*interaction, &mut color, &theme);
        if *interaction != Interaction::Pressed {
            continue;
        }

        despawn_context_menus(&mut commands, &q_context_menu);
        menu_actions::handle_pressed_action(menu_button.0, &mut ui_intent_writer);
    }
}

/// `SelectArchitectCategory` アクションを処理する専用システム
/// `SelectArchitectCategory` を専用で処理するシステム
/// 2回押下時のトグル仕様をここで維持する。
pub fn arch_category_action_system(
    interaction_query: Query<(&Interaction, &MenuButton), (Changed<Interaction>, With<Button>)>,
    mut arch_category_state: ResMut<hw_ui::components::ArchitectCategoryState>,
) {
    for (interaction, menu_button) in interaction_query.iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let MenuAction::SelectArchitectCategory(category) = menu_button.0 {
            // 同じカテゴリを再度押した場合はトグルして非表示にする
            arch_category_state.0 = if arch_category_state.0 == category {
                None
            } else {
                category
            };
        }
    }
}

/// `MovePlantBuilding` を専用で処理するシステム
/// Plantの移動先選択モードへ遷移する。
pub fn move_plant_building_action_system(
    interaction_query: Query<(&Interaction, &MenuButton), (Changed<Interaction>, With<Button>)>,
    mut selected_entity: ResMut<crate::interface::selection::SelectedEntity>,
    mut move_context: ResMut<MoveContext>,
    mut move_placement_state: ResMut<MovePlacementState>,
    mut companion_state: ResMut<CompanionPlacementState>,
    mut next_play_mode: ResMut<NextState<PlayMode>>,
) {
    for (interaction, menu_button) in interaction_query.iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let MenuAction::MovePlantBuilding(entity) = menu_button.0 else {
            continue;
        };
        selected_entity.0 = Some(entity);
        move_context.0 = Some(entity);
        move_placement_state.0 = None;
        companion_state.0 = None;
        next_play_mode.set(PlayMode::BuildingMove);
    }
}

/// `ToggleDoorLock` を専用で処理するシステム
/// ドアのロック状態と見た目を即時反映する。
pub fn door_lock_action_system(
    interaction_query: Query<(&Interaction, &MenuButton), (Changed<Interaction>, With<Button>)>,
    mut q_doors: Query<(&Transform, &mut Door, &mut Sprite)>,
    mut world_map: WorldMapWrite,
    door_visual_handles: Res<DoorVisualHandles>,
) {
    for (interaction, menu_button) in interaction_query.iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let MenuAction::ToggleDoorLock(entity) = menu_button.0 else {
            continue;
        };
        if let Ok((transform, mut door, mut sprite)) = q_doors.get_mut(entity) {
            let door_grid =
                crate::world::map::WorldMap::world_to_grid(transform.translation.truncate());
            let next_state = if door.state == DoorState::Locked {
                DoorState::Closed
            } else {
                DoorState::Locked
            };
            apply_door_state(
                &mut door,
                &mut sprite,
                &mut world_map,
                &door_visual_handles,
                door_grid,
                next_state,
            );
        }
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
            close_operation_dialog(&mut q_dialog);
        }
    } else {
        close_operation_dialog(&mut q_dialog);
    }
}
