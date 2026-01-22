//! UIセットアップモジュール
//!
//! UIの初期構造、コンポーネント、および基本的な列挙型を定義します。

use crate::systems::jobs::BuildingType;
use crate::systems::logistics::ZoneType;
use crate::systems::time::ClockText;
use bevy::prelude::*;

// ============================================================
// UI列挙型
// ============================================================

#[derive(Resource, Default, Debug, Clone, Copy)]
pub enum MenuState {
    #[default]
    Hidden,
    Architect,
    Zones,
    Orders,
}

#[derive(Debug, Clone, Copy)]
pub enum MenuAction {
    ToggleArchitect,
    ToggleZones,
    ToggleOrders,
    SelectBuild(BuildingType),
    SelectZone(ZoneType),
    SelectTaskMode(crate::systems::command::TaskMode),
    SelectAreaTask,
    OpenOperationDialog,
    AdjustFatigueThreshold(f32),
    CloseDialog,
}

// ============================================================
// UIコンポーネント
// ============================================================

#[derive(Component)]
pub struct MenuButton(pub MenuAction);

#[derive(Component)]
pub struct ArchitectSubMenu;

#[derive(Component)]
pub struct ZonesSubMenu;

#[derive(Component)]
pub struct OrdersSubMenu;

#[derive(Component)]
pub struct InfoPanel;

#[derive(Component)]
pub struct InfoPanelJobText;

#[derive(Component)]
pub struct InfoPanelHeader;

#[derive(Component)]
pub struct ModeText;

#[derive(Component)]
pub struct ContextMenu;

#[derive(Component)]
pub struct TaskSummaryText;

#[derive(Component)]
pub struct HoverTooltipText;

#[derive(Component)]
pub struct HoverTooltip;

#[derive(Component)]
pub struct OperationDialog;

#[derive(Component)]
pub struct OperationDialogFamiliarName;

#[derive(Component)]
pub struct OperationDialogThresholdText;

// ============================================================
// UIセットアップ
// ============================================================

