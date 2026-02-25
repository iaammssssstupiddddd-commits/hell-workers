//! ツールチップ表示・ターゲット解決・フェード制御

mod fade;
mod layout;
mod target;

use crate::interface::ui::components::*;
use crate::interface::ui::panels::tooltip_builder;
use crate::interface::ui::presentation::{EntityInspectionModel, EntityInspectionQuery};
use crate::interface::ui::theme::UiTheme;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::ui_widgets::popover::Popover;

pub(crate) use target::TooltipTarget;

#[derive(Default)]
pub(crate) struct TooltipRuntimeState {
    pub target: Option<TooltipTarget>,
    pub payload: String,
    pub attach_to_anchor: bool,
}

#[derive(SystemParam)]
pub(crate) struct TooltipRenderQueries<'w, 's> {
    q_children: Query<'w, 's, &'static Children>,
    q_nodes: Query<'w, 's, &'static mut Node, Without<HoverTooltip>>,
    q_tooltip_text:
        Query<'w, 's, &'static mut TextColor, Or<(With<TooltipHeader>, With<TooltipBody>)>>,
    q_tooltip_progress: Query<
        'w,
        's,
        (&'static TooltipProgressBar, &'static mut BackgroundColor),
        Without<HoverTooltip>,
    >,
}

pub(crate) fn hover_tooltip_system(
    mut commands: Commands,
    time: Res<Time>,
    hovered: Res<crate::interface::selection::HoveredEntity>,
    mut placement_failure_tooltip: ResMut<PlacementFailureTooltip>,
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
    mut render_queries: TooltipRenderQueries,
    ui_layout: layout::TooltipUiLayoutQueryParam,
    inspection: EntityInspectionQuery,
    mut runtime: Local<TooltipRuntimeState>,
) {
    placement_failure_tooltip.tick(time.delta_secs());

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
                    && !target::is_tooltip_suppressed_for_expanded_menu(*menu_button, *menu_state)
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
        hovered_menu_action = menu_button.map(|mb| mb.0);
        hovered_button_x_span = Some(layout::compute_rect_x(computed, transform));
        hovered_button_y_span = Some(layout::compute_rect_y(computed, transform));
        payload = format!(
            "ui:{}:{}",
            tooltip_data.text,
            tooltip_data.shortcut.unwrap_or_default()
        );
    } else if let Some(reason) = placement_failure_tooltip.message.as_ref() {
        target = Some(TooltipTarget::PlacementFailure);
        template = TooltipTemplate::Generic;
        payload = format!("placement_failure:{reason}");
        model = Some(EntityInspectionModel {
            header: "Cannot Place".to_string(),
            common_text: String::new(),
            tooltip_lines: vec![reason.clone()],
            soul: None,
        });
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
        && hovered_menu_action.is_some_and(|a| {
            matches!(
                a,
                MenuAction::ToggleArchitect | MenuAction::ToggleZones | MenuAction::ToggleOrders
            )
        });
    // ZIndex付きパネル内のボタン（速度ボタン等）はアンカーに留める（スタッキングコンテキスト回避）
    let button_in_zindex_panel =
        matches!(target, Some(TooltipTarget::UiButton(e)) if ui_layout.q_speed_buttons.contains(e));
    let attach_to_anchor = !matches!(target, Some(TooltipTarget::UiButton(_)))
        || expanded_toggle_hover
        || button_in_zindex_panel;
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
            let delay_secs = if matches!(target, Some(TooltipTarget::PlacementFailure)) {
                0.05
            } else {
                0.3
            };
            tooltip.delay_timer = Timer::from_seconds(delay_secs, TimerMode::Once);
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
            let (x, y) = layout::resolve_expanded_toggle_tooltip_position(
                button_x_span,
                button_y_span,
                tooltip_size,
                Vec2::new(window.width(), window.height()),
                layout::resolve_mode_text_span_x(&ui_nodes, &ui_layout.q_layout),
                layout::resolve_toggle_span_x(&ui_layout.q_ui_tooltip_buttons),
                &layout::resolve_visible_submenu_spans_x(&ui_layout, *menu_state),
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
        layout::update_tooltip_popover_positions(
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
            &render_queries.q_children,
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

    if let Ok(mut anchor_node) = render_queries.q_nodes.get_mut(tooltip_anchor) {
        if expanded_toggle_hover {
            anchor_node.left = Val::Px(0.0);
            anchor_node.top = Val::Px(0.0);
        } else if let Some(cursor_pos) = window.cursor_position() {
            anchor_node.left = Val::Px(cursor_pos.x);
            anchor_node.top = Val::Px(cursor_pos.y);
        }
    }

    fade::apply_fade_effects(
        &mut tooltip_bg,
        &mut tooltip_border,
        &mut render_queries.q_tooltip_text,
        &mut render_queries.q_tooltip_progress,
        tooltip.fade_alpha,
        &theme,
        fade_t,
    );
}
