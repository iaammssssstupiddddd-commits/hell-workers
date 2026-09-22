//! サブメニュー UI (Architect, Zones, Orders)

use super::UiAssets;
use crate::components::{
    ArchitectBuildingPanel, ArchitectCategoryListPanel, ArchitectSubMenu, DreamSubMenu, MenuAction,
    MenuButton, OrdersSubMenu, UiInputBlocker, UiSlot, ZonesSubMenu,
};
use crate::theme::UiTheme;
use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use bevy::ui_widgets::ScrollArea;

#[derive(Component)]
pub struct SubmenuViewportAnchor {
    left: Val,
    estimated_width: f32,
}

pub fn fit_submenus_to_viewport(
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    scale: Res<UiScale>,
    theme: Res<UiTheme>,
    guidance: Query<
        (&UiSlot, &Node, &ComputedNode, &UiGlobalTransform),
        Without<SubmenuViewportAnchor>,
    >,
    mut menus: Query<(&SubmenuViewportAnchor, &ComputedNode, &mut Node)>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let viewport = window.size() / scale.0.max(f32::EPSILON);
    let mut bottom = theme.spacing.bottom_bar_height;
    for (slot, node, computed, transform) in &guidance {
        if *slot == UiSlot::ModeText && node.display != Display::None && computed.size().y > 0.0 {
            let top = (transform.translation.y - computed.size().y * 0.5)
                * computed.inverse_scale_factor();
            bottom = bottom.max(viewport.y - top + 4.0);
        }
    }
    for (anchor, computed, mut node) in &mut menus {
        let desired = match anchor.left {
            Val::Px(left) => left,
            Val::Percent(percent) => viewport.x * percent / 100.0,
            _ => 0.0,
        };
        let width = if computed.size().x > 0.0 {
            computed.size().x * computed.inverse_scale_factor()
        } else {
            anchor.estimated_width
        };
        node.left = Val::Px(desired.clamp(4.0, (viewport.x - width - 4.0).max(4.0)));
        node.max_width = Val::Px((viewport.x - 8.0).max(0.0));
        node.bottom = Val::Px(bottom);
        node.max_height = Val::Px((viewport.y - bottom - 8.0).max(0.0));
    }
}
use hw_core::game_state::TaskMode;
use hw_jobs::{BuildingCategory, BuildingType};
use hw_logistics::zone::ZoneType;

/// サブメニューをスポーン
pub fn spawn_submenus(
    commands: &mut Commands,
    game_assets: &dyn UiAssets,
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

struct SubmenuContainerSpec<T: Bundle> {
    left: Val,
    width: Val,
    flex_direction: FlexDirection,
    align_items: Option<AlignItems>,
    marker: T,
}

fn spawn_submenu_container<T: Bundle>(
    commands: &mut Commands,
    theme: &UiTheme,
    parent_entity: Entity,
    spec: SubmenuContainerSpec<T>,
) -> Entity {
    let SubmenuContainerSpec {
        left,
        width,
        flex_direction,
        align_items,
        marker,
    } = spec;
    let mut node = Node {
        display: Display::None,
        width,
        height: Val::Auto,
        position_type: PositionType::Absolute,
        left,
        bottom: Val::Px(theme.spacing.bottom_bar_height),
        flex_direction,
        padding: UiRect::all(Val::Px(5.0)),
        overflow: Overflow::scroll_y(),
        ..default()
    };
    if let Some(align) = align_items {
        node.align_items = align;
    }

    let submenu = commands
        .spawn((
            node,
            BackgroundColor(theme.colors.bg_surface),
            RelativeCursorPosition::default(),
            UiInputBlocker,
            ScrollArea,
            SubmenuViewportAnchor {
                left,
                estimated_width: match width {
                    Val::Px(width) => width,
                    _ => theme.sizes.submenu_width * 2.0 + 10.0,
                },
            },
            marker,
        ))
        .id();
    commands.entity(parent_entity).add_child(submenu);
    submenu
}

fn spawn_menu_entries(
    parent: &mut ChildSpawnerCommands,
    game_assets: &dyn UiAssets,
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
    game_assets: &dyn UiAssets,
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
                min_height: Val::Px(32.0),
                min_width: Val::Px(32.0),
                flex_shrink: 0.0,
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
                    font: game_assets.font_ui().clone().into(),
                    font_size: crate::theme::font_size_rem(theme.typography.font_size_title),
                    ..default()
                },
                TextColor(theme.colors.text_primary),
                TextLayout::new(Justify::Center, LineBreak::WordOrCharacter),
            ));
        });
}

