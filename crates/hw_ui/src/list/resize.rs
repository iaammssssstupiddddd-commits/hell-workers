use super::minimize::EntityListMinimizeState;
use crate::components::{EntityListPanel, UiInputState, UiSlot};
use crate::theme::UiTheme;
use bevy::prelude::*;
use bevy::window::{CursorIcon, PrimaryWindow, SystemCursorIcon};

pub const ENTITY_LIST_DEFAULT_HEIGHT: f32 = 420.0;
pub const ENTITY_LIST_MIN_HEIGHT: f32 = 220.0;
const EDGE_DRAG_THRESHOLD_PX: f32 = 10.0;

type ModeLayoutQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static UiSlot,
        &'static Node,
        &'static ComputedNode,
        &'static UiGlobalTransform,
    ),
    Without<EntityListPanel>,
>;

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
    snap_step: f32,
    snap_anchor: f32,
}

impl EntityListResizeState {
    pub fn reset_active(&mut self) {
        self.active = false;
        self.edge = None;
    }

    pub fn is_active(&self) -> bool {
        self.active
    }
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

fn resolve_snap_anchor(min_height: f32, step: f32) -> f32 {
    min_height + step * 0.5
}

fn snap_panel_height_to_row_midpoint(
    height: f32,
    min_height: f32,
    max_height: f32,
    step: f32,
    anchor: f32,
) -> f32 {
    let min_rows = ((min_height - anchor) / step).ceil();
    let max_rows = ((max_height - anchor) / step).floor();
    if min_rows > max_rows {
        return clamp_height(height, min_height, max_height);
    }

    let desired_rows = ((height - anchor) / step).round().clamp(min_rows, max_rows);
    let snapped = anchor + desired_rows * step;
    clamp_height(snapped, min_height, max_height)
}

fn snap_panel_height_to_row_midpoint_floor(
    height: f32,
    min_height: f32,
    max_height: f32,
    step: f32,
    anchor: f32,
) -> f32 {
    let min_rows = ((min_height - anchor) / step).ceil();
    let max_rows = ((max_height - anchor) / step).floor();
    if min_rows > max_rows {
        return clamp_height(height, min_height, max_height);
    }

    let desired_rows = ((height - anchor) / step).floor().clamp(min_rows, max_rows);
    let snapped = anchor + desired_rows * step;
    clamp_height(snapped, min_height, max_height)
}

pub fn entity_list_resize_system(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    mut q_panel: Query<(&mut Node, &ComputedNode, &UiGlobalTransform), With<EntityListPanel>>,
    mut resize_state: ResMut<EntityListResizeState>,
    mut minimize_state: ResMut<EntityListMinimizeState>,
    (theme, ui_scale, mode_layout): (Res<UiTheme>, Res<UiScale>, ModeLayoutQuery),
    ui_input_state: Res<UiInputState>,
) {
    let Ok(window) = q_window.single() else {
        return;
    };
    let Ok((mut panel_node, computed, transform)) = q_panel.single_mut() else {
        return;
    };
    let scale = ui_scale.0.max(f32::EPSILON);
    let viewport_height = window.height() / scale;
    let mut bottom =
        viewport_height - theme.spacing.bottom_bar_height - theme.spacing.panel_margin_x;
    if let Some((_, _, mode_size, mode_transform)) =
        mode_layout.iter().find(|(slot, node, size, _)| {
            **slot == UiSlot::ModeText && node.display != Display::None && size.size().y > 0.0
        })
    {
        let mode_top = (mode_transform.translation.y - mode_size.size().y * 0.5)
            * mode_size.inverse_scale_factor();
        bottom = bottom.min(mode_top - theme.spacing.panel_margin_x);
    }
    let top = match panel_node.top {
        Val::Px(value) => value,
        _ => theme.spacing.panel_top,
    }
    .clamp(
        theme.spacing.panel_margin_x,
        (bottom - ENTITY_LIST_MIN_HEIGHT).max(theme.spacing.panel_margin_x),
    );
    let max_height = (viewport_height * theme.sizes.entity_list_max_height_percent / 100.0)
        .min(bottom - top)
        .max(1.0);
    let min_height = ENTITY_LIST_MIN_HEIGHT.min(max_height);
    let height = minimize_state.expanded_height.clamp(min_height, max_height);
    if panel_node.top != Val::Px(top) || minimize_state.expanded_height != height {
        resize_state.reset_active();
        panel_node.top = Val::Px(top);
        minimize_state.expanded_height = height;
    }
    if !minimize_state.minimized {
        if panel_node.height != Val::Px(height) {
            panel_node.height = Val::Px(height);
        }
        if panel_node.min_height != Val::Px(min_height) {
            panel_node.min_height = Val::Px(min_height);
        }
    }
    if ui_input_state.world_input_captured {
        resize_state.reset_active();
        return;
    }
    if minimize_state.minimized {
        resize_state.active = false;
        resize_state.edge = None;
        return;
    }

    let Some(cursor) = window.cursor_position() else {
        if !mouse_buttons.pressed(MouseButton::Left) {
            resize_state.active = false;
            resize_state.edge = None;
        }
        return;
    };
    let cursor = cursor / scale;

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
                resize_state.snap_step = theme.sizes.soul_item_height.max(1.0);
                resize_state.snap_anchor =
                    resolve_snap_anchor(ENTITY_LIST_MIN_HEIGHT, resize_state.snap_step);
            }
        }
        return;
    }

    if !mouse_buttons.pressed(MouseButton::Left) {
        resize_state.active = false;
        resize_state.edge = None;
        return;
    }

    let delta_y = cursor.y - resize_state.start_cursor_y;
    let snap_step = resize_state.snap_step.max(1.0);
    let snap_anchor = resize_state.snap_anchor;

    match resize_state.edge {
        Some(ResizeEdge::Bottom) => {
            let desired_height = resize_state.start_height + delta_y;
            let snapped_height = snap_panel_height_to_row_midpoint(
                desired_height,
                min_height,
                max_height,
                snap_step,
                snap_anchor,
            );
            panel_node.height = Val::Px(snapped_height);
            minimize_state.expanded_height = snapped_height;
        }
        Some(ResizeEdge::Top) => {
            let start_bottom = resize_state.start_top + resize_state.start_height;
            let desired_height = resize_state.start_height - delta_y;
            let mut snapped_height = snap_panel_height_to_row_midpoint(
                desired_height,
                min_height,
                max_height,
                snap_step,
                snap_anchor,
            );
            let max_height_by_top = start_bottom - theme.spacing.panel_margin_x;
            if snapped_height > max_height_by_top {
                snapped_height = snap_panel_height_to_row_midpoint_floor(
                    max_height_by_top,
                    min_height,
                    max_height,
                    snap_step,
                    snap_anchor,
                );
            }
            let clamped_height = clamp_height(snapped_height, min_height, max_height);
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
    (ui_input_state, ui_scale): (Res<UiInputState>, Res<UiScale>),
    mut q_cursor: Query<&mut CursorIcon, With<PrimaryWindow>>,
    mut commands: Commands,
) {
    let Ok((window_entity, window)) = q_window.single() else {
        return;
    };
    let Ok((computed, transform)) = q_panel.single() else {
        return;
    };

    let desired = if !ui_input_state.world_input_captured
        && !minimize_state.minimized
        && (resize_state.active
            || window.cursor_position().is_some_and(|cursor| {
                is_cursor_on_vertical_resize_edge(
                    cursor / ui_scale.0.max(f32::EPSILON),
                    computed,
                    transform,
                )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_change_clamps_expanded_height_even_with_capture_and_no_cursor() {
        let mut app = App::new();
        app.init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<EntityListResizeState>()
            .init_resource::<EntityListMinimizeState>()
            .init_resource::<UiTheme>()
            .insert_resource(UiScale(1.25))
            .insert_resource(UiInputState {
                world_input_captured: true,
                ..default()
            })
            .add_systems(Update, entity_list_resize_system);
        app.world_mut().spawn((
            Window {
                resolution: (1280, 720).into(),
                ..default()
            },
            PrimaryWindow,
        ));
        let panel = app
            .world_mut()
            .spawn((
                EntityListPanel,
                Node {
                    top: Val::Px(170.0),
                    height: Val::Px(420.0),
                    ..default()
                },
            ))
            .id();
        app.update();
        let theme = app.world().resource::<UiTheme>();
        let expected =
            720.0 / 1.25 - theme.spacing.bottom_bar_height - theme.spacing.panel_margin_x - 170.0;
        assert_eq!(
            app.world().get::<Node>(panel).unwrap().height,
            Val::Px(expected)
        );
        assert_eq!(
            app.world()
                .resource::<EntityListMinimizeState>()
                .expanded_height,
            expected
        );
        let reserved_height = 440.0 - theme.spacing.panel_margin_x - 170.0;
        app.world_mut().spawn((
            UiSlot::ModeText,
            Node::default(),
            ComputedNode {
                size: Vec2::new(1000.0, 100.0),
                inverse_scale_factor: 0.8,
                ..default()
            },
            UiGlobalTransform::from_translation(Vec2::new(500.0, 600.0)),
        ));
        app.update();
        assert_eq!(
            app.world().get::<Node>(panel).unwrap().height,
            Val::Px(reserved_height)
        );
        app.world_mut()
            .resource_mut::<EntityListMinimizeState>()
            .minimized = true;
        app.world_mut().get_mut::<Node>(panel).unwrap().height = Val::Px(44.0);
        app.update();
        assert_eq!(
            app.world().get::<Node>(panel).unwrap().height,
            Val::Px(44.0)
        );
    }

    #[test]
    fn capture_reset_clears_active_resize_edge() {
        let mut state = EntityListResizeState {
            active: true,
            edge: Some(ResizeEdge::Top),
            ..default()
        };

        state.reset_active();

        assert!(!state.is_active());
        assert!(state.edge.is_none());
    }
}
