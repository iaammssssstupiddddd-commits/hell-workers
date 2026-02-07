//! UIインタラクションモジュール
//!
//! ツールチップ、モードテキスト、タスクサマリー、およびボタン操作を管理します。

mod common;
mod dialog;
mod menu_actions;
mod mode;

use crate::entities::damned_soul::DamnedSoul;
use crate::entities::familiar::{Familiar, FamiliarOperation};
use crate::game_state::{BuildContext, PlayMode, TaskContext, ZoneContext};
use crate::interface::ui::components::*;
use crate::interface::ui::theme::UiTheme;
use crate::relationships::TaskWorkers;
use crate::systems::soul_ai::task_execution::AssignedTask;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use std::time::Duration;

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

pub fn hover_tooltip_system(
    hovered: Res<crate::interface::selection::HoveredEntity>,
    ui_nodes: Res<UiNodeRegistry>,
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut q_tooltip: Query<&mut Node, With<HoverTooltip>>,
    mut q_text: Query<&mut Text>,
    q_ui_tooltips: Query<(&Interaction, &UiTooltip), With<Button>>,
    q_souls: Query<(
        &DamnedSoul,
        &AssignedTask,
        &Transform,
        &crate::entities::damned_soul::IdleState,
        Option<&crate::relationships::CommandedBy>,
        Option<&crate::systems::logistics::Inventory>,
        Option<&crate::entities::damned_soul::SoulIdentity>,
    )>,
    q_blueprints: Query<&crate::systems::jobs::Blueprint>,
    q_familiars: Query<(&Familiar, &crate::entities::familiar::FamiliarOperation)>,
    q_familiars_escape: Query<(&Transform, &Familiar)>,
    familiar_grid: Res<crate::systems::spatial::FamiliarSpatialGrid>,
    q_items: Query<&crate::systems::logistics::ResourceItem>,
    q_trees: Query<&crate::systems::jobs::Tree>,
    q_rocks: Query<&crate::systems::jobs::Rock>,
    q_designations: Query<(
        &crate::systems::jobs::Designation,
        Option<&crate::systems::jobs::IssuedBy>,
        Option<&TaskWorkers>,
    )>,
    q_buildings: Query<(
        &crate::systems::jobs::Building,
        Option<&crate::systems::logistics::Stockpile>,
        Option<&crate::relationships::StoredItems>,
        Option<&crate::systems::jobs::MudMixerStorage>,
    )>,
) {
    let Ok(window) = q_window.single() else {
        return;
    };
    let Ok(mut tooltip_node) = q_tooltip.single_mut() else {
        return;
    };
    let Some(text_entity) = ui_nodes.get_slot(UiSlot::HoverTooltipText) else {
        return;
    };
    let Ok(mut text) = q_text.get_mut(text_entity) else {
        return;
    };

    if let Some((_, tooltip)) = q_ui_tooltips.iter().find(|(interaction, _)| {
        matches!(**interaction, Interaction::Hovered | Interaction::Pressed)
    }) {
        text.0 = tooltip.0.to_string();
        tooltip_node.display = Display::Flex;
        if let Some(cursor_pos) = window.cursor_position() {
            tooltip_node.left = Val::Px(cursor_pos.x + 15.0);
            tooltip_node.top = Val::Px(cursor_pos.y + 15.0);
        }
        return;
    }

    if let Some(entity) = hovered.0 {
        if let Some(model) = crate::interface::ui::presentation::build_entity_inspection_model(
            entity,
            &q_souls,
            &q_blueprints,
            &q_familiars,
            &q_familiars_escape,
            &familiar_grid,
            &q_items,
            &q_trees,
            &q_rocks,
            &q_designations,
            &q_buildings,
        ) {
            text.0 = model.tooltip_lines.join("\n");
            tooltip_node.display = Display::Flex;

            if let Some(cursor_pos) = window.cursor_position() {
                tooltip_node.left = Val::Px(cursor_pos.x + 15.0);
                tooltip_node.top = Val::Px(cursor_pos.y + 15.0);
            }
        }
        tooltip_node.display = Display::None;
    } else {
        tooltip_node.display = Display::None;
    }
}

pub fn ui_keyboard_shortcuts_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut menu_state: ResMut<MenuState>,
    mut next_play_mode: ResMut<NextState<PlayMode>>,
    mut build_context: ResMut<BuildContext>,
    mut zone_context: ResMut<ZoneContext>,
    mut task_context: ResMut<TaskContext>,
) {
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
}

pub fn update_mode_text_system(
    play_mode: Res<State<PlayMode>>,
    build_context: Res<BuildContext>,
    zone_context: Res<ZoneContext>,
    task_context: Res<TaskContext>,
    ui_nodes: Res<UiNodeRegistry>,
    mut q_text: Query<&mut Text>,
) {
    if !play_mode.is_changed()
        && !build_context.is_changed()
        && !zone_context.is_changed()
        && !task_context.is_changed()
    {
        return;
    }
    let Some(entity) = ui_nodes.get_slot(UiSlot::ModeText) else {
        return;
    };
    if let Ok(mut text) = q_text.get_mut(entity) {
        text.0 = mode::build_mode_text(
            play_mode.get(),
            &build_context,
            &zone_context,
            &task_context,
        );
    }
}

pub fn task_summary_ui_system(
    q_designations: Query<&crate::systems::jobs::Priority, With<crate::systems::jobs::Designation>>,
    ui_nodes: Res<UiNodeRegistry>,
    mut q_text: Query<&mut Text>,
) {
    let Some(entity) = ui_nodes.get_slot(UiSlot::TaskSummaryText) else {
        return;
    };
    if let Ok(mut text) = q_text.get_mut(entity) {
        let total = q_designations.iter().count();
        let high = q_designations.iter().filter(|p| p.0 > 0).count();
        text.0 = format!("Tasks: {} ({} High)", total, high);
    }
}

#[derive(Default)]
pub struct FpsCounter {
    pub frame_count: u32,
    pub elapsed_time: Duration,
}

pub fn update_fps_display_system(
    time: Res<Time>,
    mut fps_counter: Local<FpsCounter>,
    ui_nodes: Res<UiNodeRegistry>,
    mut q_text: Query<&mut Text>,
) {
    fps_counter.elapsed_time += time.delta();
    fps_counter.frame_count += 1;

    if fps_counter.elapsed_time >= Duration::from_secs(1) {
        let Some(entity) = ui_nodes.get_slot(UiSlot::FpsText) else {
            return;
        };
        if let Ok(mut text) = q_text.get_mut(entity) {
            let fps = fps_counter.frame_count as f32 / fps_counter.elapsed_time.as_secs_f32();
            text.0 = format!("FPS: {:.0}", fps);
            fps_counter.frame_count = 0;
            fps_counter.elapsed_time = Duration::ZERO;
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
    selected_entity: Res<crate::interface::selection::SelectedEntity>,
    mut q_familiar_ops: Query<&mut FamiliarOperation>,
    mut q_dialog: Query<&mut Node, With<OperationDialog>>,
    q_context_menu: Query<Entity, With<ContextMenu>>,
    mut commands: Commands,
    mut ev_max_soul_changed: MessageWriter<crate::events::FamiliarOperationMaxSoulChangedEvent>,
    theme: Res<UiTheme>,
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
            &selected_entity,
            &mut q_familiar_ops,
            &mut q_dialog,
            &mut ev_max_soul_changed,
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
