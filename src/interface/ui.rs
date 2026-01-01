use bevy::prelude::*;
use crate::systems::jobs::{BuildingType, Blueprint, CurrentJob};
use crate::systems::logistics::{ZoneType, ZoneMode};
use crate::entities::colonist::Colonist;
use crate::entities::damned_soul::DamnedSoul;
use crate::entities::familiar::{Familiar, ActiveCommand, FamiliarCommand};
use crate::systems::work::AssignedTask;
use crate::systems::time::{TimeSpeed, SpeedButton, ClockText};

#[derive(Resource, Default, Debug, Clone, Copy)]
pub enum MenuState {
    #[default]
    Hidden,
    Architect,
    Zones,
}

#[derive(Debug, Clone, Copy)]
pub enum MenuAction {
    ToggleArchitect,
    ToggleZones,
    SelectBuild(BuildingType),
    SelectZone(ZoneType),
}

#[derive(Component)]
pub struct MenuButton(pub MenuAction);

#[derive(Component)]
pub struct ArchitectSubMenu;

#[derive(Component)]
pub struct ZonesSubMenu;

#[derive(Component)]
pub struct InfoPanel;

#[derive(Component)]
pub struct InfoPanelJobText;

#[derive(Component)]
pub struct InfoPanelHeader;

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
        // Architect button
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
            MenuButton(MenuAction::ToggleArchitect),
        )).with_children(|button| {
            button.spawn((
                Text::new("Architect"),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });

        // Zones button
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
            MenuButton(MenuAction::ToggleZones),
        )).with_children(|button| {
            button.spawn((
                Text::new("Zones"),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
    });

    // Sub-menus (Architect and Zones)
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

    // Info Panel
    commands.spawn((
        Node {
            display: Display::None,
            width: Val::Px(200.0),
            height: Val::Auto,
            position_type: PositionType::Absolute,
            right: Val::Px(20.0),
            top: Val::Px(120.0), // Below clock
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

    // Time Control & Clock UI
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
                (TimeSpeed::Paused, "||"),
                (TimeSpeed::Normal, ">"),
                (TimeSpeed::Fast, ">>"),
                (TimeSpeed::Super, ">>>"),
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
                    SpeedButton(speed),
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
) {
    for (interaction, menu_button, mut color) in interaction_query.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
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
                    MenuAction::ToggleZones => {
                        *menu_state = match *menu_state {
                            MenuState::Zones => MenuState::Hidden,
                            _ => MenuState::Zones,
                        };
                        build_mode.0 = None;
                        zone_mode.0 = None;
                    }
                    MenuAction::SelectBuild(kind) => {
                        build_mode.0 = Some(kind);
                        zone_mode.0 = None;
                    }
                    MenuAction::SelectZone(kind) => {
                        zone_mode.0 = Some(kind);
                        build_mode.0 = None;
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
    mut q_architect: Query<&mut Node, (With<ArchitectSubMenu>, Without<ZonesSubMenu>)>,
    mut q_zones: Query<&mut Node, (With<ZonesSubMenu>, Without<ArchitectSubMenu>)>,
) {
    if let Ok(mut node) = q_architect.get_single_mut() {
        node.display = if matches!(*menu_state, MenuState::Architect) { Display::Flex } else { Display::None };
    }
    if let Ok(mut node) = q_zones.get_single_mut() {
        node.display = if matches!(*menu_state, MenuState::Zones) { Display::Flex } else { Display::None };
    }
}

pub fn info_panel_system(
    selected: Res<crate::interface::selection::SelectedEntity>,
    mut q_panel: Query<&mut Node, With<InfoPanel>>,
    mut q_text_job: Query<&mut Text, (With<InfoPanelJobText>, Without<InfoPanelHeader>)>,
    mut q_text_header: Query<&mut Text, (With<InfoPanelHeader>, Without<InfoPanelJobText>)>,
    q_colonists: Query<&CurrentJob, With<Colonist>>,
    q_souls: Query<(&DamnedSoul, &AssignedTask)>,
    q_blueprints: Query<&Blueprint>,
) {
    let mut panel_node = q_panel.single_mut();
    
    if let Some(entity) = selected.0 {
        let mut header_text = q_text_header.single_mut();
        let mut job_text = q_text_job.single_mut();

        if let Ok(job) = q_colonists.get(entity) {
            panel_node.display = Display::Flex;
            header_text.0 = "Colonist Info".to_string();
            if let Some(job_entity) = job.0 {
                if let Ok(bp) = q_blueprints.get(job_entity) {
                    job_text.0 = format!("Job: Building {:?} ({:.0}%)", bp.kind, bp.progress * 100.0);
                } else {
                    job_text.0 = "Job: Moving".to_string();
                }
            } else {
                job_text.0 = "Job: Idle".to_string();
            }
        } else if let Ok((soul, task)) = q_souls.get(entity) {
            panel_node.display = Display::Flex;
            header_text.0 = "Damned Soul Info".to_string();
            let task_str = match task {
                AssignedTask::None => "Idle",
                AssignedTask::Gather { .. } => "Gathering",
                AssignedTask::Haul { .. } => "Hauling",
            };
            job_text.0 = format!(
                "Motivation: {:.0}%\nLaziness: {:.0}%\nFatigue: {:.0}%\nTask: {}",
                soul.motivation * 100.0,
                soul.laziness * 100.0,
                soul.fatigue * 100.0,
                task_str
            );
        } else {
            panel_node.display = Display::None;
        }
    } else {
        panel_node.display = Display::None;
    }
}
