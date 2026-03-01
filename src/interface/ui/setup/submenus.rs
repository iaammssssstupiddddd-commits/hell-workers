//! サブメニュー UI (Architect, Zones, Orders)

use crate::interface::ui::components::{
    ArchitectBuildingPanel, ArchitectCategoryListPanel, ArchitectSubMenu, DreamSubMenu, MenuAction,
    MenuButton, OrdersSubMenu, UiInputBlocker, ZonesSubMenu,
};
use crate::interface::ui::theme::UiTheme;
use crate::systems::command::TaskMode;
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

struct MenuEntrySpec<'a> {
    label: &'a str,
    action: MenuAction,
    background_color: Color,
}

impl<'a> MenuEntrySpec<'a> {
    fn new(label: &'a str, action: MenuAction, background_color: Color) -> Self {
        Self {
            label,
            action,
            background_color,
        }
    }
}

fn spawn_submenu_container<T: Bundle>(
    commands: &mut Commands,
    theme: &UiTheme,
    parent_entity: Entity,
    left: Val,
    width: Val,
    flex_direction: FlexDirection,
    align_items: Option<AlignItems>,
    marker: T,
) -> Entity {
    let mut node = Node {
        display: Display::None,
        width,
        height: Val::Auto,
        position_type: PositionType::Absolute,
        left,
        bottom: Val::Px(theme.spacing.bottom_bar_height),
        flex_direction,
        padding: UiRect::all(Val::Px(5.0)),
        ..default()
    };
    if let Some(align) = align_items {
        node.align_items = align;
    }

    let submenu = commands
        .spawn((
            node,
            BackgroundColor(theme.colors.submenu_bg),
            RelativeCursorPosition::default(),
            UiInputBlocker,
            marker,
        ))
        .id();
    commands.entity(parent_entity).add_child(submenu);
    submenu
}

fn spawn_menu_entries(
    parent: &mut ChildSpawnerCommands,
    game_assets: &Res<crate::assets::GameAssets>,
    theme: &UiTheme,
    entries: Vec<MenuEntrySpec<'static>>,
) {
    for entry in entries {
        spawn_menu_button(
            parent,
            game_assets,
            theme,
            entry.label,
            entry.action,
            entry.background_color,
        );
    }
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
    items: Vec<MenuEntrySpec<'static>>,
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
            spawn_menu_entries(panel, game_assets, theme, items);
        });
}

fn spawn_architect_submenu(
    commands: &mut Commands,
    game_assets: &Res<crate::assets::GameAssets>,
    theme: &UiTheme,
    parent_entity: Entity,
) {
    let submenu = spawn_submenu_container(
        commands,
        theme,
        parent_entity,
        Val::Px(theme.sizes.submenu_left_architect),
        Val::Auto,
        FlexDirection::Row,
        Some(AlignItems::Start),
        ArchitectSubMenu,
    );

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
                for category in architect_categories() {
                    spawn_category_button(cat_panel, game_assets, theme, *category);
                }
            });

        for category in architect_categories() {
            spawn_building_panel(
                parent,
                game_assets,
                theme,
                *category,
                architect_building_specs(*category, theme.colors.button_default),
            );
        }
    });
}

fn spawn_zones_submenu(
    commands: &mut Commands,
    game_assets: &Res<crate::assets::GameAssets>,
    theme: &UiTheme,
    parent_entity: Entity,
) {
    let submenu = spawn_submenu_container(
        commands,
        theme,
        parent_entity,
        Val::Px(theme.sizes.submenu_left_zones),
        Val::Px(theme.sizes.submenu_width),
        FlexDirection::Column,
        None,
        ZonesSubMenu,
    );

    commands.entity(submenu).with_children(|parent| {
        let entries = zones_menu_specs(theme);
        spawn_menu_entries(parent, game_assets, theme, entries);
    });
}

fn spawn_orders_submenu(
    commands: &mut Commands,
    game_assets: &Res<crate::assets::GameAssets>,
    theme: &UiTheme,
    parent_entity: Entity,
) {
    let submenu = spawn_submenu_container(
        commands,
        theme,
        parent_entity,
        Val::Px(theme.sizes.submenu_left_orders),
        Val::Px(theme.sizes.submenu_width),
        FlexDirection::Column,
        None,
        OrdersSubMenu,
    );

    commands.entity(submenu).with_children(|parent| {
        let entries = orders_menu_specs(theme);
        spawn_menu_entries(parent, game_assets, theme, entries);
    });
}

