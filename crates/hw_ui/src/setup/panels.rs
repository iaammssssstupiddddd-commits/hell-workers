//! パネル UI (Info Panel / Hover Tooltip)
//!
//! InfoPanel と HoverTooltip を Startup 時に生成する。

use crate::components::{
    HoverActionOverlay, HoverTooltip, MenuAction, MenuButton, UiNodeRegistry, UiSlot,
    UiInputBlocker,
};
use crate::theme::UiTheme;
use bevy::prelude::*;
use bevy::ui_widgets::popover::{Popover, PopoverAlign, PopoverPlacement, PopoverSide};
use super::UiSetupAssets;

/// パネルをスポーン
pub fn spawn_panels(
    commands: &mut Commands,
    game_assets: &dyn UiSetupAssets,
    theme: &UiTheme,
    overlay_parent: Entity,
    ui_nodes: &mut UiNodeRegistry,
) {
    spawn_hover_tooltip(commands, theme, overlay_parent, ui_nodes);
    spawn_hover_action_overlay(commands, game_assets, theme, overlay_parent);
}

fn spawn_hover_action_overlay(
    commands: &mut Commands,
    game_assets: &dyn UiSetupAssets,
    theme: &UiTheme,
    parent_entity: Entity,
) {
    let hover_action_button = commands
        .spawn((
            Button,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Auto,
                height: Val::Auto,
                display: Display::None,
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            HoverActionOverlay { target: None },
            BackgroundColor(theme.colors.button_default),
            BorderColor::all(theme.colors.border_default),
            MenuButton(MenuAction::MovePlantBuilding(Entity::PLACEHOLDER)),
            UiInputBlocker,
            ZIndex(30),
            Name::new("Hover Action Button"),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Move"),
                TextFont {
                    font: game_assets.font_ui().clone(),
                    font_size: theme.typography.font_size_base,
                    ..default()
                },
                TextColor(theme.colors.text_primary_semantic),
            ));
        })
        .id();
    commands.entity(parent_entity).add_child(hover_action_button);
}

fn spawn_hover_tooltip(
    commands: &mut Commands,
    theme: &UiTheme,
    parent_entity: Entity,
    ui_nodes: &mut UiNodeRegistry,
) {
    let tooltip_anchor = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Px(1.0),
                height: Val::Px(1.0),
                ..default()
            },
            UiSlot::TooltipAnchor,
            Name::new("Hover Tooltip Anchor"),
        ))
        .id();
    commands.entity(parent_entity).add_child(tooltip_anchor);
    ui_nodes.set_slot(UiSlot::TooltipAnchor, tooltip_anchor);

    let tooltip_root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                display: Display::None,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                border: UiRect::all(Val::Px(theme.sizes.tooltip_border_width)), // Semantic
                padding: UiRect::all(Val::Px(theme.sizes.tooltip_padding)),     // Semantic
                min_width: Val::Px(theme.sizes.tooltip_min_width),
                max_width: Val::Px(theme.sizes.tooltip_max_width), // Constraint
                border_radius: bevy::ui::BorderRadius::all(Val::Px(
                    theme.sizes.tooltip_corner_radius,
                )),
                ..default()
            },
            BackgroundColor(theme.colors.bg_overlay),
            BorderColor::all(theme.colors.tooltip_border),
            HoverTooltip::default(),
            Popover {
                positions: vec![
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
                ],
                window_margin: 8.0,
            },
            OverrideClip,
            ZIndex(50),
            Name::new("Hover Tooltip"),
        ))
        .id();
    commands.entity(tooltip_anchor).add_child(tooltip_root);
}
