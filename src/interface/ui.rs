use bevy::prelude::*;
use crate::systems::jobs::{BuildingType, Blueprint};
use crate::systems::logistics::{ZoneType, ZoneMode};
use crate::entities::damned_soul::DamnedSoul;
use crate::entities::familiar::Familiar;
use crate::systems::work::AssignedTask;
use crate::systems::time::{TimeSpeed, SpeedButton, ClockText};

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
}

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

pub fn setup_ui(mut commands: Commands) {
    // Bottom bar
    commands.spawn((
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
    )).with_children(|parent| {
        let buttons = [
            ("Architect", MenuAction::ToggleArchitect),
            ("Zones", MenuAction::ToggleZones),
            ("Orders", MenuAction::ToggleOrders),
        ];

        for (label, action) in buttons {
            parent.spawn((
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
            )).with_children(|button| {
                button.spawn((
                    Text::new(label),
                    TextFont { font_size: 18.0, ..default() },
                    TextColor(Color::WHITE),
                ));
            });
        }

        // Mode Display
        parent.spawn((
            Text::new("Mode: Normal"),
            TextFont { font_size: 18.0, ..default() },
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
    commands.spawn((
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
    )).insert(ArchitectSubMenu).with_children(|parent| {
        parent.spawn((
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
        )).with_children(|button| {
            button.spawn((
                Text::new("Wall"),
                TextFont { font_size: 16.0, ..default() },
                TextColor(Color::WHITE),
            ));
        });
    });

    // Zones Sub-menu
    commands.spawn((
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
    )).insert(ZonesSubMenu).with_children(|parent| {
        parent.spawn((
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
        )).with_children(|button| {
            button.spawn((
                Text::new("Stockpile"),
                TextFont { font_size: 16.0, ..default() },
                TextColor(Color::WHITE),
            ));
        });
    });

    // Orders Sub-menu
    commands.spawn((
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
    )).insert(OrdersSubMenu).with_children(|parent| {
        let tasks = [
            ("Chop", crate::systems::command::TaskMode::DesignateChop),
            ("Mine", crate::systems::command::TaskMode::DesignateMine),
            ("Haul", crate::systems::command::TaskMode::DesignateHaul),
        ];

        for (label, mode) in tasks {
            parent.spawn((
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
            )).with_children(|button| {
                button.spawn((
                    Text::new(label),
                    TextFont { font_size: 16.0, ..default() },
                    TextColor(Color::WHITE),
                ));
            });
        }
    });

    // Info Panel
    commands.spawn((
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
    )).with_children(|parent| {
        parent.spawn((
            Text::new("Entity Info"),
            TextFont { font_size: 20.0, ..default() },
            TextColor(Color::srgb(1.0, 1.0, 0.0)),
            InfoPanelHeader,
        ));
        parent.spawn((
            Text::new("Status: Idle"),
            TextFont { font_size: 16.0, ..default() },
            TextColor(Color::WHITE),
            InfoPanelJobText,
        ));
    });

    // Time Control
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(20.0),
            top: Val::Px(20.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::End,
            ..default()
        },
    )).with_children(|parent| {
        parent.spawn((
            Text::new("Day 1, 00:00"),
            TextFont { font_size: 24.0, ..default() },
            TextColor(Color::WHITE),
            ClockText,
        ));

        parent.spawn(Node {
            flex_direction: FlexDirection::Row,
            margin: UiRect::top(Val::Px(5.0)),
            ..default()
        }).with_children(|speed_row| {
            let speeds = [
                (crate::systems::time::TimeSpeed::Paused, "||"),
                (crate::systems::time::TimeSpeed::Normal, ">"),
                (crate::systems::time::TimeSpeed::Fast, ">>"),
                (crate::systems::time::TimeSpeed::Super, ">>>"),
            ];

            for (speed, label) in speeds {
                speed_row.spawn((
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
                )).with_children(|btn| {
                    btn.spawn((
                        Text::new(label),
                        TextFont { font_size: 16.0, ..default() },
                        TextColor(Color::WHITE),
                    ));
                });
            }
        });
    });
}

pub fn ui_interaction_system(
    mut interaction_query: Query<
        (&Interaction, &MenuButton, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
    mut menu_state: ResMut<MenuState>,
    mut build_mode: ResMut<crate::interface::selection::BuildMode>,
    mut zone_mode: ResMut<ZoneMode>,
    mut task_mode: ResMut<crate::systems::command::TaskMode>,
    q_context_menu: Query<Entity, With<ContextMenu>>,
    mut commands: Commands,
) {
    for (interaction, menu_button, mut color) in interaction_query.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                // メニューボタンが押されたらコンテキストメニューを閉じる
                for entity in q_context_menu.iter() {
                    commands.entity(entity).despawn_recursive();
                }

                *color = BackgroundColor(Color::srgb(0.5, 0.5, 0.5));
                match menu_button.0 {
                    MenuAction::ToggleArchitect => {
                        *menu_state = match *menu_state {
                            MenuState::Architect => MenuState::Hidden,
                            _ => MenuState::Architect,
                        };
                        build_mode.0 = None;
                        zone_mode.0 = None;
                    }
                    MenuAction::ToggleOrders => {
                        *menu_state = match *menu_state {
                            MenuState::Orders => MenuState::Hidden,
                            _ => MenuState::Orders,
                        };
                        build_mode.0 = None;
                        zone_mode.0 = None;
                        *task_mode = crate::systems::command::TaskMode::None;
                    }
                    MenuAction::ToggleZones => {
                        *menu_state = match *menu_state {
                            MenuState::Zones => MenuState::Hidden,
                            _ => MenuState::Zones,
                        };
                        build_mode.0 = None;
                        zone_mode.0 = None;
                        *task_mode = crate::systems::command::TaskMode::None;
                    }
                    MenuAction::SelectBuild(kind) => {
                        build_mode.0 = Some(kind);
                        zone_mode.0 = None;
                        *task_mode = crate::systems::command::TaskMode::None;
                    }
                    MenuAction::SelectZone(kind) => {
                        zone_mode.0 = Some(kind);
                        build_mode.0 = None;
                        *task_mode = crate::systems::command::TaskMode::None;
                    }
                    MenuAction::SelectTaskMode(mode) => {
                        *task_mode = mode;
                        build_mode.0 = None;
                        zone_mode.0 = None;
                        info!("UI: TaskMode set to {:?}", mode);
                    }
                    MenuAction::SelectAreaTask => {
                        *task_mode = crate::systems::command::TaskMode::AreaSelection(None);
                        build_mode.0 = None;
                        zone_mode.0 = None;
                        info!("UI: Area Selection Mode entered");
                    }
                }
            }
            Interaction::Hovered => {
                *color = BackgroundColor(Color::srgb(0.4, 0.4, 0.4));
            }
            Interaction::None => {
                *color = BackgroundColor(Color::srgb(0.2, 0.2, 0.2));
            }
        }
    }
}