pub fn setup_ui(mut commands: Commands) {
    // Bottom bar
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(50.0),
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                bottom: Val::Px(0.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Start,
                padding: UiRect::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.8)),
        ))
        .with_children(|parent| {
            let buttons = [
                ("Architect", MenuAction::ToggleArchitect),
                ("Zones", MenuAction::ToggleZones),
                ("Orders", MenuAction::ToggleOrders),
            ];

            for (label, action) in buttons {
                parent
                    .spawn((
                        Button,
                        Node {
                            width: Val::Px(100.0),
                            height: Val::Px(40.0),
                            margin: UiRect::right(Val::Px(10.0)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.2, 0.2, 0.2)),
                        MenuButton(action),
                    ))
                    .with_children(|button| {
                        button.spawn((
                            Text::new(label),
                            TextFont {
                                font_size: 18.0,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    });
            }

            // Mode Display
            parent.spawn((
                Text::new("Mode: Normal"),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::srgb(0.0, 1.0, 1.0)),
                Node {
                    margin: UiRect::left(Val::Px(20.0)),
                    ..default()
                },
                ModeText,
            ));
        });

    // --- Sub-menus ---

    // Architect Sub-menu
    commands
        .spawn((
            Node {
                display: Display::None,
                width: Val::Px(120.0),
                height: Val::Auto,
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                bottom: Val::Px(50.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.1, 0.1, 0.1, 0.9)),
        ))
        .insert(ArchitectSubMenu)
        .with_children(|parent| {
            parent
                .spawn((
                    Button,
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(40.0),
                        margin: UiRect::bottom(Val::Px(5.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.3, 0.3, 0.3)),
                    MenuButton(MenuAction::SelectBuild(BuildingType::Wall)),
                ))
                .with_children(|button| {
                    button.spawn((
                        Text::new("Wall"),
                        TextFont {
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });
        });

    // Zones Sub-menu
    commands
        .spawn((
            Node {
                display: Display::None,
                width: Val::Px(120.0),
                height: Val::Auto,
                position_type: PositionType::Absolute,
                left: Val::Px(110.0),
                bottom: Val::Px(50.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.1, 0.1, 0.1, 0.9)),
        ))
        .insert(ZonesSubMenu)
        .with_children(|parent| {
            parent
                .spawn((
                    Button,
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(40.0),
                        margin: UiRect::bottom(Val::Px(5.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.3, 0.3, 0.3)),
                    MenuButton(MenuAction::SelectZone(ZoneType::Stockpile)),
                ))
                .with_children(|button| {
                    button.spawn((
                        Text::new("Stockpile"),
                        TextFont {
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });
        });

    // Orders Sub-menu
    commands
        .spawn((
            Node {
                display: Display::None,
                width: Val::Px(120.0),
                height: Val::Auto,
                position_type: PositionType::Absolute,
                left: Val::Px(220.0),
                bottom: Val::Px(50.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.1, 0.1, 0.1, 0.9)),
        ))
        .insert(OrdersSubMenu)
        .with_children(|parent| {
            let tasks = [
                (
                    "Chop",
                    crate::systems::command::TaskMode::DesignateChop(None),
                ),
                (
                    "Mine",
                    crate::systems::command::TaskMode::DesignateMine(None),
                ),
                (
                    "Haul",
                    crate::systems::command::TaskMode::DesignateHaul(None),
                ),
                (
                    "Cancel",
                    crate::systems::command::TaskMode::CancelDesignation(None),
                ),
            ];

            for (label, mode) in tasks {
                parent
                    .spawn((
                        Button,
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(40.0),
                            margin: UiRect::bottom(Val::Px(5.0)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.3, 0.3, 0.3)),
                        MenuButton(MenuAction::SelectTaskMode(mode)),
                    ))
                    .with_children(|button| {
                        button.spawn((
                            Text::new(label),
                            TextFont {
                                font_size: 16.0,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    });
            }
        });

    // Info Panel
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

    // Time Control
    commands
        .spawn((Node {
            position_type: PositionType::Absolute,
            right: Val::Px(20.0),
            top: Val::Px(20.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::End,
            ..default()
        },))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Day 1, 00:00"),
                TextFont {
                    font_size: 24.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                ClockText,
            ));

            parent
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    margin: UiRect::top(Val::Px(5.0)),
                    ..default()
                })
                .with_children(|speed_row| {
                    let speeds = [
                        (crate::systems::time::TimeSpeed::Paused, "||"),
                        (crate::systems::time::TimeSpeed::Normal, ">"),
                        (crate::systems::time::TimeSpeed::Fast, ">>"),
                        (crate::systems::time::TimeSpeed::Super, ">>>"),
                    ];

                    for (speed, label) in speeds {
                        speed_row
                            .spawn((
                                Button,
                                Node {
                                    width: Val::Px(40.0),
                                    height: Val::Px(30.0),
                                    margin: UiRect::left(Val::Px(5.0)),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.2, 0.2, 0.2)),
                                crate::systems::time::SpeedButton(speed),
                            ))
                            .with_children(|btn| {
                                btn.spawn((
                                    Text::new(label),
                                    TextFont {
                                        font_size: 16.0,
                                        ..default()
                                    },
                                    TextColor(Color::WHITE),
                                ));
                            });
                    }
                });

            // Task Summary
            parent
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    margin: UiRect::top(Val::Px(10.0)),
                    padding: UiRect::all(Val::Px(5.0)),
                    ..default()
                })
                .with_children(|summary| {
                    summary.spawn((
                        Text::new("Tasks: 0 (0 High)"),
                        TextFont {
                            font_size: 18.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.8, 0.8, 1.0)),
                        TaskSummaryText,
                    ));
                });
        });

    // Hover Tooltip
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
            BorderColor(Color::srgb(0.5, 0.5, 0.5)),
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

    // Operation Dialog
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
            BorderColor(Color::srgb(0.4, 0.4, 0.4)),
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
                            font_size: 20.0,
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
                                    font_size: 14.0,
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
                    font_size: 14.0,
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
                        Text::new("80%"),
                        TextFont {
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

            // Future slot hint
            parent.spawn((
                Text::new("(More settings coming soon)"),
                TextFont {
                    font_size: 10.0,
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