fn spawn_dream_submenu(
    commands: &mut Commands,
    game_assets: &Res<crate::assets::GameAssets>,
    theme: &UiTheme,
    parent_entity: Entity,
) {
    let submenu = spawn_submenu_container(
        commands,
        theme,
        parent_entity,
        Val::Px(theme.sizes.submenu_left_dream),
        Val::Px(theme.sizes.submenu_width),
        FlexDirection::Column,
        None,
        DreamSubMenu,
    );

    commands.entity(submenu).with_children(|parent| {
        let entries = dream_menu_specs(theme);
        spawn_menu_entries(parent, game_assets, theme, entries);
    });
}

const ARCHITECT_CATEGORIES: [BuildingCategory; 4] = [
    BuildingCategory::Structure,
    BuildingCategory::Architecture,
    BuildingCategory::Plant,
    BuildingCategory::Temporary,
];

fn architect_categories() -> &'static [BuildingCategory] {
    &ARCHITECT_CATEGORIES
}

fn architect_building_specs(
    category: BuildingCategory,
    button_color: Color,
) -> Vec<MenuEntrySpec<'static>> {
    match category {
        BuildingCategory::Structure => vec![
            MenuEntrySpec::new(
                "Wall",
                MenuAction::SelectBuild(BuildingType::Wall),
                button_color,
            ),
            MenuEntrySpec::new("Floor", MenuAction::SelectFloorPlace, button_color),
            MenuEntrySpec::new(
                "Bridge",
                MenuAction::SelectBuild(BuildingType::Bridge),
                button_color,
            ),
        ],
        BuildingCategory::Architecture => {
            vec![MenuEntrySpec::new(
                "Door",
                MenuAction::SelectBuild(BuildingType::Door),
                button_color,
            )]
        }
        BuildingCategory::Plant => vec![
            MenuEntrySpec::new(
                "Tank",
                MenuAction::SelectBuild(BuildingType::Tank),
                button_color,
            ),
            MenuEntrySpec::new(
                "MudMixer",
                MenuAction::SelectBuild(BuildingType::MudMixer),
                button_color,
            ),
        ],
        BuildingCategory::Temporary => vec![
            MenuEntrySpec::new(
                "RestArea",
                MenuAction::SelectBuild(BuildingType::RestArea),
                button_color,
            ),
            MenuEntrySpec::new(
                "WB Parking",
                MenuAction::SelectBuild(BuildingType::WheelbarrowParking),
                button_color,
            ),
            MenuEntrySpec::new(
                "SandPile",
                MenuAction::SelectBuild(BuildingType::SandPile),
                button_color,
            ),
            MenuEntrySpec::new(
                "BonePile",
                MenuAction::SelectBuild(BuildingType::BonePile),
                button_color,
            ),
        ],
    }
}

fn zones_menu_specs(theme: &UiTheme) -> Vec<MenuEntrySpec<'static>> {
    vec![
        MenuEntrySpec::new(
            "Stockpile",
            MenuAction::SelectZone(ZoneType::Stockpile),
            theme.colors.button_default,
        ),
        MenuEntrySpec::new(
            "Remove",
            MenuAction::RemoveZone(ZoneType::Stockpile),
            theme.colors.status_danger,
        ),
    ]
}

fn orders_menu_specs(theme: &UiTheme) -> Vec<MenuEntrySpec<'static>> {
    vec![
        MenuEntrySpec::new(
            "Chop",
            MenuAction::SelectTaskMode(TaskMode::DesignateChop(None)),
            theme.colors.button_default,
        ),
        MenuEntrySpec::new(
            "Mine",
            MenuAction::SelectTaskMode(TaskMode::DesignateMine(None)),
            theme.colors.button_default,
        ),
        MenuEntrySpec::new(
            "Haul",
            MenuAction::SelectTaskMode(TaskMode::DesignateHaul(None)),
            theme.colors.button_default,
        ),
        MenuEntrySpec::new(
            "Cancel",
            MenuAction::SelectTaskMode(TaskMode::CancelDesignation(None)),
            theme.colors.button_default,
        ),
        MenuEntrySpec::new("Area", MenuAction::SelectAreaTask, theme.colors.button_default),
    ]
}

fn dream_menu_specs(theme: &UiTheme) -> Vec<MenuEntrySpec<'static>> {
    vec![MenuEntrySpec::new(
        "Plant Trees",
        MenuAction::SelectDreamPlanting,
        theme.colors.button_default,
    )]
}
