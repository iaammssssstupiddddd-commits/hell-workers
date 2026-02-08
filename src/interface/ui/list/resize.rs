use super::minimize::EntityListMinimizeState;
use crate::interface::ui::components::EntityListPanel;
use crate::interface::ui::theme::UiTheme;
use bevy::prelude::*;
use bevy::window::{CursorIcon, PrimaryWindow, SystemCursorIcon};

pub const ENTITY_LIST_DEFAULT_HEIGHT: f32 = 420.0;
pub const ENTITY_LIST_MIN_HEIGHT: f32 = 220.0;
const EDGE_DRAG_THRESHOLD_PX: f32 = 10.0;
const ENTITY_LIST_HEADER_MARGIN_BOTTOM: f32 = 10.0;
const ENTITY_LIST_ROW_SNAP_Y_OFFSET_ADJUST: f32 = 7.0;

#[derive(Clone, Copy)]
enum ResizeEdge {
    Top,
    Bottom,
}

#[derive(Resource, Default)]
pub struct EntityListResizeState {
    active: bool,
    edge: Option<ResizeEdge>,
    start_cursor_y: f32,
    start_height: f32,
    start_top: f32,
}

fn is_cursor_on_vertical_resize_edge(
    cursor: Vec2,
    computed: &ComputedNode,
    transform: &UiGlobalTransform,
) -> bool {
    let inverse_scale = computed.inverse_scale_factor();
    let size = computed.size() * inverse_scale;
    let center = transform.translation * inverse_scale;
    let left = center.x - size.x * 0.5;
    let right = center.x + size.x * 0.5;
    let top = center.y - size.y * 0.5;
    let bottom = center.y + size.y * 0.5;
    let cursor_over_x = cursor.x >= left && cursor.x <= right;
    let cursor_over_y = cursor.y >= top && cursor.y <= bottom;
    if !(cursor_over_x && cursor_over_y) {
        return false;
    }
    let dist_top = (cursor.y - top).abs();
    let dist_bottom = (bottom - cursor.y).abs();
    dist_top <= EDGE_DRAG_THRESHOLD_PX || dist_bottom <= EDGE_DRAG_THRESHOLD_PX
}

fn clamp_height(height: f32, min_height: f32, max_height: f32) -> f32 {
    height.clamp(min_height, max_height)
}

fn panel_row_snap_offset(theme: &UiTheme) -> f32 {
    theme.spacing.panel_padding * 2.0
        + theme.sizes.header_height
        + ENTITY_LIST_HEADER_MARGIN_BOTTOM
        + theme.sizes.panel_border_width * 2.0
        + ENTITY_LIST_ROW_SNAP_Y_OFFSET_ADJUST
}

fn snap_panel_height_to_row_grid(
    height: f32,
    min_height: f32,
    max_height: f32,
    theme: &UiTheme,
) -> f32 {
    let step = theme.sizes.soul_item_height.max(1.0);
    let offset = panel_row_snap_offset(theme);

    let min_rows = ((min_height - offset) / step).ceil();
    let max_rows = ((max_height - offset) / step).floor();
    if min_rows > max_rows {
        return clamp_height(height, min_height, max_height);
    }

    let desired_rows = ((height - offset) / step).round().clamp(min_rows, max_rows);
    let snapped = offset + desired_rows * step;
    clamp_height(snapped, min_height, max_height)
}

fn snap_panel_height_to_row_grid_floor(
    height: f32,
    min_height: f32,
    max_height: f32,
    theme: &UiTheme,
) -> f32 {
    let step = theme.sizes.soul_item_height.max(1.0);
    let offset = panel_row_snap_offset(theme);

    let min_rows = ((min_height - offset) / step).ceil();
    let max_rows = ((max_height - offset) / step).floor();
    if min_rows > max_rows {
        return clamp_height(height, min_height, max_height);
    }

    let desired_rows = ((height - offset) / step).floor().clamp(min_rows, max_rows);
    let snapped = offset + desired_rows * step;
    clamp_height(snapped, min_height, max_height)
}

