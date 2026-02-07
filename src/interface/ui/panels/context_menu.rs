//! コンテキストメニュー管理

use crate::entities::damned_soul::DamnedSoul;
use crate::entities::familiar::Familiar;
use crate::interface::selection::HoveredEntity;
use crate::interface::ui::components::*;
use crate::interface::ui::theme::UiTheme;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use bevy::ui_widgets::popover::{Popover, PopoverAlign, PopoverPlacement, PopoverSide};

#[derive(Clone, Copy, Debug)]
enum ContextTarget {
    Familiar(Entity),
    Soul(Entity),
    Building(Entity),
    Resource(Entity),
}

pub fn context_menu_system(
    mut commands: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    hovered: Res<HoveredEntity>,
    ui_input_state: Res<UiInputState>,
    mut selected_entity: ResMut<crate::interface::selection::SelectedEntity>,
    q_context_menu: Query<Entity, With<ContextMenu>>,
    q_familiars: Query<(), With<Familiar>>,
    q_souls: Query<(), With<DamnedSoul>>,
    q_buildings: Query<
        (),
        Or<(
            With<crate::systems::jobs::Building>,
            With<crate::systems::jobs::Blueprint>,
        )>,
    >,
    q_resources: Query<
        (),
        Or<(
            With<crate::systems::logistics::ResourceItem>,
            With<crate::systems::jobs::Tree>,
            With<crate::systems::jobs::Rock>,
        )>,
    >,
    game_assets: Res<crate::assets::GameAssets>,
    theme: Res<UiTheme>,
) {
    if buttons.just_pressed(MouseButton::Left) {
        // UI上クリック時（メニュー項目クリック含む）は、ボタン処理側に委譲する。
        // ワールド上クリック時のみここでメニューを閉じる。
        if !ui_input_state.pointer_over_ui {
            despawn_context_menus(&mut commands, &q_context_menu);
        }
        return;
    }

    if !buttons.just_pressed(MouseButton::Right) {
        return;
    }
    if ui_input_state.pointer_over_ui {
        return;
    }

    despawn_context_menus(&mut commands, &q_context_menu);

    let Some(target_entity) = hovered.0 else {
        return;
    };
    let target = classify_target(
        target_entity,
        &q_familiars,
        &q_souls,
        &q_buildings,
        &q_resources,
    );
    let Some(target) = target else {
        return;
    };

    let Ok(window) = q_window.single() else {
        return;
    };
    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };

    selected_entity.0 = Some(target_entity);

    let anchor = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(cursor_pos.x),
                top: Val::Px(cursor_pos.y),
                width: Val::Px(1.0),
                height: Val::Px(1.0),
                ..default()
            },
            ContextMenu,
            Name::new("Context Menu Anchor"),
        ))
        .id();

    commands.entity(anchor).with_children(|parent| {
        parent
            .spawn((
                Node {
                    min_width: Val::Px(140.0),
                    max_width: Val::Px(220.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(2.0),
                    padding: UiRect::all(Val::Px(6.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(theme.colors.submenu_bg),
                BorderColor::all(theme.colors.border_default),
                RelativeCursorPosition::default(),
                UiInputBlocker,
                Popover {
                    positions: vec![
                        PopoverPlacement {
                            side: PopoverSide::Bottom,
                            align: PopoverAlign::Start,
                            gap: 4.0,
                        },
                        PopoverPlacement {
                            side: PopoverSide::Right,
                            align: PopoverAlign::Start,
                            gap: 4.0,
                        },
                        PopoverPlacement {
                            side: PopoverSide::Top,
                            align: PopoverAlign::Start,
                            gap: 4.0,
                        },
                    ],
                    window_margin: 8.0,
                },
                ZIndex(220),
                Name::new("Context Menu"),
            ))
            .with_children(|menu| match target {
                ContextTarget::Familiar(entity) => {
                    spawn_menu_item(
                        menu,
                        "Inspect (Pin)",
                        MenuAction::InspectEntity(entity),
                        &game_assets,
                        &theme,
                    );
                    spawn_menu_item(
                        menu,
                        "Assign Area Task",
                        MenuAction::SelectAreaTask,
                        &game_assets,
                        &theme,
                    );
                    spawn_menu_item(
                        menu,
                        "Open Operation",
                        MenuAction::OpenOperationDialog,
                        &game_assets,
                        &theme,
                    );
                }
                ContextTarget::Soul(entity) => {
                    spawn_menu_item(
                        menu,
                        "Inspect (Pin)",
                        MenuAction::InspectEntity(entity),
                        &game_assets,
                        &theme,
                    );
                }
                ContextTarget::Building(entity) => {
                    spawn_menu_item(
                        menu,
                        "Inspect (Pin)",
                        MenuAction::InspectEntity(entity),
                        &game_assets,
                        &theme,
                    );
                }
                ContextTarget::Resource(entity) => {
                    spawn_menu_item(
                        menu,
                        "Inspect (Pin)",
                        MenuAction::InspectEntity(entity),
                        &game_assets,
                        &theme,
                    );
                }
            });
    });
}

fn classify_target(
    entity: Entity,
    q_familiars: &Query<(), With<Familiar>>,
    q_souls: &Query<(), With<DamnedSoul>>,
    q_buildings: &Query<
        (),
        Or<(
            With<crate::systems::jobs::Building>,
            With<crate::systems::jobs::Blueprint>,
        )>,
    >,
    q_resources: &Query<
        (),
        Or<(
            With<crate::systems::logistics::ResourceItem>,
            With<crate::systems::jobs::Tree>,
            With<crate::systems::jobs::Rock>,
        )>,
    >,
) -> Option<ContextTarget> {
    if q_familiars.get(entity).is_ok() {
        Some(ContextTarget::Familiar(entity))
    } else if q_souls.get(entity).is_ok() {
        Some(ContextTarget::Soul(entity))
    } else if q_buildings.get(entity).is_ok() {
        Some(ContextTarget::Building(entity))
    } else if q_resources.get(entity).is_ok() {
        Some(ContextTarget::Resource(entity))
    } else {
        None
    }
}

fn spawn_menu_item(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    action: MenuAction,
    game_assets: &crate::assets::GameAssets,
    theme: &UiTheme,
) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(30.0),
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(theme.colors.interactive_default),
            MenuButton(action),
        ))
        .with_children(|row| {
            row.spawn((
                Text::new(label),
                TextFont {
                    font: game_assets.font_ui.clone(),
                    font_size: theme.typography.font_size_sm,
                    ..default()
                },
                TextColor(theme.colors.text_primary_semantic),
            ));
        });
}

fn despawn_context_menus(
    commands: &mut Commands,
    q_context_menu: &Query<Entity, With<ContextMenu>>,
) {
    for entity in q_context_menu.iter() {
        commands.entity(entity).despawn();
    }
}
