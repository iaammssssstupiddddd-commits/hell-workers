use crate::interface::ui::components::*;
use crate::interface::ui::panels::tooltip_builder;
use crate::interface::ui::presentation::{EntityInspectionModel, EntityInspectionQuery};
use crate::interface::ui::theme::UiTheme;
use bevy::ecs::system::SystemParam;
use bevy::math::TryStableInterpolate;
use bevy::prelude::*;
use bevy::ui_widgets::popover::{Popover, PopoverAlign, PopoverPlacement, PopoverSide};

#[derive(Default)]
pub(crate) struct TooltipRuntimeState {
    target: Option<TooltipTarget>,
    payload: String,
    attach_to_anchor: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TooltipTarget {
    UiButton(Entity),
    WorldEntity(Entity),
}

#[derive(SystemParam)]
pub(crate) struct TooltipUiLayoutQueryParam<'w, 's> {
    q_ui_tooltip_buttons: Query<
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
    q_layout: Query<'w, 's, (&'static ComputedNode, &'static UiGlobalTransform)>,
    q_architect_submenu:
        Query<'w, 's, (&'static ComputedNode, &'static UiGlobalTransform), With<ArchitectSubMenu>>,
    q_zones_submenu:
        Query<'w, 's, (&'static ComputedNode, &'static UiGlobalTransform), With<ZonesSubMenu>>,
    q_orders_submenu:
        Query<'w, 's, (&'static ComputedNode, &'static UiGlobalTransform), With<OrdersSubMenu>>,
}

fn is_tooltip_suppressed_for_expanded_menu(
    menu_button: Option<&MenuButton>,
    menu_state: MenuState,
) -> bool {
    let Some(menu_button) = menu_button else {
        return false;
    };
    matches!(
        (menu_state, menu_button.0),
        (MenuState::Architect, MenuAction::ToggleArchitect)
            | (MenuState::Zones, MenuAction::ToggleZones)
            | (MenuState::Orders, MenuAction::ToggleOrders)
    )
}

fn compute_rect_x(computed: &ComputedNode, transform: &UiGlobalTransform) -> (f32, f32) {
    let inverse_scale = computed.inverse_scale_factor();
    let center_x = transform.translation.x * inverse_scale;
    let half_w = computed.size().x * inverse_scale * 0.5;
    (center_x - half_w, center_x + half_w)
}

fn compute_rect_y(computed: &ComputedNode, transform: &UiGlobalTransform) -> (f32, f32) {
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

fn resolve_toggle_span_x(
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

fn resolve_mode_text_span_x(
    ui_nodes: &UiNodeRegistry,
    q_layout: &Query<(&ComputedNode, &UiGlobalTransform)>,
) -> Option<(f32, f32)> {
    let entity = ui_nodes.get_slot(UiSlot::ModeText)?;
    let (computed, transform) = q_layout.get(entity).ok()?;
    Some(compute_rect_x(computed, transform))
}

fn resolve_visible_submenu_spans_x(
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

fn resolve_expanded_toggle_tooltip_position(
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

fn update_tooltip_popover_positions(
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

pub(crate) fn hover_tooltip_system(
    mut commands: Commands,
    time: Res<Time>,
    hovered: Res<crate::interface::selection::HoveredEntity>,
    menu_state: Res<MenuState>,
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
        &mut Popover,
        &ComputedNode,
    )>,
    q_children: Query<&Children>,
    mut q_nodes: Query<&mut Node, Without<HoverTooltip>>,
    mut q_tooltip_text: Query<&mut TextColor, Or<(With<TooltipHeader>, With<TooltipBody>)>>,
    mut q_tooltip_progress: Query<
        (&TooltipProgressBar, &mut BackgroundColor),
        Without<HoverTooltip>,
    >,
    ui_layout: TooltipUiLayoutQueryParam,
    inspection: EntityInspectionQuery,
    mut runtime: Local<TooltipRuntimeState>,
) {
    let Ok(window) = q_window.single() else {
        return;
    };
    let Some(tooltip_anchor) = ui_nodes.get_slot(UiSlot::TooltipAnchor) else {
        return;
    };
    let Ok((
        tooltip_entity,
        mut tooltip,
        mut tooltip_node,
        mut tooltip_bg,
        mut tooltip_border,
        mut tooltip_popover,
        tooltip_computed,
    )) = q_tooltip.single_mut()
    else {
        return;
    };

    let hovered_button =
        ui_layout
            .q_ui_tooltip_buttons
            .iter()
            .find(|(_, interaction, _, menu_button, _, _)| {
                matches!(**interaction, Interaction::Hovered | Interaction::Pressed)
                    && !is_tooltip_suppressed_for_expanded_menu(*menu_button, *menu_state)
            });

    let mut target = None;
    let mut template = TooltipTemplate::Generic;
    let mut model: Option<EntityInspectionModel> = None;
    let mut ui_tooltip: Option<UiTooltip> = None;
    let mut hovered_menu_action = None;
    let mut hovered_button_x_span = None;
    let mut hovered_button_y_span = None;
    let mut payload = String::new();

    if let Some((button_entity, _, tooltip_data, menu_button, computed, transform)) = hovered_button
    {
        target = Some(TooltipTarget::UiButton(button_entity));
        template = TooltipTemplate::UiButton;
        ui_tooltip = Some(UiTooltip {
            text: tooltip_data.text,
            shortcut: tooltip_data.shortcut,
        });
        hovered_menu_action = menu_button.map(|menu_button| menu_button.0);
        hovered_button_x_span = Some(compute_rect_x(computed, transform));
        hovered_button_y_span = Some(compute_rect_y(computed, transform));
        payload = format!(
            "ui:{}:{}",
            tooltip_data.text,
            tooltip_data.shortcut.unwrap_or_default()
        );
    } else if let Some(entity) = hovered.0
        && let Some(built_model) = inspection.build_model(entity)
    {
        template = inspection.classify_template(entity);
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
    let expanded_toggle_hover = matches!(target, Some(TooltipTarget::UiButton(_)))
        && !matches!(*menu_state, MenuState::Hidden)
        && hovered_menu_action.is_some_and(is_menu_toggle_action);
    let attach_to_anchor =
        !matches!(target, Some(TooltipTarget::UiButton(_))) || expanded_toggle_hover;
    let attachment_changed = runtime.attach_to_anchor != attach_to_anchor;

    if target_changed || attachment_changed {
        runtime.target = target;
        runtime.attach_to_anchor = attach_to_anchor;
        if attach_to_anchor {
            commands.entity(tooltip_anchor).add_child(tooltip_entity);
        } else {
            match target {
                Some(TooltipTarget::UiButton(button_entity)) => {
                    commands.entity(button_entity).add_child(tooltip_entity);
                }
                _ => {
                    commands.entity(tooltip_anchor).add_child(tooltip_entity);
                }
            }
        }
        if target_changed {
            tooltip.template_type = template;
            tooltip.delay_timer = Timer::from_seconds(0.3, TimerMode::Once);
            tooltip.delay_timer.reset();
            tooltip.fade_alpha = 0.0;
        }
    }

    if payload_changed {
        runtime.payload = payload;
    }

    if expanded_toggle_hover {
        let tooltip_size = tooltip_computed.size() * tooltip_computed.inverse_scale_factor();
        if let Some((button_x_span, button_y_span)) =
            hovered_button_x_span.zip(hovered_button_y_span)
        {
            let (x, y) = resolve_expanded_toggle_tooltip_position(
                button_x_span,
                button_y_span,
                tooltip_size,
                Vec2::new(window.width(), window.height()),
                resolve_mode_text_span_x(&ui_nodes, &ui_layout.q_layout),
                resolve_toggle_span_x(&ui_layout.q_ui_tooltip_buttons),
                &resolve_visible_submenu_spans_x(&ui_layout, *menu_state),
            );
            tooltip_node.position_type = PositionType::Absolute;
            tooltip_node.left = Val::Px(x);
            tooltip_node.top = Val::Px(y);
            tooltip_node.right = Val::Auto;
            tooltip_node.bottom = Val::Auto;
        }
        if !tooltip_popover.positions.is_empty() {
            tooltip_popover.positions.clear();
        }
    } else {
        update_tooltip_popover_positions(
            &mut tooltip_popover,
            target,
            *menu_state,
            hovered_menu_action,
        );
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
    }
    let desired_alpha = if target.is_some() && tooltip.delay_timer.is_finished() {
        1.0
    } else {
        0.0
    };
    let fade_duration = if desired_alpha > tooltip.fade_alpha {
        0.1
    } else {
        0.05
    };
    let fade_t = (time.delta_secs() / fade_duration).clamp(0.0, 1.0);
    tooltip.fade_alpha += (desired_alpha - tooltip.fade_alpha) * fade_t;
    tooltip.fade_alpha = tooltip.fade_alpha.clamp(0.0, 1.0);

    if target.is_some() && !tooltip.delay_timer.is_finished() {
        tooltip_node.display = Display::None;
    } else if tooltip.fade_alpha <= f32::EPSILON {
        tooltip_node.display = Display::None;
    } else {
        tooltip_node.display = Display::Flex;
    }

    if let Ok(mut anchor_node) = q_nodes.get_mut(tooltip_anchor) {
        if expanded_toggle_hover {
            anchor_node.left = Val::Px(0.0);
            anchor_node.top = Val::Px(0.0);
        } else if let Some(cursor_pos) = window.cursor_position() {
            anchor_node.left = Val::Px(cursor_pos.x);
            anchor_node.top = Val::Px(cursor_pos.y);
        }
    }

    let bg = theme.colors.tooltip_bg.to_srgba();
    let bg_target = Color::srgba(bg.red, bg.green, bg.blue, 0.95 * tooltip.fade_alpha);
    tooltip_bg.0 = tooltip_bg
        .0
        .try_interpolate_stable(&bg_target, fade_t)
        .unwrap_or(bg_target);

    let border = theme.colors.tooltip_border.to_srgba();
    let border_target = Color::srgba(
        border.red,
        border.green,
        border.blue,
        border.alpha * tooltip.fade_alpha,
    );
    let border_next = tooltip_border
        .top
        .try_interpolate_stable(&border_target, fade_t)
        .unwrap_or(border_target);
    *tooltip_border = BorderColor::all(border_next);

    for mut text_color in q_tooltip_text.iter_mut() {
        let current = text_color.0.to_srgba();
        let text_target =
            Color::srgba(current.red, current.green, current.blue, tooltip.fade_alpha);
        text_color.0 = text_color
            .0
            .try_interpolate_stable(&text_target, fade_t)
            .unwrap_or(text_target);
    }

    for (progress, mut color) in q_tooltip_progress.iter_mut() {
        let current = color.0.to_srgba();
        let base_alpha = (0.35 + 0.65 * progress.0).clamp(0.0, 1.0);
        let progress_target = Color::srgba(
            current.red,
            current.green,
            current.blue,
            base_alpha * tooltip.fade_alpha,
        );
        color.0 = color
            .0
            .try_interpolate_stable(&progress_target, fade_t)
            .unwrap_or(progress_target);
    }
}