fn spawn_category_button(
    parent: &mut ChildSpawnerCommands,
    assets: &dyn UiAssets,
    theme: &UiTheme,
    category: Option<BuildingCategory>,
) {
    let label = match category {
        None => "すべて",
        Some(BuildingCategory::Structure) => "壁・床・橋",
        Some(BuildingCategory::Architecture) => "出入口",
        Some(BuildingCategory::Plant) => "生産・設備",
        Some(BuildingCategory::Temporary) => "休息・屋外",
    };
    parent
        .spawn((
            Button,
            Node {
                min_height: Val::Px(32.0),
                padding: UiRect::horizontal(Val::Px(12.0)),
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(theme.colors.button_default),
            MenuButton(MenuAction::SelectArchitectCategory(category)),
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(label),
                TextFont {
                    font: assets.font_ui().clone().into(),
                    font_size: crate::theme::font_size_rem(14.0),
                    ..default()
                },
                TextColor(theme.colors.text_primary_semantic),
            ));
        });
}

fn spawn_building_panel(
    parent: &mut ChildSpawnerCommands,
    assets: &dyn UiAssets,
    theme: &UiTheme,
    category: BuildingCategory,
    items: Vec<MenuEntrySpec<'static>>,
) {
    for entry in items {
        let kind = match entry.action {
            MenuAction::SelectBuild(kind) => kind,
            MenuAction::SelectFloorPlace => BuildingType::Floor,
            MenuAction::SelectTaskMode(TaskMode::SoulSpaPlace(_)) => BuildingType::SoulSpa,
            _ => continue,
        };
        let (name, role) = crate::catalog::building_copy(kind);
        parent
            .spawn((
                Button,
                Node {
                    width: Val::Px(140.0),
                    height: Val::Px(56.0),
                    flex_shrink: 0.0,
                    padding: UiRect::all(Val::Px(6.0)),
                    column_gap: Val::Px(6.0),
                    align_items: AlignItems::Center,
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(theme.colors.button_default),
                MenuButton(entry.action),
                ArchitectBuildingPanel(category),
                crate::components::UiTooltip::new(role),
            ))
            .with_children(|card| {
                card.spawn((
                    ImageNode::new(assets.building_preview(kind).clone()),
                    crate::components::BuildingCatalogPreview(kind),
                    Node {
                        width: Val::Px(32.0),
                        height: Val::Px(32.0),
                        flex_shrink: 0.0,
                        ..default()
                    },
                ));
                card.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    flex_grow: 1.0,
                    min_width: Val::Px(0.0),
                    ..default()
                })
                .with_children(|copy| {
                    copy.spawn((
                        Text::new(name),
                        TextFont {
                            font: assets.font_ui().clone().into(),
                            font_size: crate::theme::font_size_rem(14.0),
                            ..default()
                        },
                        TextColor(theme.colors.text_primary_semantic),
                    ));
                    copy.spawn((
                        Text::new(role),
                        TextFont {
                            font: assets.font_ui().clone().into(),
                            font_size: crate::theme::font_size_rem(12.0),
                            ..default()
                        },
                        TextColor(theme.colors.text_secondary_semantic),
                    ));
                });
            });
    }
}

fn spawn_architect_submenu(
    commands: &mut Commands,
    assets: &dyn UiAssets,
    theme: &UiTheme,
    parent_entity: Entity,
) {
    let submenu = spawn_submenu_container(
        commands,
        theme,
        parent_entity,
        SubmenuContainerSpec {
            left: Val::Px(12.0),
            width: Val::Px(880.0),
            flex_direction: FlexDirection::Column,
            align_items: Some(AlignItems::Stretch),
            marker: ArchitectSubMenu,
        },
    );
    commands.entity(submenu).with_children(|parent| {
        parent
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    column_gap: Val::Px(6.0),
                    margin: UiRect::bottom(Val::Px(6.0)),
                    flex_shrink: 0.0,
                    ..default()
                },
                ArchitectCategoryListPanel,
            ))
            .with_children(|categories| {
                spawn_category_button(categories, assets, theme, None);
                for category in architect_categories() {
                    spawn_category_button(categories, assets, theme, Some(*category));
                }
            });
        parent
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(4.0),
                row_gap: Val::Px(4.0),
                flex_shrink: 0.0,
                ..default()
            })
            .with_children(|cards| {
                for category in architect_categories() {
                    spawn_building_panel(
                        cards,
                        assets,
                        theme,
                        *category,
                        architect_building_specs(*category, theme.colors.button_default),
                    );
                }
            });
    });
}

fn spawn_zones_submenu(
    commands: &mut Commands,
    game_assets: &dyn UiAssets,
    theme: &UiTheme,
    parent_entity: Entity,
) {
    let submenu = spawn_submenu_container(
        commands,
        theme,
        parent_entity,
        SubmenuContainerSpec {
            left: Val::Px(theme.sizes.submenu_left_zones),
            width: Val::Px(theme.sizes.submenu_width),
            flex_direction: FlexDirection::Column,
            align_items: None,
            marker: ZonesSubMenu,
        },
    );

    commands.entity(submenu).with_children(|parent| {
        let entries = zones_menu_specs(theme);
        spawn_menu_entries(parent, game_assets, theme, entries);
    });
}

