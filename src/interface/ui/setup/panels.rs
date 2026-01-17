//! パネル UI (Info Panel, Hover Tooltip)

use crate::interface::ui::components::{
    HoverTooltip, HoverTooltipText, InfoPanel, InfoPanelCommonText, InfoPanelGenderIcon,
    InfoPanelHeader, InfoPanelInventoryText, InfoPanelStatFatigue, InfoPanelStatMotivation,
    InfoPanelStatStress, InfoPanelTaskText,
};
use bevy::prelude::*;
use bevy::ui::{BackgroundGradient, ColorStop, LinearGradient};

/// パネルをスポーン
pub fn spawn_panels(commands: &mut Commands, game_assets: &Res<crate::assets::GameAssets>) {
    spawn_info_panel(commands, game_assets);
    spawn_hover_tooltip(commands, game_assets);
}

fn spawn_info_panel(commands: &mut Commands, game_assets: &Res<crate::assets::GameAssets>) {
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
            BackgroundGradient::from(LinearGradient {
                angle: 0.0, // 左から右
                stops: vec![
                    ColorStop::new(Color::srgba(0.3, 0.1, 0.3, 0.9), Val::Percent(0.0)), // 紫っぽい
                    ColorStop::new(Color::srgba(0.0, 0.0, 0.0, 0.8), Val::Percent(100.0)),
                ],
                ..default()
            }),
            InfoPanel,
        ))
        .with_children(|parent| {
            // Header row
            parent
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    margin: UiRect::bottom(Val::Px(5.0)),
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((
                        Text::new("Entity Info"),
                        TextFont {
                            font: game_assets.font_ui.clone(),
                            font_size: crate::constants::FONT_SIZE_HEADER,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 1.0, 0.0)),
                        InfoPanelHeader,
                    ));
                    row.spawn((
                        ImageNode::default(),
                        Node {
                            width: Val::Px(16.0),
                            height: Val::Px(16.0),
                            margin: UiRect::left(Val::Px(8.0)),
                            display: Display::None,
                            ..default()
                        },
                        InfoPanelGenderIcon,
                    ));
                });

            // Soul Stats Column
            parent
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    ..default()
                })
                .with_children(|col| {
                    // Motivation (as text for now, but in separate node)
                    col.spawn((
                        Text::new(""),
                        TextFont {
                            font: game_assets.font_ui.clone(),
                            font_size: crate::constants::FONT_SIZE_SMALL,
                            ..default()
                        },
                        InfoPanelStatMotivation,
                    ));
                    // Stress row
                    col.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|row| {
                        row.spawn((
                            ImageNode::new(game_assets.icon_stress.clone()),
                            Node {
                                width: Val::Px(14.0),
                                height: Val::Px(14.0),
                                margin: UiRect::right(Val::Px(4.0)),
                                ..default()
                            },
                        ));
                        row.spawn((
                            Text::new("Stress: 0%"),
                            TextFont {
                                font: game_assets.font_ui.clone(),
                                font_size: crate::constants::FONT_SIZE_SMALL,
                                ..default()
                            },
                            InfoPanelStatStress,
                        ));
                    });
                    // Fatigue row
                    col.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|row| {
                        row.spawn((
                            ImageNode::new(game_assets.icon_fatigue.clone()),
                            Node {
                                width: Val::Px(14.0),
                                height: Val::Px(14.0),
                                margin: UiRect::right(Val::Px(4.0)),
                                ..default()
                            },
                        ));
                        row.spawn((
                            Text::new("Fatigue: 0%"),
                            TextFont {
                                font: game_assets.font_ui.clone(),
                                font_size: crate::constants::FONT_SIZE_SMALL,
                                ..default()
                            },
                            InfoPanelStatFatigue,
                        ));
                    });
                    // Task row
                    col.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        margin: UiRect::top(Val::Px(5.0)),
                        ..default()
                    })
                    .with_children(|row| {
                        row.spawn((
                            Text::new("Task: Idle"),
                            TextFont {
                                font: game_assets.font_ui.clone(),
                                font_size: crate::constants::FONT_SIZE_SMALL,
                                ..default()
                            },
                            InfoPanelTaskText,
                        ));
                    });
                    // Inventory row
                    col.spawn((
                        Text::new("Carrying: None"),
                        TextFont {
                            font: game_assets.font_ui.clone(),
                            font_size: crate::constants::FONT_SIZE_SMALL,
                            ..default()
                        },
                        InfoPanelInventoryText,
                    ));
                });

            // Common/Generic Text (Blueprints, Items, etc)
            parent.spawn((
                Text::new(""),
                TextFont {
                    font: game_assets.font_ui.clone(),
                    font_size: crate::constants::FONT_SIZE_BODY,
                    ..default()
                },
                TextColor(Color::WHITE),
                InfoPanelCommonText,
            ));
        });
}

fn spawn_hover_tooltip(commands: &mut Commands, game_assets: &Res<crate::assets::GameAssets>) {
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
                    font: game_assets.font_ui.clone(),
                    font_size: crate::constants::FONT_SIZE_SMALL,
                    ..default()
                },
                TextColor(Color::WHITE),
                HoverTooltipText,
            ));
        });
}
