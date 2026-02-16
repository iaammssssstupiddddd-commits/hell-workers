//! ツールチップのレイアウト・Popover位置

use crate::interface::ui::components::*;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::ui_widgets::popover::{Popover, PopoverAlign, PopoverPlacement, PopoverSide};

use super::target::TooltipTarget;

#[derive(SystemParam)]
pub(crate) struct TooltipUiLayoutQueryParam<'w, 's> {
    pub q_ui_tooltip_buttons: Query<
        'w,
        's,
        (
            Entity,
            &'static Interaction,
            &'static UiTooltip,
            Option<&'static MenuButton>,
            &'static ComputedNode,
            &'static UiGlobalTransform,
        ),
        With<Button>,
    >,
    pub q_speed_buttons: Query<'w, 's, (), With<SpeedButtonMarker>>,
    pub q_layout: Query<'w, 's, (&'static ComputedNode, &'static UiGlobalTransform)>,
    pub q_architect_submenu:
        Query<'w, 's, (&'static ComputedNode, &'static UiGlobalTransform), With<ArchitectSubMenu>>,
    pub q_zones_submenu:
        Query<'w, 's, (&'static ComputedNode, &'static UiGlobalTransform), With<ZonesSubMenu>>,
    pub q_orders_submenu:
        Query<'w, 's, (&'static ComputedNode, &'static UiGlobalTransform), With<OrdersSubMenu>>,
}

pub(crate) fn compute_rect_x(computed: &ComputedNode, transform: &UiGlobalTransform) -> (f32, f32) {
    let inverse_scale = computed.inverse_scale_factor();
    let center_x = transform.translation.x * inverse_scale;
    let half_w = computed.size().x * inverse_scale * 0.5;
    (center_x - half_w, center_x + half_w)
}

pub(crate) fn compute_rect_y(computed: &ComputedNode, transform: &UiGlobalTransform) -> (f32, f32) {
    let inverse_scale = computed.inverse_scale_factor();
    let center_y = transform.translation.y * inverse_scale;
    let half_h = computed.size().y * inverse_scale * 0.5;
    (center_y - half_h, center_y + half_h)
}

fn overlap_len(a: (f32, f32), b: (f32, f32)) -> f32 {
    (a.1.min(b.1) - a.0.max(b.0)).max(0.0)
}

fn is_menu_toggle_action(action: MenuAction) -> bool {
    matches!(
        action,
        MenuAction::ToggleArchitect | MenuAction::ToggleZones | MenuAction::ToggleOrders
    )
}

pub(crate) fn resolve_toggle_span_x(
    q_ui_tooltip_buttons: &Query<
        (
            Entity,
            &Interaction,
            &UiTooltip,
            Option<&MenuButton>,
            &ComputedNode,
            &UiGlobalTransform,
        ),
        With<Button>,
    >,
) -> Option<(f32, f32)> {
    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut found = false;

    for (_, _, _, menu_button, computed, transform) in q_ui_tooltip_buttons.iter() {
        let Some(menu_button) = menu_button else {
            continue;
        };
        if !is_menu_toggle_action(menu_button.0) {
            continue;
        }
        let (left_x, right_x) = compute_rect_x(computed, transform);
        min_x = min_x.min(left_x);
        max_x = max_x.max(right_x);
        found = true;
    }

    if found { Some((min_x, max_x)) } else { None }
}

pub(crate) fn resolve_mode_text_span_x(
    ui_nodes: &UiNodeRegistry,
    q_layout: &Query<(&ComputedNode, &UiGlobalTransform)>,
) -> Option<(f32, f32)> {
    let entity = ui_nodes.get_slot(UiSlot::ModeText)?;
    let (computed, transform) = q_layout.get(entity).ok()?;
    Some(compute_rect_x(computed, transform))
}

