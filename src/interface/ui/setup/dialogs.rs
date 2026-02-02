//! ダイアログ UI

use crate::interface::ui::components::{
    MenuAction, MenuButton, OperationDialog, OperationDialogFamiliarName,
    OperationDialogMaxSoulText, OperationDialogThresholdText,
};
use bevy::prelude::*;

/// ダイアログをスポーン
pub fn spawn_dialogs(commands: &mut Commands, game_assets: &Res<crate::assets::GameAssets>) {
    spawn_operation_dialog(commands, game_assets);
}

fn spawn_operation_dialog(commands: &mut Commands, game_assets: &Res<crate::assets::GameAssets>) {
    commands
        .spawn((
            Node {
                display: Display::None,
                width: Val::Px(300.0),
                height: Val::Auto,
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                top: Val::Percent(40.0),
                margin: UiRect::left(Val::Px(-150.0)), // Center horizontally
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(15.0)),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.05, 0.95)),
            BorderColor::all(Color::srgb(0.4, 0.4, 0.4)),
            Interaction::default(),
            OperationDialog,
            ZIndex(200),
        ))
        .with_children(|parent| {
            // Header with Close Button
            parent
                .spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    margin: UiRect::bottom(Val::Px(10.0)),
                    ..default()
                })
                .with_children(|header| {
                    header.spawn((
                        Text::new("Familiar Operation"),
                        TextFont {
                            font: game_assets.font_ui.clone(),
                            font_size: crate::constants::FONT_SIZE_HEADER,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 1.0, 0.0)),
                    ));

                    header
                        .spawn((
                            Button,
                            Node {
                                width: Val::Px(24.0),
                                height: Val::Px(24.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.4, 0.1, 0.1)),
                            MenuButton(MenuAction::CloseDialog),
                        ))
                        .with_children(|btn| {
                            btn.spawn((
                                Text::new("X"),
                                TextFont {
                                    font: game_assets.font_ui.clone(),
                                    font_size: crate::constants::FONT_SIZE_SMALL,
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                            ));
                        });
                });

            // Familiar Name
            parent.spawn((
                Text::new("Familiar Name"),
                TextFont {
                    font: game_assets.font_familiar.clone(),
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::srgb(0.8, 0.8, 0.8)),
                OperationDialogFamiliarName,
                Node {
                    margin: UiRect::bottom(Val::Px(15.0)),
                    ..default()
                },
            ));

            // Section Label
            parent.spawn((
                Text::new("Work Standards:"),
                TextFont {
                    font: game_assets.font_ui.clone(),
                    font_size: crate::constants::FONT_SIZE_SMALL,
                    ..default()
                },
                TextColor(Color::srgb(0.6, 0.6, 0.6)),
                Node {
                    margin: UiRect::bottom(Val::Px(5.0)),
                    ..default()
                },
            ));

            // Fatigue Threshold Adjustment
            parent
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        padding: UiRect::vertical(Val::Px(10.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.05)),
                ))
                .with_children(|row| {
                    // Decrease button
                    row.spawn((
                        Button,
                        Node {
                            width: Val::Px(30.0),
                            height: Val::Px(30.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.3, 0.3, 0.3)),
                        MenuButton(MenuAction::AdjustFatigueThreshold(-0.1)),
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new("-"),
                            TextFont {
                                font_size: 20.0,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    });

                    // Current value
                    row.spawn((
                        Text::new("1"),
                        TextFont {
                            font: game_assets.font_ui.clone(),
                            font_size: 18.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                        Node {
                            margin: UiRect::horizontal(Val::Px(20.0)),
                            ..default()
                        },
                        OperationDialogThresholdText,
                    ));

                    // Increase button
                    row.spawn((
                        Button,
                        Node {
                            width: Val::Px(30.0),
                            height: Val::Px(30.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.3, 0.3, 0.3)),
                        MenuButton(MenuAction::AdjustFatigueThreshold(0.1)),
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new("+"),
                            TextFont {
                                font_size: 20.0,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    });
                });

            // Max Controlled Souls Adjustment
            parent.spawn((
                Text::new("Max Controlled Souls:"),
                TextFont {
                    font: game_assets.font_ui.clone(),
                    font_size: crate::constants::FONT_SIZE_SMALL,
                    ..default()
                },
                TextColor(Color::srgb(0.6, 0.6, 0.6)),
                Node {
                    margin: UiRect {
                        top: Val::Px(15.0),
                        bottom: Val::Px(5.0),
                        ..default()
                    },
                    ..default()
                },
            ));

            parent
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        padding: UiRect::vertical(Val::Px(10.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.05)),
                ))
                .with_children(|row| {
                    row.spawn((
                        Button,
                        Node {
                            width: Val::Px(30.0),
                            height: Val::Px(30.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.3, 0.3, 0.3)),
                        MenuButton(MenuAction::AdjustMaxControlledSoul(-1)),
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new("-"),
                            TextFont {
                                font: game_assets.font_ui.clone(),
                                font_size: crate::constants::FONT_SIZE_HEADER,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    });

                    row.spawn((
                        Text::new("1"),
                        TextFont {
                            font_size: 18.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                        Node {
                            margin: UiRect::horizontal(Val::Px(20.0)),
                            ..default()
                        },
                        OperationDialogMaxSoulText,
                    ));

                    row.spawn((
                        Button,
                        Node {
                            width: Val::Px(30.0),
                            height: Val::Px(30.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.3, 0.3, 0.3)),
                        MenuButton(MenuAction::AdjustMaxControlledSoul(1)),
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new("+"),
                            TextFont {
                                font: game_assets.font_ui.clone(),
                                font_size: crate::constants::FONT_SIZE_HEADER,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    });
                });

            // Future slot hint
            parent.spawn((
                Text::new("(Settings automatically synced)"),
                TextFont {
                    font: game_assets.font_ui.clone(),
                    font_size: crate::constants::FONT_SIZE_TINY,
                    ..default()
                },
                TextColor(Color::srgba(1.0, 1.0, 1.0, 0.3)),
                Node {
                    margin: UiRect::top(Val::Px(20.0)),
                    align_self: AlignSelf::Center,
                    ..default()
                },
            ));
        });
}