fn spawn_orders_submenu(
    commands: &mut Commands,
    game_assets: &dyn UiAssets,
    theme: &UiTheme,
    parent_entity: Entity,
) {
    let submenu = spawn_submenu_container(
        commands,
        theme,
        parent_entity,
        SubmenuContainerSpec {
            left: Val::Px(theme.sizes.submenu_left_orders),
            width: Val::Px(theme.sizes.submenu_width),
            flex_direction: FlexDirection::Column,
            align_items: None,
            marker: OrdersSubMenu,
        },
    );

    commands.entity(submenu).with_children(|parent| {
        let entries = orders_menu_specs(theme);
        spawn_menu_entries(parent, game_assets, theme, entries);
    });
}

fn spawn_dream_submenu(
    commands: &mut Commands,
    game_assets: &dyn UiAssets,
    theme: &UiTheme,
    parent_entity: Entity,
) {
    let submenu = spawn_submenu_container(
        commands,
        theme,
        parent_entity,
        SubmenuContainerSpec {
            left: Val::Px(theme.sizes.submenu_left_dream),
            width: Val::Px(theme.sizes.submenu_width),
            flex_direction: FlexDirection::Column,
            align_items: None,
            marker: DreamSubMenu,
        },
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
            MenuEntrySpec::new(
                "Soul Spa",
                MenuAction::SelectTaskMode(TaskMode::SoulSpaPlace(None)),
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
            MenuEntrySpec::new(
                "Outdoor Lamp",
                MenuAction::SelectBuild(BuildingType::OutdoorLamp),
                button_color,
            ),
        ],
    }
}

fn zones_menu_specs(theme: &UiTheme) -> Vec<MenuEntrySpec<'static>> {
    vec![
        MenuEntrySpec::new(
            "保管場所",
            MenuAction::SelectZone(ZoneType::Stockpile),
            theme.colors.button_default,
        ),
        MenuEntrySpec::new(
            "Yardを拡張",
            MenuAction::SelectZone(ZoneType::Yard),
            theme.colors.button_default,
        ),
        MenuEntrySpec::new(
            "保管範囲を削除",
            MenuAction::RemoveZone(ZoneType::Stockpile),
            theme.colors.status_danger,
        ),
    ]
}

fn orders_menu_specs(theme: &UiTheme) -> Vec<MenuEntrySpec<'static>> {
    vec![
        MenuEntrySpec::new(
            "伐採",
            MenuAction::SelectTaskMode(TaskMode::DesignateChop(None)),
            theme.colors.button_default,
        ),
        MenuEntrySpec::new(
            "採掘",
            MenuAction::SelectTaskMode(TaskMode::DesignateMine(None)),
            theme.colors.button_default,
        ),
        MenuEntrySpec::new(
            "運搬",
            MenuAction::SelectTaskMode(TaskMode::DesignateHaul(None)),
            theme.colors.button_default,
        ),
        MenuEntrySpec::new(
            "建物を解体",
            MenuAction::SelectTaskMode(TaskMode::DesignateDeconstruct(None)),
            theme.colors.status_danger,
        ),
        MenuEntrySpec::new(
            "作業指示を取消",
            MenuAction::SelectTaskMode(TaskMode::CancelDesignation(None)),
            theme.colors.button_default,
        ),
        MenuEntrySpec::new(
            "使い魔の作業範囲を指定 / 変更",
            MenuAction::SelectAreaTask,
            theme.colors.button_default,
        ),
    ]
}

fn dream_menu_specs(theme: &UiTheme) -> Vec<MenuEntrySpec<'static>> {
    vec![MenuEntrySpec::new(
        "植樹",
        MenuAction::SelectDreamPlanting,
        theme.colors.button_default,
    )]
}

#[cfg(test)]
mod layout_tests {
    use super::*;

    #[test]
    fn submenu_reserves_visible_guidance_height_at_scaled_viewport() {
        let mut app = App::new();
        app.insert_resource(UiScale(1.25))
            .init_resource::<UiTheme>()
            .add_systems(Update, fit_submenus_to_viewport);
        app.world_mut().spawn((
            Window {
                resolution: (1280, 720).into(),
                ..default()
            },
            bevy::window::PrimaryWindow,
        ));
        app.world_mut().spawn((
            UiSlot::ModeText,
            Node::default(),
            ComputedNode {
                size: Vec2::new(600.0, 80.0),
                inverse_scale_factor: 0.8,
                ..default()
            },
            UiGlobalTransform::from_translation(Vec2::new(300.0, 620.0)),
        ));
        let menu = app
            .world_mut()
            .spawn((
                Node::default(),
                ComputedNode::default(),
                SubmenuViewportAnchor {
                    left: Val::Px(100.0),
                    estimated_width: 150.0,
                },
            ))
            .id();
        app.update();
        let node = app.world().get::<Node>(menu).unwrap();
        assert_eq!(node.bottom, Val::Px(116.0));
        assert_eq!(node.max_height, Val::Px(452.0));
    }
}