pub(crate) fn resolve_visible_submenu_spans_x(
    ui_layout: &TooltipUiLayoutQueryParam,
    menu_state: MenuState,
) -> Vec<(f32, f32)> {
    let mut spans = Vec::with_capacity(3);

    match menu_state {
        MenuState::Architect => {
            if let Ok((computed, transform)) = ui_layout.q_architect_submenu.single() {
                spans.push(compute_rect_x(computed, transform));
            }
        }
        MenuState::Zones => {
            if let Ok((computed, transform)) = ui_layout.q_zones_submenu.single() {
                spans.push(compute_rect_x(computed, transform));
            }
        }
        MenuState::Orders => {
            if let Ok((computed, transform)) = ui_layout.q_orders_submenu.single() {
                spans.push(compute_rect_x(computed, transform));
            }
        }
        MenuState::Hidden => {}
    }

    spans
}

fn sum_overlap_len(span: (f32, f32), blocked_spans: &[(f32, f32)]) -> f32 {
    blocked_spans
        .iter()
        .map(|blocked| overlap_len(span, *blocked))
        .sum()
}

pub(crate) fn resolve_expanded_toggle_tooltip_position(
    button_x_span: (f32, f32),
    button_y_span: (f32, f32),
    tooltip_size: Vec2,
    window_size: Vec2,
    mode_x_span: Option<(f32, f32)>,
    toggle_span: Option<(f32, f32)>,
    submenu_spans: &[(f32, f32)],
) -> (f32, f32) {
    let gap = 8.0;
    let tooltip_width = tooltip_size.x.max(1.0);
    let tooltip_height = tooltip_size.y.max(1.0);

    let left_bounds = (button_x_span.0 - gap - tooltip_width, button_x_span.0 - gap);
    let right_bounds = (button_x_span.1 + gap, button_x_span.1 + gap + tooltip_width);

    let left_mode_overlap = mode_x_span.map_or(0.0, |span| overlap_len(left_bounds, span));
    let right_mode_overlap = mode_x_span.map_or(0.0, |span| overlap_len(right_bounds, span));
    let left_toggle_overlap = toggle_span.map_or(0.0, |span| overlap_len(left_bounds, span));
    let right_toggle_overlap = toggle_span.map_or(0.0, |span| overlap_len(right_bounds, span));
    let left_submenu_overlap = sum_overlap_len(left_bounds, submenu_spans);
    let right_submenu_overlap = sum_overlap_len(right_bounds, submenu_spans);

    let left_score = left_mode_overlap * 24.0 + left_submenu_overlap * 8.0 + left_toggle_overlap;
    let right_score =
        right_mode_overlap * 24.0 + right_submenu_overlap * 8.0 + right_toggle_overlap;

    let prefer_left = if (left_score - right_score).abs() < 1.0 {
        let center_x = (button_x_span.0 + button_x_span.1) * 0.5;
        let toggle_mid_x = toggle_span
            .map(|(min_x, max_x)| (min_x + max_x) * 0.5)
            .unwrap_or(center_x);
        center_x >= toggle_mid_x
    } else {
        left_score < right_score
    };

    let raw_x = if prefer_left {
        button_x_span.0 - gap - tooltip_width
    } else {
        button_x_span.1 + gap
    };
    let raw_y = button_y_span.0 - gap - tooltip_height;

    let clamped_x = raw_x.clamp(8.0, (window_size.x - tooltip_width - 8.0).max(8.0));
    let clamped_y = raw_y.clamp(8.0, (window_size.y - tooltip_height - 8.0).max(8.0));

    (clamped_x, clamped_y)
}

pub(crate) fn update_tooltip_popover_positions(
    popover: &mut Popover,
    target: Option<TooltipTarget>,
    menu_state: MenuState,
    _hovered_menu_action: Option<MenuAction>,
) {
    let desired_positions = if matches!(target, Some(TooltipTarget::UiButton(_)))
        && !matches!(menu_state, MenuState::Hidden)
    {
        vec![
            PopoverPlacement {
                side: PopoverSide::Right,
                align: PopoverAlign::Start,
                gap: 8.0,
            },
            PopoverPlacement {
                side: PopoverSide::Left,
                align: PopoverAlign::Start,
                gap: 8.0,
            },
        ]
    } else {
        vec![
            PopoverPlacement {
                side: PopoverSide::Bottom,
                align: PopoverAlign::Start,
                gap: 6.0,
            },
            PopoverPlacement {
                side: PopoverSide::Top,
                align: PopoverAlign::Start,
                gap: 6.0,
            },
        ]
    };

    if popover.positions != desired_positions {
        popover.positions = desired_positions;
    }
}
