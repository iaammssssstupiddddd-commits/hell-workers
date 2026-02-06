//! サブメニュー UI (Architect, Zones, Orders)

use crate::interface::ui::components::{
    ArchitectSubMenu, MenuAction, MenuButton, OrdersSubMenu, ZonesSubMenu,
};
use crate::interface::ui::theme::*;
use crate::systems::jobs::BuildingType;
use crate::systems::logistics::ZoneType;
use bevy::prelude::*;

/// サブメニューをスポーン
pub fn spawn_submenus(commands: &mut Commands, game_assets: &Res<crate::assets::GameAssets>) {
    spawn_architect_submenu(commands, game_assets);
    spawn_zones_submenu(commands, game_assets);
    spawn_orders_submenu(commands, game_assets);
}

fn spawn_architect_submenu(commands: &mut Commands, game_assets: &Res<crate::assets::GameAssets>) {
    commands
        .spawn((
            Node {
                display: Display::None,
                width: Val::Px(SUBMENU_WIDTH),
                height: Val::Auto,
                position_type: PositionType::Absolute,
                left: Val::Px(SUBMENU_LEFT_ARCHITECT),
                bottom: Val::Px(BOTTOM_BAR_HEIGHT),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.1, 0.1, 0.1, 0.9)),
        ))
        .insert(ArchitectSubMenu)
        .with_children(|parent| {
            // Wall button
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
                    BackgroundColor(COLOR_BUTTON_DEFAULT),
                    MenuButton(MenuAction::SelectBuild(BuildingType::Wall)),
                ))
                .with_children(|button| {
                    button.spawn((
                        Text::new("Wall"),
                        TextFont {
                            font: game_assets.font_ui.clone(),
                            font_size: FONT_SIZE_TITLE,
                            ..default()
                        },
                        TextColor(COLOR_TEXT_PRIMARY),
                    ));
                });

            // Tank button
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
                    BackgroundColor(COLOR_BUTTON_DEFAULT),
                    MenuButton(MenuAction::SelectBuild(BuildingType::Tank)),
                ))
                .with_children(|button| {
                    button.spawn((
                        Text::new("Tank"),
                        TextFont {
                            font: game_assets.font_ui.clone(),
                            font_size: FONT_SIZE_TITLE,
                            ..default()
                        },
                        TextColor(COLOR_TEXT_PRIMARY),
                    ));
                });

            // Floor button
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
                    BackgroundColor(COLOR_BUTTON_DEFAULT),
                    MenuButton(MenuAction::SelectBuild(BuildingType::Floor)),
                ))
                .with_children(|button| {
                    button.spawn((
                        Text::new("Floor"),
                        TextFont {
                            font: game_assets.font_ui.clone(),
                            font_size: FONT_SIZE_TITLE,
                            ..default()
                        },
                        TextColor(COLOR_TEXT_PRIMARY),
                    ));
                });

            // MudMixer button
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
                    BackgroundColor(COLOR_BUTTON_DEFAULT),
                    MenuButton(MenuAction::SelectBuild(BuildingType::MudMixer)),
                ))
                .with_children(|button| {
                    button.spawn((
                        Text::new("MudMixer"),
                        TextFont {
                            font: game_assets.font_ui.clone(),
                            font_size: FONT_SIZE_TITLE,
                            ..default()
                        },
                        TextColor(COLOR_TEXT_PRIMARY),
                    ));
                });
        });
}

fn spawn_zones_submenu(commands: &mut Commands, game_assets: &Res<crate::assets::GameAssets>) {
    commands
        .spawn((
            Node {
                display: Display::None,
                width: Val::Px(SUBMENU_WIDTH),
                height: Val::Auto,
                position_type: PositionType::Absolute,
                left: Val::Px(SUBMENU_LEFT_ZONES),
                bottom: Val::Px(BOTTOM_BAR_HEIGHT),
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
                    BackgroundColor(COLOR_BUTTON_DEFAULT),
                    MenuButton(MenuAction::SelectZone(ZoneType::Stockpile)),
                ))
                .with_children(|button| {
                    button.spawn((
                        Text::new("Stockpile"),
                        TextFont {
                            font: game_assets.font_ui.clone(),
                            font_size: FONT_SIZE_TITLE,
                            ..default()
                        },
                        TextColor(COLOR_TEXT_PRIMARY),
                    ));
                });
        });
}

fn spawn_orders_submenu(commands: &mut Commands, game_assets: &Res<crate::assets::GameAssets>) {
    commands
        .spawn((
            Node {
                display: Display::None,
                width: Val::Px(SUBMENU_WIDTH),
                height: Val::Auto,
                position_type: PositionType::Absolute,
                left: Val::Px(SUBMENU_LEFT_ORDERS),
                bottom: Val::Px(BOTTOM_BAR_HEIGHT),
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
                        BackgroundColor(COLOR_BUTTON_DEFAULT),
                        MenuButton(MenuAction::SelectTaskMode(mode)),
                    ))
                    .with_children(|button| {
                        button.spawn((
                            Text::new(label),
                            TextFont {
                                font: game_assets.font_ui.clone(),
                                font_size: FONT_SIZE_TITLE,
                                ..default()
                            },
                            TextColor(COLOR_TEXT_PRIMARY),
                        ));
                    });
            }
        });
}
