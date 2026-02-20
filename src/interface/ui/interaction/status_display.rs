use super::mode;
use crate::constants::{
    DREAM_UI_PULSE_BRIGHTNESS, DREAM_UI_PULSE_DURATION, DREAM_UI_PULSE_TRIGGER_DELTA, TILE_SIZE,
};
use crate::entities::familiar::Familiar;
use crate::game_state::{
    BuildContext, CompanionPlacementState, PlayMode, TaskContext, ZoneContext,
};
use crate::interface::selection::SelectedEntity;
use crate::interface::ui::components::*;
use crate::interface::ui::theme::UiTheme;
use crate::relationships::ManagedBy;
use crate::systems::command::{
    AreaEditClipboard, AreaEditSession, TaskArea, count_positions_in_area,
    overlap_summary_from_areas,
};
use crate::systems::jobs::Designation;
use bevy::math::TryStableInterpolate;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use std::time::Duration;

pub fn update_mode_text_system(
    play_mode: Res<State<PlayMode>>,
    build_context: Res<BuildContext>,
    companion_state: Res<CompanionPlacementState>,
    zone_context: Res<ZoneContext>,
    task_context: Res<TaskContext>,
    selected_entity: Res<SelectedEntity>,
    area_edit_session: Res<AreaEditSession>,
    area_edit_clipboard: Res<AreaEditClipboard>,
    ui_nodes: Res<UiNodeRegistry>,
    q_familiars: Query<&Familiar>,
    q_task_areas: Query<(Entity, Ref<TaskArea>), With<Familiar>>,
    q_unassigned_tasks: Query<&Transform, (With<Designation>, Without<ManagedBy>)>,
    mut q_text: Query<&mut Text>,
) {
    let area_mode_active = matches!(
        task_context.0,
        crate::systems::command::TaskMode::AreaSelection(_)
    );
    let selected_area_changed = selected_entity.0.is_some_and(|selected| {
        q_task_areas
            .iter()
            .find(|(entity, _)| *entity == selected)
            .is_some_and(|(_, area)| area.is_changed())
    });

    if !play_mode.is_changed()
        && !build_context.is_changed()
        && !companion_state.is_changed()
        && !zone_context.is_changed()
        && !task_context.is_changed()
        && !selected_entity.is_changed()
        && !area_edit_session.is_changed()
        && !area_edit_clipboard.is_changed()
        && !selected_area_changed
        && !area_mode_active
    {
        return;
    }
    let Some(entity) = ui_nodes.get_slot(UiSlot::ModeText) else {
        return;
    };
    if let Ok(mut text) = q_text.get_mut(entity) {
        let selected_familiar_name = selected_entity
            .0
            .and_then(|entity| q_familiars.get(entity).ok())
            .map(|familiar| familiar.name.as_str());
        let selected_area = selected_entity.0.and_then(|selected| {
            q_task_areas
                .iter()
                .find(|(entity, _)| *entity == selected)
                .map(|(_, area)| (*area).clone())
        });
        let selected_area_size_tiles = selected_area.as_ref().map(|area| {
            let size = area.size();
            UVec2::new(
                (size.x.abs() / TILE_SIZE).round().max(1.0) as u32,
                (size.y.abs() / TILE_SIZE).round().max(1.0) as u32,
            )
        });
        let area_overlap = selected_entity.0.and_then(|selected| {
            selected_area.as_ref().and_then(|selected_area| {
                overlap_summary_from_areas(
                    selected,
                    selected_area,
                    q_task_areas
                        .iter()
                        .map(|(entity, area)| (entity, (*area).clone())),
                )
            })
        });
        let unassigned_tasks_in_area = selected_area.as_ref().map(|area| {
            count_positions_in_area(
                area,
                q_unassigned_tasks
                    .iter()
                    .map(|transform| transform.translation.truncate()),
            )
        });

        text.0 = mode::build_mode_text(
            play_mode.get(),
            &build_context,
            &companion_state,
            &zone_context,
            &task_context,
            selected_familiar_name,
            selected_area_size_tiles,
            area_edit_session.is_dragging(),
            area_edit_session.operation_label(),
            area_overlap,
            area_edit_clipboard.has_area(),
            unassigned_tasks_in_area,
        );
    }
}