pub fn entity_list_resize_system(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    mut q_panel: Query<(&mut Node, &ComputedNode, &UiGlobalTransform), With<EntityListPanel>>,
    mut resize_state: ResMut<EntityListResizeState>,
    mut minimize_state: ResMut<EntityListMinimizeState>,
    theme: Res<UiTheme>,
) {
    if minimize_state.minimized {
        resize_state.active = false;
        resize_state.edge = None;
        return;
    }

    let Ok(window) = q_window.single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        if !mouse_buttons.pressed(MouseButton::Left) {
            resize_state.active = false;
            resize_state.edge = None;
        }
        return;
    };
    let Ok((mut panel_node, computed, transform)) = q_panel.single_mut() else {
        return;
    };

    if !resize_state.active {
        if mouse_buttons.just_pressed(MouseButton::Left)
            && is_cursor_on_vertical_resize_edge(cursor, computed, transform)
        {
            let inverse_scale = computed.inverse_scale_factor();
            let size = computed.size() * inverse_scale;
            let center = transform.translation * inverse_scale;
            let top = center.y - size.y * 0.5;
            let bottom = center.y + size.y * 0.5;
            let dist_top = (cursor.y - top).abs();
            let dist_bottom = (bottom - cursor.y).abs();
            let edge =
                if dist_top <= EDGE_DRAG_THRESHOLD_PX || dist_bottom <= EDGE_DRAG_THRESHOLD_PX {
                    if dist_top <= dist_bottom {
                        Some(ResizeEdge::Top)
                    } else {
                        Some(ResizeEdge::Bottom)
                    }
                } else {
                    None
                };

            if let Some(edge) = edge {
                resize_state.active = true;
                resize_state.edge = Some(edge);
                resize_state.start_cursor_y = cursor.y;
                resize_state.start_height = match panel_node.height {
                    Val::Px(height) => height,
                    _ => ENTITY_LIST_DEFAULT_HEIGHT,
                };
                resize_state.start_top = match panel_node.top {
                    Val::Px(top_px) => top_px,
                    _ => theme.spacing.panel_top,
                };
            }
        }
        return;
    }

    if !mouse_buttons.pressed(MouseButton::Left) {
        resize_state.active = false;
        resize_state.edge = None;
        return;
    }

    let max_height_percent = window.height() * (theme.sizes.entity_list_max_height_percent / 100.0);
    let max_height_layout =
        window.height() - theme.spacing.bottom_bar_height - theme.spacing.panel_margin_x;
    let max_height = max_height_percent
        .min(max_height_layout)
        .max(ENTITY_LIST_MIN_HEIGHT);
    let delta_y = cursor.y - resize_state.start_cursor_y;

    match resize_state.edge {
        Some(ResizeEdge::Bottom) => {
            let desired_height = resize_state.start_height + delta_y;
            let snapped_height = snap_panel_height_to_row_grid(
                desired_height,
                ENTITY_LIST_MIN_HEIGHT,
                max_height,
                &theme,
            );
            panel_node.height = Val::Px(snapped_height);
            minimize_state.expanded_height = snapped_height;
        }
        Some(ResizeEdge::Top) => {
            let start_bottom = resize_state.start_top + resize_state.start_height;
            let desired_height = resize_state.start_height - delta_y;
            let mut snapped_height = snap_panel_height_to_row_grid(
                desired_height,
                ENTITY_LIST_MIN_HEIGHT,
                max_height,
                &theme,
            );
            let max_height_by_top = start_bottom - theme.spacing.panel_margin_x;
            if snapped_height > max_height_by_top {
                snapped_height = snap_panel_height_to_row_grid_floor(
                    max_height_by_top,
                    ENTITY_LIST_MIN_HEIGHT,
                    max_height,
                    &theme,
                );
            }
            let clamped_height = clamp_height(snapped_height, ENTITY_LIST_MIN_HEIGHT, max_height);
            let clamped_top = (start_bottom - clamped_height).max(theme.spacing.panel_margin_x);
            panel_node.top = Val::Px(clamped_top);
            panel_node.height = Val::Px(clamped_height);
            minimize_state.expanded_height = clamped_height;
        }
        None => {}
    }
}

pub fn entity_list_resize_cursor_system(
    q_window: Query<(Entity, &Window), With<PrimaryWindow>>,
    q_panel: Query<(&ComputedNode, &UiGlobalTransform), With<EntityListPanel>>,
    resize_state: Res<EntityListResizeState>,
    minimize_state: Res<EntityListMinimizeState>,
    mut q_cursor: Query<&mut CursorIcon, With<PrimaryWindow>>,
    mut commands: Commands,
) {
    let Ok((window_entity, window)) = q_window.single() else {
        return;
    };
    let Ok((computed, transform)) = q_panel.single() else {
        return;
    };

    let desired = if !minimize_state.minimized
        && (resize_state.active
            || window.cursor_position().is_some_and(|cursor| {
                is_cursor_on_vertical_resize_edge(cursor, computed, transform)
            })) {
        CursorIcon::System(SystemCursorIcon::NsResize)
    } else {
        CursorIcon::System(SystemCursorIcon::Default)
    };

    if let Ok(mut icon) = q_cursor.get_mut(window_entity) {
        if *icon != desired {
            *icon = desired;
        }
    } else {
        commands.entity(window_entity).insert(desired);
    }
}