pub fn menu_visibility_system(
    menu_state: Res<MenuState>,
    mut q_architect: Query<&mut Node, (With<ArchitectSubMenu>, Without<ZonesSubMenu>, Without<OrdersSubMenu>)>,
    mut q_zones: Query<&mut Node, (With<ZonesSubMenu>, Without<ArchitectSubMenu>, Without<OrdersSubMenu>)>,
    mut q_orders: Query<&mut Node, (With<OrdersSubMenu>, Without<ArchitectSubMenu>, Without<ZonesSubMenu>)>,
) {
    if let Ok(mut node) = q_architect.get_single_mut() {
        node.display = if matches!(*menu_state, MenuState::Architect) { Display::Flex } else { Display::None };
    }
    if let Ok(mut node) = q_zones.get_single_mut() {
        node.display = if matches!(*menu_state, MenuState::Zones) { Display::Flex } else { Display::None };
    }
    if let Ok(mut node) = q_orders.get_single_mut() {
        node.display = if matches!(*menu_state, MenuState::Orders) { Display::Flex } else { Display::None };
    }
}

pub fn info_panel_system(
    selected: Res<crate::interface::selection::SelectedEntity>,
    mut q_panel: Query<&mut Node, With<InfoPanel>>,
    mut q_text_job: Query<&mut Text, (With<InfoPanelJobText>, Without<InfoPanelHeader>)>,
    mut q_text_header: Query<&mut Text, (With<InfoPanelHeader>, Without<InfoPanelJobText>)>,
    q_souls: Query<(&DamnedSoul, &AssignedTask, &crate::systems::logistics::Inventory)>,
    q_blueprints: Query<&Blueprint>,
    q_familiars: Query<&Familiar>,
    q_items: Query<&crate::systems::logistics::ResourceItem>,
    q_trees: Query<&crate::systems::jobs::Tree>,
    q_rocks: Query<&crate::systems::jobs::Rock>,
) {
    let mut panel_node = q_panel.single_mut();
    panel_node.display = Display::None;

    if let Some(entity) = selected.0 {
        let mut header_text = q_text_header.single_mut();
        let mut job_text = q_text_job.single_mut();

        if let Ok((soul, task, inventory)) = q_souls.get(entity) {
            panel_node.display = Display::Flex;
            header_text.0 = "Damned Soul Info".to_string();
            
            // タスクの詳細情報
            let task_str = match task {
                AssignedTask::None => "Idle".to_string(),
                AssignedTask::Gather { phase, .. } => format!("Gather ({:?})", phase),
                AssignedTask::Haul { phase, .. } => format!("Haul ({:?})", phase),
            };
            
            // インベントリ情報
            let inv_str = if let Some(item_entity) = inventory.0 {
                if let Ok(item) = q_items.get(item_entity) {
                    format!("Carrying: {:?}", item.0)
                } else {
                    format!("Carrying: Entity {:?}", item_entity)
                }
            } else {
                "Carrying: None".to_string()
            };
            
            job_text.0 = format!(
                "Motivation: {:.0}%\nLaziness: {:.0}%\nFatigue: {:.0}%\nTask: {}\n{}",
                soul.motivation * 100.0,
                soul.laziness * 100.0,
                soul.fatigue * 100.0,
                task_str,
                inv_str
            );
        } else if let Ok(bp) = q_blueprints.get(entity) {
            panel_node.display = Display::Flex;
            header_text.0 = "Blueprint Info".to_string();
            job_text.0 = format!(
                "Type: {:?}\nProgress: {:.0}%",
                bp.kind,
                bp.progress * 100.0
            );
        } else if let Ok(familiar) = q_familiars.get(entity) {
            panel_node.display = Display::Flex;
            header_text.0 = "Familiar Info".to_string();
            job_text.0 = format!(
                "Type: {:?}\nRange: {:.0} tiles",
                familiar.familiar_type,
                familiar.command_radius / 16.0
            );
        } else if let Ok(item) = q_items.get(entity) {
            panel_node.display = Display::Flex;
            header_text.0 = "Resource Item".to_string();
            job_text.0 = format!("Type: {:?}", item.0);
        } else if let Ok(_) = q_trees.get(entity) {
            panel_node.display = Display::Flex;
            header_text.0 = "Tree".to_string();
            job_text.0 = "Natural resource: Wood".to_string();
        } else if let Ok(_) = q_rocks.get(entity) {
            panel_node.display = Display::Flex;
            header_text.0 = "Rock".to_string();
            job_text.0 = "Natural resource: Stone".to_string();
        }
    }
}

