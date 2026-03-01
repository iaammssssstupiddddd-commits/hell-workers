//! サブメニュー UI (Architect, Zones, Orders)

use crate::interface::ui::components::{
    ArchitectBuildingPanel, ArchitectCategoryListPanel, ArchitectSubMenu, DreamSubMenu, MenuAction,
    MenuButton, OrdersSubMenu, UiInputBlocker, ZonesSubMenu,
};
use crate::interface::ui::theme::UiTheme;
use crate::systems::jobs::{BuildingCategory, BuildingType};
use crate::systems::logistics::ZoneType;
use bevy::ecs::hierarchy::ChildSpawnerCommands;
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
    spawn_dream_submenu(commands, game_assets, theme, parent_entity);
}

fn spawn_menu_button(
    parent: &mut ChildSpawnerCommands,
    game_assets: &Res<crate::assets::GameAssets>,
    theme: &UiTheme,
    label: &str,
    action: MenuAction,
    bg_color: Color,
) {
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
            BackgroundColor(bg_color),
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

fn spawn_category_button(
    parent: &mut ChildSpawnerCommands,
    game_assets: &Res<crate::assets::GameAssets>,
    theme: &UiTheme,
    category: BuildingCategory,
) {
    spawn_menu_button(
        parent,
        game_assets,
        theme,
        category.label(),
        MenuAction::SelectArchitectCategory(Some(category)),
        theme.colors.button_default,
    );
}

fn spawn_building_panel(
    parent: &mut ChildSpawnerCommands,
    game_assets: &Res<crate::assets::GameAssets>,
    theme: &UiTheme,
    category: BuildingCategory,
    items: &[(&str, MenuAction)],
) {
    parent
        .spawn((
            Node {
                display: Display::None,
                width: Val::Px(theme.sizes.submenu_width),
                height: Val::Auto,
                flex_direction: FlexDirection::Column,
                padding: UiRect::left(Val::Px(5.0)),
                border: UiRect::left(Val::Px(1.0)),
                ..default()
            },
            BorderColor::all(theme.colors.border_default),
            ArchitectBuildingPanel(category),
        ))
        .with_children(|panel| {
            for (label, action) in items {
                spawn_menu_button(
                    panel,
                    game_assets,
                    theme,
                    label,
                    action.clone(),
                    theme.colors.button_default,
                );
            }
        });
}

fn spawn_architect_submenu(
    commands: &mut Commands,
    game_assets: &Res<crate::assets::GameAssets>,
    theme: &UiTheme,
    parent_entity: Entity,
) {
    let submenu = commands
        .spawn((
            Node {
                display: Display::None,
                width: Val::Auto,
                height: Val::Auto,
                position_type: PositionType::Absolute,
                left: Val::Px(theme.sizes.submenu_left_architect),
                bottom: Val::Px(theme.spacing.bottom_bar_height),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Start,
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
        // カテゴリ選択パネル（常時表示・左列）
        parent
            .spawn((
                Node {
                    display: Display::Flex,
                    width: Val::Px(theme.sizes.submenu_width),
                    height: Val::Auto,
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                ArchitectCategoryListPanel,
            ))
            .with_children(|cat_panel| {
                spawn_category_button(cat_panel, game_assets, theme, BuildingCategory::Structure);
                spawn_category_button(cat_panel, game_assets, theme, BuildingCategory::Architecture);
                spawn_category_button(cat_panel, game_assets, theme, BuildingCategory::Plant);
                spawn_category_button(cat_panel, game_assets, theme, BuildingCategory::Temporary);
            });

        // Structure パネル
        spawn_building_panel(
            parent,
            game_assets,
            theme,
            BuildingCategory::Structure,
            &[
                ("Wall", MenuAction::SelectBuild(BuildingType::Wall)),
                ("Floor", MenuAction::SelectFloorPlace),
                ("Bridge", MenuAction::SelectBuild(BuildingType::Bridge)),
            ],
        );

        // Architecture パネル
        spawn_building_panel(
            parent,
            game_assets,
            theme,
            BuildingCategory::Architecture,
            &[("Door", MenuAction::SelectBuild(BuildingType::Door))],
        );

        // Plant パネル
        spawn_building_panel(
            parent,
            game_assets,
            theme,
            BuildingCategory::Plant,
            &[
                ("Tank", MenuAction::SelectBuild(BuildingType::Tank)),
                ("MudMixer", MenuAction::SelectBuild(BuildingType::MudMixer)),
            ],
        );

        // Temporary パネル
        spawn_building_panel(
            parent,
            game_assets,
            theme,
            BuildingCategory::Temporary,
            &[
                ("RestArea", MenuAction::SelectBuild(BuildingType::RestArea)),
                (
                    "WB Parking",
                    MenuAction::SelectBuild(BuildingType::WheelbarrowParking),
                ),
                ("SandPile", MenuAction::SelectBuild(BuildingType::SandPile)),
                ("BonePile", MenuAction::SelectBuild(BuildingType::BonePile)),
            ],
        );
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

fn spawn_dream_submenu(
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
                left: Val::Px(theme.sizes.submenu_left_dream),
                bottom: Val::Px(theme.spacing.bottom_bar_height),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(theme.colors.submenu_bg),
            bevy::ui::RelativeCursorPosition::default(),
            UiInputBlocker,
            DreamSubMenu,
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
                MenuButton(MenuAction::SelectDreamPlanting),
            ))
            .with_children(|button| {
                button.spawn((
                    Text::new("Plant Trees"),
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
