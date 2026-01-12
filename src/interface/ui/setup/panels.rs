//! パネル UI (Info Panel, Hover Tooltip)

use crate::interface::ui::components::{
    HoverTooltip, HoverTooltipText, InfoPanel, InfoPanelHeader, InfoPanelJobText,
};
use bevy::prelude::*;

/// パネルをスポーン
pub fn spawn_panels(commands: &mut Commands) {
    spawn_info_panel(commands);
    spawn_hover_tooltip(commands);
}

fn spawn_info_panel(commands: &mut Commands) {
    commands
        .spawn((
            Node {
                display: Display::None,
                width: Val::Px(200.0),
                height: Val::Auto,
                position_type: PositionType::Absolute,
                right: Val::Px(20.0),
                top: Val::Px(120.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.8)),
            InfoPanel,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Entity Info"),
                TextFont {
                    font_size: 20.0,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 1.0, 0.0)),
                InfoPanelHeader,
            ));
            parent.spawn((
                Text::new("Status: Idle"),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                InfoPanelJobText,
            ));
        });
}

fn spawn_hover_tooltip(commands: &mut Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                display: Display::None,
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(1.0)),
                padding: UiRect::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.9)),
            BorderColor::all(Color::srgb(0.5, 0.5, 0.5)),
            HoverTooltip,
            ZIndex(100),
        ))
        .with_children(|tooltip| {
            tooltip.spawn((
                Text::new(""),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                HoverTooltipText,
            ));
        });
}