pub fn update_mode_text_system(
    task_mode: Res<crate::systems::command::TaskMode>,
    build_mode: Res<crate::interface::selection::BuildMode>,
    mut q_text: Query<&mut Text, With<ModeText>>,
) {
    if let Ok(mut text) = q_text.get_single_mut() {
        let mode_str = if let Some(kind) = build_mode.0 {
            format!("Mode: Build ({:?})", kind)
        } else {
            match *task_mode {
                crate::systems::command::TaskMode::None => "Mode: Normal".to_string(),
                crate::systems::command::TaskMode::DesignateChop => "Mode: Chop (Click tree)".to_string(),
                crate::systems::command::TaskMode::DesignateMine => "Mode: Mine (Click rock)".to_string(),
                crate::systems::command::TaskMode::DesignateHaul => "Mode: Haul (Click item)".to_string(),
                crate::systems::command::TaskMode::SelectBuildTarget => "Mode: Build Select".to_string(),
                crate::systems::command::TaskMode::AreaSelection(None) => "Mode: Area (Click start)".to_string(),
                crate::systems::command::TaskMode::AreaSelection(Some(_)) => "Mode: Area (Click end)".to_string(),
            }
        };
        text.0 = mode_str;
    }
}

pub fn familiar_context_menu_system(
    mut commands: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<crate::interface::camera::MainCamera>>,
    q_familiars: Query<&GlobalTransform, With<crate::entities::familiar::Familiar>>,
    q_context_menu: Query<Entity, With<ContextMenu>>,
    q_ui_interaction: Query<&Interaction, With<Button>>,
) {
    if buttons.just_pressed(MouseButton::Left) {
        // UIクリック時は無視
        if q_ui_interaction.iter().any(|i| *i != Interaction::None) {
            return;
        }

        // 既存のメニューを閉じる
        for entity in q_context_menu.iter() {
            commands.entity(entity).despawn_recursive();
        }

        let (camera, camera_transform) = q_camera.single();
        let window = q_window.single();

        if let Some(cursor_pos) = window.cursor_position() {
            if let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) {
                let mut clicked_familiar = false;
                for transform in q_familiars.iter() {
                    let pos = transform.translation().truncate();
                    if pos.distance(world_pos) < crate::constants::TILE_SIZE / 2.0 {
                        clicked_familiar = true;
                        break;
                    }
                }

                if clicked_familiar {
                    commands.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(cursor_pos.x),
                            top: Val::Px(cursor_pos.y),
                            width: Val::Px(80.0),
                            height: Val::Auto,
                            flex_direction: FlexDirection::Column,
                            padding: UiRect::all(Val::Px(5.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.1, 0.1, 0.1, 0.9)),
                        ContextMenu,
                    )).with_children(|parent| {
                        parent.spawn((
                            Button,
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(30.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.3, 0.3, 0.3)),
                            MenuButton(MenuAction::SelectAreaTask),
                        )).with_children(|button| {
                            button.spawn((
                                Text::new("Task"),
                                TextFont { font_size: 14.0, ..default() },
                                TextColor(Color::WHITE),
                            ));
                        });
                    });
                }
            }
        }
    }
}
