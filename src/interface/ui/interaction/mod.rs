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
use crate::interface::ui::panels::tooltip_builder;
use crate::interface::ui::presentation::EntityInspectionModel;
use crate::interface::ui::theme::UiTheme;
use crate::relationships::TaskWorkers;
use crate::systems::soul_ai::task_execution::AssignedTask;
use bevy::ecs::system::SystemParam;
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

#[derive(Default)]
pub(crate) struct TooltipRuntimeState {
    target: Option<TooltipTarget>,
    payload: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TooltipTarget {
    UiButton(Entity),
    WorldEntity(Entity),
}

#[derive(SystemParam)]
pub(crate) struct TooltipInspectionQueryParam<'w, 's> {
    q_souls: Query<
        'w,
        's,
        (
            &'static DamnedSoul,
            &'static AssignedTask,
            &'static Transform,
            &'static crate::entities::damned_soul::IdleState,
            Option<&'static crate::relationships::CommandedBy>,
            Option<&'static crate::systems::logistics::Inventory>,
            Option<&'static crate::entities::damned_soul::SoulIdentity>,
        ),
    >,
    q_blueprints: Query<'w, 's, &'static crate::systems::jobs::Blueprint>,
    q_familiars: Query<
        'w,
        's,
        (
            &'static Familiar,
            &'static crate::entities::familiar::FamiliarOperation,
        ),
    >,
    q_familiars_escape: Query<'w, 's, (&'static Transform, &'static Familiar)>,
    familiar_grid: Res<'w, crate::systems::spatial::FamiliarSpatialGrid>,
    q_items: Query<'w, 's, &'static crate::systems::logistics::ResourceItem>,
    q_trees: Query<'w, 's, &'static crate::systems::jobs::Tree>,
    q_rocks: Query<'w, 's, &'static crate::systems::jobs::Rock>,
    q_designations: Query<
        'w,
        's,
        (
            &'static crate::systems::jobs::Designation,
            Option<&'static crate::systems::jobs::IssuedBy>,
            Option<&'static TaskWorkers>,
        ),
    >,
    q_buildings: Query<
        'w,
        's,
        (
            &'static crate::systems::jobs::Building,
            Option<&'static crate::systems::logistics::Stockpile>,
            Option<&'static crate::relationships::StoredItems>,
            Option<&'static crate::systems::jobs::MudMixerStorage>,
        ),
    >,
}

fn classify_tooltip_template(
    entity: Entity,
    q_souls: &Query<(
        &DamnedSoul,
        &AssignedTask,
        &Transform,
        &crate::entities::damned_soul::IdleState,
        Option<&crate::relationships::CommandedBy>,
        Option<&crate::systems::logistics::Inventory>,
        Option<&crate::entities::damned_soul::SoulIdentity>,
    )>,
    q_blueprints: &Query<&crate::systems::jobs::Blueprint>,
    q_items: &Query<&crate::systems::logistics::ResourceItem>,
    q_trees: &Query<&crate::systems::jobs::Tree>,
    q_rocks: &Query<&crate::systems::jobs::Rock>,
    q_buildings: &Query<(
        &crate::systems::jobs::Building,
        Option<&crate::systems::logistics::Stockpile>,
        Option<&crate::relationships::StoredItems>,
        Option<&crate::systems::jobs::MudMixerStorage>,
    )>,
) -> TooltipTemplate {
    if q_souls.get(entity).is_ok() {
        TooltipTemplate::Soul
    } else if q_buildings.get(entity).is_ok() || q_blueprints.get(entity).is_ok() {
        TooltipTemplate::Building
    } else if q_items.get(entity).is_ok()
        || q_trees.get(entity).is_ok()
        || q_rocks.get(entity).is_ok()
    {
        TooltipTemplate::Resource
    } else {
        TooltipTemplate::Generic
    }
}

pub fn hover_tooltip_system(
    mut commands: Commands,
    time: Res<Time>,
    hovered: Res<crate::interface::selection::HoveredEntity>,
    ui_nodes: Res<UiNodeRegistry>,
    game_assets: Res<crate::assets::GameAssets>,
    theme: Res<UiTheme>,
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut q_tooltip: Query<(
        Entity,
        &mut HoverTooltip,
        &mut Node,
        &mut BackgroundColor,
        &mut BorderColor,
    )>,
    q_children: Query<&Children>,
    mut q_nodes: Query<&mut Node, Without<HoverTooltip>>,
    mut q_tooltip_text: Query<&mut TextColor, Or<(With<TooltipHeader>, With<TooltipBody>)>>,
    mut q_tooltip_progress: Query<
        (&TooltipProgressBar, &mut BackgroundColor),
        Without<HoverTooltip>,
    >,
    q_ui_tooltips: Query<(Entity, &Interaction, &UiTooltip), With<Button>>,
    inspection: TooltipInspectionQueryParam,
    mut runtime: Local<TooltipRuntimeState>,
) {
    let Ok(window) = q_window.single() else {
        return;
    };
    let Some(tooltip_anchor) = ui_nodes.get_slot(UiSlot::TooltipAnchor) else {
        return;
    };
    let Ok((tooltip_entity, mut tooltip, mut tooltip_node, mut tooltip_bg, mut tooltip_border)) =
        q_tooltip.single_mut()
    else {
        return;
    };

    let hovered_button = q_ui_tooltips.iter().find(|(_, interaction, _)| {
        matches!(**interaction, Interaction::Hovered | Interaction::Pressed)
    });

    let mut target = None;
    let mut template = TooltipTemplate::Generic;
    let mut model: Option<EntityInspectionModel> = None;
    let mut ui_tooltip: Option<UiTooltip> = None;
    let mut payload = String::new();

    if let Some((button_entity, _, tooltip_data)) = hovered_button {
        target = Some(TooltipTarget::UiButton(button_entity));
        template = TooltipTemplate::UiButton;
        ui_tooltip = Some(UiTooltip {
            text: tooltip_data.text,
            shortcut: tooltip_data.shortcut,
        });
        payload = format!(
            "ui:{}:{}",
            tooltip_data.text,
            tooltip_data.shortcut.unwrap_or_default()
        );
    } else if let Some(entity) = hovered.0
        && let Some(built_model) = crate::interface::ui::presentation::build_entity_inspection_model(
            entity,
            &inspection.q_souls,
            &inspection.q_blueprints,
            &inspection.q_familiars,
            &inspection.q_familiars_escape,
            &inspection.familiar_grid,
            &inspection.q_items,
            &inspection.q_trees,
            &inspection.q_rocks,
            &inspection.q_designations,
            &inspection.q_buildings,
        )
    {
        template = classify_tooltip_template(
            entity,
            &inspection.q_souls,
            &inspection.q_blueprints,
            &inspection.q_items,
            &inspection.q_trees,
            &inspection.q_rocks,
            &inspection.q_buildings,
        );
        payload = format!(
            "entity:{entity:?}:{}:{}:{}",
            built_model.header,
            built_model.common_text,
            built_model.tooltip_lines.join("|"),
        );
        model = Some(built_model);
        target = Some(TooltipTarget::WorldEntity(entity));
    }

    let target_changed = runtime.target != target;
    let payload_changed = runtime.payload != payload;
    let template_changed = tooltip.template_type != template;

    if target_changed {
        runtime.target = target;
        match target {
            Some(TooltipTarget::UiButton(button_entity)) => {
                commands.entity(button_entity).add_child(tooltip_entity);
            }
            _ => {
                commands.entity(tooltip_anchor).add_child(tooltip_entity);
            }
        }
        tooltip.template_type = template;
        tooltip.delay_timer = Timer::from_seconds(0.3, TimerMode::Once);
        tooltip.delay_timer.reset();
        tooltip.fade_alpha = 0.0;
    }

    if payload_changed {
        runtime.payload = payload;
    }

    if target.is_some() && (target_changed || payload_changed || template_changed) {
        tooltip.template_type = template;
        tooltip_builder::rebuild_tooltip_content(
            &mut commands,
            tooltip_entity,
            &q_children,
            &game_assets,
            &theme,
            template,
            model.as_ref(),
            ui_tooltip.as_ref(),
        );
    }

    if target.is_some() {
        tooltip.delay_timer.tick(time.delta());
        if tooltip.delay_timer.is_finished() {
            tooltip.fade_alpha = (tooltip.fade_alpha + time.delta_secs() / 0.1).min(1.0);
            tooltip_node.display = Display::Flex;
        } else {
            tooltip_node.display = Display::None;
        }
    } else {
        tooltip.fade_alpha = (tooltip.fade_alpha - time.delta_secs() / 0.05).max(0.0);
        if tooltip.fade_alpha <= f32::EPSILON {
            tooltip_node.display = Display::None;
        } else {
            tooltip_node.display = Display::Flex;
        }
    }

    if let Some(cursor_pos) = window.cursor_position()
        && let Ok(mut anchor_node) = q_nodes.get_mut(tooltip_anchor)
    {
        anchor_node.left = Val::Px(cursor_pos.x);
        anchor_node.top = Val::Px(cursor_pos.y);
    }

    let bg = theme.colors.tooltip_bg.to_srgba();
    tooltip_bg.0 = Color::srgba(bg.red, bg.green, bg.blue, 0.95 * tooltip.fade_alpha);

    let border = theme.colors.tooltip_border.to_srgba();
    *tooltip_border = BorderColor::all(Color::srgba(
        border.red,
        border.green,
        border.blue,
        border.alpha * tooltip.fade_alpha,
    ));

    for mut text_color in q_tooltip_text.iter_mut() {
        let current = text_color.0.to_srgba();
        text_color.0 = Color::srgba(current.red, current.green, current.blue, tooltip.fade_alpha);
    }

    for (progress, mut color) in q_tooltip_progress.iter_mut() {
        let current = color.0.to_srgba();
        let base_alpha = (0.35 + 0.65 * progress.0).clamp(0.0, 1.0);
        color.0 = Color::srgba(
            current.red,
            current.green,
            current.blue,
            base_alpha * tooltip.fade_alpha,
        );
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
