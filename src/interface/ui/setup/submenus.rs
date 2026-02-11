//! サブメニュー UI (Architect, Zones, Orders)

use crate::interface::ui::components::{
    ArchitectSubMenu, MenuAction, MenuButton, OrdersSubMenu, UiInputBlocker, ZonesSubMenu,
};
use crate::interface::ui::theme::UiTheme;
use crate::systems::jobs::BuildingType;
use crate::systems::logistics::ZoneType;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

/// サブメニューをスポーン
pub fn spawn_submenus(
    commands: &mut Commands,
    game_assets: &Res<crate::assets::GameAssets>,
    theme: &UiTheme,
    parent_entity: Entity,
) {
    spawn_architect_submenu(commands, game_assets, theme, parent_entity);
    spawn_zones_submenu(commands, game_assets, theme, parent_entity);
    spawn_orders_submenu(commands, game_assets, theme, parent_entity);
}

fn spawn_architect_submenu(
    commands: &mut Commands,
    game_assets: &Res<crate::assets::GameAssets>,
    theme: &UiTheme,
    parent_entity: Entity,
) {
    let items = [
        ("Wall", BuildingType::Wall),
        ("Tank", BuildingType::Tank),
        ("Floor", BuildingType::Floor),
        ("MudMixer", BuildingType::MudMixer),
        ("SandPile", BuildingType::SandPile),
        ("WB Parking", BuildingType::WheelbarrowParking),
    ];

    let submenu = commands
        .spawn((
            Node {
                display: Display::None,
                width: Val::Px(theme.sizes.submenu_width),
                height: Val::Auto,
                position_type: PositionType::Absolute,
                left: Val::Px(theme.sizes.submenu_left_architect),
                bottom: Val::Px(theme.spacing.bottom_bar_height),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(theme.colors.submenu_bg),
            RelativeCursorPosition::default(),
            UiInputBlocker,
            ArchitectSubMenu,
        ))
        .id();
    commands.entity(parent_entity).add_child(submenu);

    commands.entity(submenu).with_children(|parent| {
        for (label, kind) in items {
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
                    BackgroundColor(theme.colors.button_default),
                    MenuButton(MenuAction::SelectBuild(kind)),
                ))
                .with_children(|button| {
                    button.spawn((
                        Text::new(label),
                        TextFont {
                            font: game_assets.font_ui.clone(),
                            font_size: theme.typography.font_size_title,
                            ..default()
                        },
                        TextColor(theme.colors.text_primary),
                    ));
                });
        }
    });
}

fn spawn_zones_submenu(
    commands: &mut Commands,
    game_assets: &Res<crate::assets::GameAssets>,
    theme: &UiTheme,
    parent_entity: Entity,
) {
    let submenu = commands
        .spawn((
            Node {
                display: Display::None,
                width: Val::Px(theme.sizes.submenu_width),
                height: Val::Auto,
                position_type: PositionType::Absolute,
                left: Val::Px(theme.sizes.submenu_left_zones),
                bottom: Val::Px(theme.spacing.bottom_bar_height),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(theme.colors.submenu_bg),
            RelativeCursorPosition::default(),
            UiInputBlocker,
            ZonesSubMenu,
        ))
        .id();
    commands.entity(parent_entity).add_child(submenu);

    commands.entity(submenu).with_children(|parent| {
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
                BackgroundColor(theme.colors.button_default),
                MenuButton(MenuAction::SelectZone(ZoneType::Stockpile)),
            ))
            .with_children(|button| {
                button.spawn((
                    Text::new("Stockpile"),
                    TextFont {
                        font: game_assets.font_ui.clone(),
                        font_size: theme.typography.font_size_title,
                        ..default()
                    },
                    TextColor(theme.colors.text_primary),
                ));
            });

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
                BackgroundColor(theme.colors.status_danger), // 削除アクションなので赤系
                MenuButton(MenuAction::RemoveZone(ZoneType::Stockpile)),
            ))
            .with_children(|button| {
                button.spawn((
                    Text::new("Remove"),
                    TextFont {
                        font: game_assets.font_ui.clone(),
                        font_size: theme.typography.font_size_title,
                        ..default()
                    },
                    TextColor(theme.colors.text_primary),
                ));
            });
    });
}

fn spawn_orders_submenu(
    commands: &mut Commands,
    game_assets: &Res<crate::assets::GameAssets>,
    theme: &UiTheme,
    parent_entity: Entity,
) {
    let actions = [
        (
            "Chop",
            MenuAction::SelectTaskMode(crate::systems::command::TaskMode::DesignateChop(None)),
        ),
        (
            "Mine",
            MenuAction::SelectTaskMode(crate::systems::command::TaskMode::DesignateMine(None)),
        ),
        (
            "Haul",
            MenuAction::SelectTaskMode(crate::systems::command::TaskMode::DesignateHaul(None)),
        ),
        (
            "Cancel",
            MenuAction::SelectTaskMode(crate::systems::command::TaskMode::CancelDesignation(None)),
        ),
        ("Area", MenuAction::SelectAreaTask),
    ];

    let submenu = commands
        .spawn((
            Node {
                display: Display::None,
                width: Val::Px(theme.sizes.submenu_width),
                height: Val::Auto,
                position_type: PositionType::Absolute,
                left: Val::Px(theme.sizes.submenu_left_orders),
                bottom: Val::Px(theme.spacing.bottom_bar_height),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(theme.colors.submenu_bg),
            RelativeCursorPosition::default(),
            UiInputBlocker,
            OrdersSubMenu,
        ))
        .id();
    commands.entity(parent_entity).add_child(submenu);

    commands.entity(submenu).with_children(|parent| {
        for (label, action) in actions {
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
                    BackgroundColor(theme.colors.button_default),
                    MenuButton(action),
                ))
                .with_children(|button| {
                    button.spawn((
                        Text::new(label),
                        TextFont {
                            font: game_assets.font_ui.clone(),
                            font_size: theme.typography.font_size_title,
                            ..default()
                        },
                        TextColor(theme.colors.text_primary),
                    ));
                });
        }
    });
}