pub fn task_summary_ui_system(
    q_designations: Query<&crate::systems::jobs::Priority, With<crate::systems::jobs::Designation>>,
    theme: Res<crate::interface::ui::theme::UiTheme>,
    ui_nodes: Res<UiNodeRegistry>,
    mut q_text: Query<(&mut Text, &mut TextColor)>,
) {
    let Some(entity) = ui_nodes.get_slot(UiSlot::TaskSummaryText) else {
        return;
    };
    if let Ok((mut text, mut color)) = q_text.get_mut(entity) {
        let total = q_designations.iter().count();
        let high = q_designations.iter().filter(|p| p.0 > 0).count();
        text.0 = format!("Tasks: {} ({} High)", total, high);
        // High task warning color
        if high > 0 {
            color.0 = theme.colors.task_high_warning;
        } else {
            color.0 = theme.colors.panel_accent_time_control;
        }
    }
}

pub fn update_area_edit_preview_ui_system(
    task_context: Res<TaskContext>,
    selected_entity: Res<SelectedEntity>,
    area_edit_session: Res<AreaEditSession>,
    area_edit_clipboard: Res<AreaEditClipboard>,
    ui_nodes: Res<UiNodeRegistry>,
    q_task_areas: Query<(Entity, &TaskArea), With<Familiar>>,
    q_unassigned_tasks: Query<&Transform, (With<Designation>, Without<ManagedBy>)>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    mut q_node: Query<&mut Node>,
    mut q_text: Query<&mut Text>,
) {
    let Some(preview_entity) = ui_nodes.get_slot(UiSlot::AreaEditPreview) else {
        return;
    };
    let Ok(mut node) = q_node.get_mut(preview_entity) else {
        return;
    };
    let Ok(mut text) = q_text.get_mut(preview_entity) else {
        return;
    };

    if !matches!(
        task_context.0,
        crate::systems::command::TaskMode::AreaSelection(_)
    ) {
        node.display = Display::None;
        return;
    }

    let Some(selected) = selected_entity.0 else {
        node.display = Display::None;
        return;
    };
    let Some(area) = q_task_areas
        .iter()
        .find(|(entity, _)| *entity == selected)
        .map(|(_, area)| area)
    else {
        node.display = Display::None;
        return;
    };
    let Ok(window) = q_window.single() else {
        node.display = Display::None;
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        node.display = Display::None;
        return;
    };

    let size = area.size();
    let width_tiles = (size.x.abs() / TILE_SIZE).round().max(1.0) as i32;
    let height_tiles = (size.y.abs() / TILE_SIZE).round().max(1.0) as i32;

    let state = if area_edit_session.is_dragging() {
        if let Some(op) = area_edit_session.operation_label() {
            format!("Dragging {}", op)
        } else {
            "Dragging".to_string()
        }
    } else {
        "Ready".to_string()
    };

    let overlap = overlap_summary_from_areas(
        selected,
        area,
        q_task_areas
            .iter()
            .map(|(entity, area)| (entity, area.clone())),
    );
    let overlap_text = if let Some((count, ratio)) = overlap {
        if count > 0 {
            format!("Overlap:{} ({:.0}%)", count, ratio * 100.0)
        } else {
            "Overlap:0".to_string()
        }
    } else {
        "Overlap:-".to_string()
    };
    let clip_text = if area_edit_clipboard.has_area() {
        "Clip:Ready"
    } else {
        "Clip:Empty"
    };
    let tasks_in_area = count_positions_in_area(
        area,
        q_unassigned_tasks
            .iter()
            .map(|transform| transform.translation.truncate()),
    );
    let warn_text = if overlap.is_some_and(|(count, ratio)| count > 0 && ratio >= 0.5) {
        " | WARN:HighOverlap"
    } else {
        ""
    };

    text.0 = format!(
        "Area {}x{}t | {} | {} | Tasks:{} | {}{}",
        width_tiles, height_tiles, state, overlap_text, tasks_in_area, clip_text, warn_text
    );
    node.display = Display::Flex;
    node.left = Val::Px((cursor.x + 14.0).min(window.width() - 360.0).max(4.0));
    node.top = Val::Px((cursor.y + 18.0).min(window.height() - 34.0).max(4.0));
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

pub fn update_speed_button_highlight_system(
    time: Res<Time<Virtual>>,
    theme: Res<crate::interface::ui::theme::UiTheme>,
    mut q_buttons: Query<(
        &crate::interface::ui::components::SpeedButtonMarker,
        &mut BackgroundColor,
        &mut BorderColor,
    )>,
) {
    let current_speed = if time.is_paused() {
        crate::systems::time::TimeSpeed::Paused
    } else {
        let speed = time.relative_speed();
        if speed <= 1.0 {
            crate::systems::time::TimeSpeed::Normal
        } else if speed <= 2.0 {
            crate::systems::time::TimeSpeed::Fast
        } else {
            crate::systems::time::TimeSpeed::Super
        }
    };

    for (marker, mut bg, mut border) in q_buttons.iter_mut() {
        if marker.0 == current_speed {
            bg.0 = theme.colors.speed_button_active;
            *border = BorderColor::all(theme.colors.accent_ember);
        } else {
            bg.0 = theme.colors.button_default;
            *border = BorderColor::all(Color::NONE);
        }
    }
}

#[derive(Component)]
pub struct DreamLossPopupUi {
    pub lifetime: f32,
    pub max_lifetime: f32,
    pub start_y: f32,
}

pub fn update_dream_loss_popup_ui_system(
    mut commands: Commands,
    time: Res<Time>,
    mut q_popups: Query<(Entity, &mut DreamLossPopupUi, &mut Node, &mut TextColor)>,
) {
    let dt = time.delta_secs();
    for (entity, mut popup, mut node, mut text_color) in q_popups.iter_mut() {
        popup.lifetime -= dt;
        if popup.lifetime <= 0.0 {
            commands.entity(entity).try_despawn();
            continue;
        }
        let progress = 1.0 - (popup.lifetime / popup.max_lifetime).clamp(0.0, 1.0);
        node.top = Val::Px(popup.start_y - progress * 25.0);
        let mut color = text_color.0.to_srgba();
        color.alpha = 1.0 - progress;
        text_color.0 = color.into();
    }
}

pub fn update_dream_pool_display_system(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<crate::assets::GameAssets>,
    dream_pool: Res<crate::entities::damned_soul::DreamPool>,
    theme: Res<UiTheme>,
    ui_nodes: Res<UiNodeRegistry>,
    mut q_text: Query<(&mut Text, &mut TextColor, &mut DreamPoolPulse)>,
) {
    let Some(entity) = ui_nodes.get_slot(UiSlot::DreamPoolText) else {
        return;
    };
    if let Ok((mut text, mut text_color, mut pulse)) = q_text.get_mut(entity) {
        if dream_pool.is_changed() {
            text.0 = format!("Dream: {:.1}", dream_pool.points);
        }

        pulse.timer = (pulse.timer - time.delta_secs()).max(0.0);

        let delta = dream_pool.points - pulse.last_points;
        if delta > 0.0 {
            pulse.pending_gain += delta;
            while pulse.pending_gain >= DREAM_UI_PULSE_TRIGGER_DELTA {
                pulse.pending_gain -= DREAM_UI_PULSE_TRIGGER_DELTA;
                pulse.timer = DREAM_UI_PULSE_DURATION;
            }
        } else if delta < -0.1 {
            // 消費時はアイコンから上に浮かび上がるテキストを発生させる
            if let Some(icon_entity) = ui_nodes.get_slot(UiSlot::DreamPoolIcon) {
                let popup = commands.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(-30.0),
                        top: Val::Px(0.0),
                        ..default()
                    },
                    Text::new(format!("{:.1}", delta)),
                    TextFont {
                        font: assets.font_ui.clone(),
                        font_size: theme.typography.font_size_clock,
                        ..default()
                    },
                    TextColor(theme.colors.task_high_warning),
                    DreamLossPopupUi {
                        lifetime: 1.5,
                        max_lifetime: 1.5,
                        start_y: 0.0,
                    },
                    GlobalZIndex(10050),
                    Name::new("DreamLossPopup"),
                )).id();
                commands.entity(icon_entity).add_child(popup);
            }
        }
        pulse.last_points = dream_pool.points;

        let base_color = theme.colors.accent_soul_bright;
        // プラスのパルス（白・発光）
        if pulse.timer > 0.0 {
            let progress = 1.0 - (pulse.timer / DREAM_UI_PULSE_DURATION).clamp(0.0, 1.0);
            let pulse_alpha =
                (progress * std::f32::consts::PI).sin().max(0.0) * DREAM_UI_PULSE_BRIGHTNESS;
            let bright_color = Color::WHITE;
            text_color.0 = base_color
                .try_interpolate_stable(&bright_color, pulse_alpha)
                .unwrap_or(bright_color);
        } else {
            text_color.0 = base_color;
        }
    }
}
