//! ダイアログ UI

use crate::interface::ui::components::{
    MenuAction, MenuButton, OperationDialog, UiInputBlocker, UiNodeRegistry, UiSlot,
};
use crate::interface::ui::theme::UiTheme;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

/// ダイアログをスポーン
pub fn spawn_dialogs(
    commands: &mut Commands,
    game_assets: &Res<crate::assets::GameAssets>,
    theme: &UiTheme,
    parent_entity: Entity,
    ui_nodes: &mut UiNodeRegistry,
) {
    spawn_operation_dialog(commands, game_assets, theme, parent_entity, ui_nodes);
}

fn spawn_operation_dialog(
    commands: &mut Commands,
    game_assets: &Res<crate::assets::GameAssets>,
    theme: &UiTheme,
    parent_entity: Entity,
    ui_nodes: &mut UiNodeRegistry,
) {
    let dialog_root = commands
        .spawn((
            Node {
                display: Display::None,
                width: Val::Px(300.0),
                height: Val::Auto,
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                top: Val::Percent(40.0),
                margin: UiRect::left(Val::Px(-150.0)),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(15.0)),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(theme.colors.dialog_bg),
            BorderColor::all(theme.colors.dialog_border),
            Interaction::default(),
            RelativeCursorPosition::default(),
            UiInputBlocker,
            OperationDialog,
            ZIndex(200),
        ))
        .id();
    commands.entity(parent_entity).add_child(dialog_root);

    commands.entity(dialog_root).with_children(|parent| {
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
                        font_size: theme.typography.font_size_dialog_header,
                        ..default()
                    },
                    TextColor(theme.colors.text_accent),
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
                        BackgroundColor(theme.panels.bottom_bar.top),
                        MenuButton(MenuAction::CloseDialog),
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new("X"),
                            TextFont {
                                font: game_assets.font_ui.clone(),
                                font_size: theme.typography.font_size_dialog_small,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    });
            });

        // Familiar Name
        let familiar_name = parent
            .spawn((
                Text::new("Familiar Name"),
                TextFont {
                    font: game_assets.font_familiar.clone(),
                    font_size: theme.typography.font_size_title,
                    ..default()
                },
                TextColor(theme.colors.header_text),
                UiSlot::DialogFamiliarName,
                Node {
                    margin: UiRect::bottom(Val::Px(15.0)),
                    ..default()
                },
            ))
            .id();
        ui_nodes.set_slot(UiSlot::DialogFamiliarName, familiar_name);

        // Section Label
        parent.spawn((
            Text::new("Work Standards:"),
            TextFont {
                font: game_assets.font_ui.clone(),
                font_size: theme.typography.font_size_dialog_small,
                ..default()
            },
            TextColor(theme.colors.text_secondary),
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
                BackgroundColor(theme.colors.overlay_row_bg),
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
                    BackgroundColor(theme.colors.button_default),
                    MenuButton(MenuAction::AdjustFatigueThreshold(-0.1)),
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("-"),
                        TextFont {
                            font_size: theme.typography.font_size_title,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });

                // Current value
                let threshold = row
                    .spawn((
                        Text::new("1"),
                        TextFont {
                            font: game_assets.font_ui.clone(),
                            font_size: theme.typography.font_size_title,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                        Node {
                            margin: UiRect::horizontal(Val::Px(20.0)),
                            ..default()
                        },
                        UiSlot::DialogThresholdText,
                    ))
                    .id();
                ui_nodes.set_slot(UiSlot::DialogThresholdText, threshold);

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
                    BackgroundColor(theme.colors.button_default),
                    MenuButton(MenuAction::AdjustFatigueThreshold(0.1)),
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("+"),
                        TextFont {
                            font_size: theme.typography.font_size_title,
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
                font_size: theme.typography.font_size_dialog_small,
                ..default()
            },
            TextColor(theme.colors.text_secondary),
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
                BackgroundColor(theme.colors.overlay_row_bg),
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
                    BackgroundColor(theme.colors.button_default),
                    MenuButton(MenuAction::AdjustMaxControlledSoul(-1)),
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("-"),
                        TextFont {
                            font: game_assets.font_ui.clone(),
                            font_size: theme.typography.font_size_dialog_header,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });

                let max_soul = row
                    .spawn((
                        Text::new("1"),
                        TextFont {
                            font_size: theme.typography.font_size_title,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                        Node {
                            margin: UiRect::horizontal(Val::Px(20.0)),
                            ..default()
                        },
                        UiSlot::DialogMaxSoulText,
                    ))
                    .id();
                ui_nodes.set_slot(UiSlot::DialogMaxSoulText, max_soul);

                row.spawn((
                    Button,
                    Node {
                        width: Val::Px(30.0),
                        height: Val::Px(30.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(theme.colors.button_default),
                    MenuButton(MenuAction::AdjustMaxControlledSoul(1)),
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("+"),
                        TextFont {
                            font: game_assets.font_ui.clone(),
                            font_size: theme.typography.font_size_dialog_header,
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
                font_size: theme.typography.font_size_dialog_tiny,
                ..default()
            },
            TextColor(theme.colors.text_muted),
            Node {
                margin: UiRect::top(Val::Px(20.0)),
                align_self: AlignSelf::Center,
                ..default()
            },
        ));
    });
}
