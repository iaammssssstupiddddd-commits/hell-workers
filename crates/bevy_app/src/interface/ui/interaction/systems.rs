use crate::entities::familiar::{Familiar, FamiliarOperation};
use crate::input_actions::ActiveModeCleanupParams;
use crate::systems::jobs::{Door, DoorState, apply_door_state};
use crate::world::map::WorldMapWrite;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use hw_core::game_state::PlayMode;
use hw_ui::UiIntent;
use hw_ui::components::*;
use hw_ui::interaction::common::update_interaction_color;
use hw_ui::interaction::dialog::close_operation_dialog;
use hw_ui::theme::UiTheme;
use hw_world::DoorVisualHandles;

use super::menu_actions;

type MenuButtonWithColorQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Interaction,
        &'static MenuButton,
        &'static mut BackgroundColor,
    ),
    (Changed<Interaction>, With<Button>),
>;

type MenuButtonQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Interaction, &'static MenuButton),
    (Changed<Interaction>, With<Button>),
>;

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

/// UI ボタンの操作を受け取り、`UiIntent` を発行する統合システム
pub fn ui_interaction_system(
    mut interaction_query: MenuButtonWithColorQuery,
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

        super::despawn_context_menus(&mut commands, &q_context_menu);
        menu_actions::handle_pressed_action(menu_button.0, &mut ui_intent_writer);
    }
}

/// `SelectArchitectCategory` アクションを処理する専用システム
/// `SelectArchitectCategory` を専用で処理するシステム
/// 2回押下時のトグル仕様をここで維持する。
pub fn arch_category_action_system(
    interaction_query: MenuButtonQuery,
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
    interaction_query: MenuButtonQuery,
    mut selected_entity: ResMut<crate::interface::selection::SelectedEntity>,
    play_mode: Res<State<PlayMode>>,
    resolved_frame: Res<crate::input_actions::ResolvedInputFrame>,
    mut cleanup: ActiveModeCleanupParams,
) {
    if resolved_frame.pointer_selection_suppressed() {
        return;
    }
    for (interaction, menu_button) in interaction_query.iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let MenuAction::MovePlantBuilding(entity) = menu_button.0 else {
            continue;
        };
        if cleanup.has_active_owner_state(play_mode.get()) {
            cleanup.cancel_active_mode();
        }
        selected_entity.0 = Some(entity);
        cleanup.move_context.0 = Some(entity);
        cleanup.move_placement_state.0 = None;
        cleanup.companion_state.0 = None;
        cleanup.next_play_mode.set(PlayMode::BuildingMove);
    }
}

/// `ToggleDoorLock` を専用で処理するシステム
/// ドアのロック状態と見た目を即時反映する。
pub fn door_lock_action_system(
    interaction_query: MenuButtonQuery,
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
            if let Some(entity) = ui_nodes.get_slot(UiSlot::DialogFamiliarName)
                && let Ok(mut text) = q_text.get_mut(entity)
            {
                text.0 = format!("Editing: {}", familiar.name);
            }
            if let Some(entity) = ui_nodes.get_slot(UiSlot::DialogThresholdText)
                && let Ok(mut text) = q_text.get_mut(entity)
            {
                let val_str = format!("{:.0}%", op.fatigue_threshold * 100.0);
                if text.0 != val_str {
                    text.0 = val_str;
                }
            }
            if let Some(entity) = ui_nodes.get_slot(UiSlot::DialogMaxSoulText)
                && let Ok(mut text) = q_text.get_mut(entity)
            {
                let val_str = format!("{}", op.max_controlled_soul);
                if text.0 != val_str {
                    text.0 = val_str;
                }
            }
        } else {
            close_operation_dialog(&mut q_dialog);
        }
    } else {
        close_operation_dialog(&mut q_dialog);
    }
}
