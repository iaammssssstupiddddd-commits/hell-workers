//! パネル UI (Info Panel / Hover Tooltip)
//!
//! InfoPanel と HoverTooltip を Startup 時に生成する。

use crate::interface::ui::components::{HoverTooltip, UiNodeRegistry, UiSlot};
use crate::interface::ui::panels::spawn_info_panel_ui;
use crate::interface::ui::theme::UiTheme;
use bevy::prelude::*;

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
    spawn_hover_tooltip(commands, game_assets, theme, overlay_parent, ui_nodes);
}

fn spawn_hover_tooltip(
    commands: &mut Commands,
    game_assets: &Res<crate::assets::GameAssets>,
    theme: &UiTheme,
    parent_entity: Entity,
    ui_nodes: &mut UiNodeRegistry,
) {
    let tooltip_root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                display: Display::None,
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(theme.sizes.tooltip_border_width)), // Semantic
                padding: UiRect::all(Val::Px(theme.sizes.tooltip_padding)), // Semantic
                max_width: Val::Px(theme.sizes.tooltip_max_width), // Constraint
                border_radius: bevy::ui::BorderRadius::all(Val::Px(theme.sizes.tooltip_corner_radius)),
                ..default()
            },
            BackgroundColor(theme.colors.tooltip_bg),
            BorderColor::all(theme.colors.tooltip_border),
            HoverTooltip,
            ZIndex(100),
        ))
        .id();
    commands.entity(parent_entity).add_child(tooltip_root);

    commands
        .entity(tooltip_root)
        .with_children(|tooltip| {
            let text_entity = tooltip
                .spawn((
                Text::new(""),
                TextFont {
                    font: game_assets.font_ui.clone(),
                    font_size: theme.typography.font_size_sm, // Semantic
                    ..default()
                },
                TextColor(theme.colors.text_primary_semantic), // Semantic
                UiSlot::HoverTooltipText,
            ))
                .id();
            ui_nodes.set_slot(UiSlot::HoverTooltipText, text_entity);
        });
}
