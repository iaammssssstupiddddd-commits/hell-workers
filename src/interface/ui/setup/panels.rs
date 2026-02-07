//! パネル UI (Info Panel / Hover Tooltip)
//!
//! InfoPanel と HoverTooltip を Startup 時に生成する。

use crate::interface::ui::components::{HoverTooltip, UiNodeRegistry, UiSlot};
use crate::interface::ui::panels::spawn_info_panel_ui;
use crate::interface::ui::theme::UiTheme;
use bevy::prelude::*;
use bevy::ui_widgets::popover::{Popover, PopoverAlign, PopoverPlacement, PopoverSide};

/// パネルをスポーン
pub fn spawn_panels(
    commands: &mut Commands,
    game_assets: &Res<crate::assets::GameAssets>,
    theme: &UiTheme,
    info_panel_parent: Entity,
    overlay_parent: Entity,
    ui_nodes: &mut UiNodeRegistry,
) {
    spawn_info_panel_ui(commands, game_assets, theme, info_panel_parent, ui_nodes);
    spawn_hover_tooltip(commands, theme, overlay_parent, ui_nodes);
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
                border: UiRect::all(Val::Px(theme.sizes.tooltip_border_width)), // Semantic
                padding: UiRect::all(Val::Px(theme.sizes.tooltip_padding)),     // Semantic
                max_width: Val::Px(theme.sizes.tooltip_max_width),              // Constraint
                border_radius: bevy::ui::BorderRadius::all(Val::Px(
                    theme.sizes.tooltip_corner_radius,
                )),
                ..default()
            },
            BackgroundColor(theme.colors.tooltip_bg),
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
            ZIndex(100),
            Name::new("Hover Tooltip"),
        ))
        .id();
    commands.entity(tooltip_anchor).add_child(tooltip_root);
}
