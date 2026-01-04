//! UIパネル・メニューモジュール
//!
//! メニューの表示制御、情報パネルの更新、コンテキストメニューの管理を行います。

use crate::entities::damned_soul::DamnedSoul;
use crate::entities::familiar::Familiar;
use crate::interface::ui_setup::*;
use crate::systems::jobs::Blueprint;
use crate::systems::work::AssignedTask;
use bevy::prelude::*;

// ============================================================
// メニュー表示制御
// ============================================================

pub fn menu_visibility_system(
    menu_state: Res<MenuState>,
    mut q_architect: Query<
        &mut Node,
        (
            With<ArchitectSubMenu>,
            Without<ZonesSubMenu>,
            Without<OrdersSubMenu>,
        ),
    >,
    mut q_zones: Query<
        &mut Node,
        (
            With<ZonesSubMenu>,
            Without<ArchitectSubMenu>,
            Without<OrdersSubMenu>,
        ),
    >,
    mut q_orders: Query<
        &mut Node,
        (
            With<OrdersSubMenu>,
            Without<ArchitectSubMenu>,
            Without<ZonesSubMenu>,
        ),
    >,
) {
    if let Ok(mut node) = q_architect.get_single_mut() {
        node.display = if matches!(*menu_state, MenuState::Architect) {
            Display::Flex
        } else {
            Display::None
        };
    }
    if let Ok(mut node) = q_zones.get_single_mut() {
        node.display = if matches!(*menu_state, MenuState::Zones) {
            Display::Flex
        } else {
            Display::None
        };
    }
    if let Ok(mut node) = q_orders.get_single_mut() {
        node.display = if matches!(*menu_state, MenuState::Orders) {
            Display::Flex
        } else {
            Display::None
        };
    }
}

// ============================================================
// 情報パネル更新
// ============================================================

pub fn info_panel_system(
    selected: Res<crate::interface::selection::SelectedEntity>,
    mut q_panel: Query<&mut Node, With<InfoPanel>>,
    mut q_text_job: Query<&mut Text, (With<InfoPanelJobText>, Without<InfoPanelHeader>)>,
    mut q_text_header: Query<&mut Text, (With<InfoPanelHeader>, Without<InfoPanelJobText>)>,
    q_souls: Query<(
        &DamnedSoul,
        &AssignedTask,
        &crate::systems::logistics::Inventory,
        Option<&crate::entities::damned_soul::SoulIdentity>,
    )>,
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

        if let Ok((soul, task, inventory, identity_opt)) = q_souls.get(entity) {
            panel_node.display = Display::Flex;

            let header = if let Some(identity) = identity_opt {
                let gender_icon = match identity.gender {
                    crate::entities::damned_soul::Gender::Male => "♂",
                    crate::entities::damned_soul::Gender::Female => "♀",
                };
                format!("{} {}", identity.name, gender_icon)
            } else {
                "Damned Soul Info".to_string()
            };
            header_text.0 = header;

            let task_str = match task {
                AssignedTask::None => "Idle".to_string(),
                AssignedTask::Gather { phase, .. } => format!("Gather ({:?})", phase),
                AssignedTask::Haul { phase, .. } => format!("Haul ({:?})", phase),
            };

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
            job_text.0 = format!("Type: {:?}\nProgress: {:.0}%", bp.kind, bp.progress * 100.0);
        } else if let Ok(familiar) = q_familiars.get(entity) {
            panel_node.display = Display::Flex;
            header_text.0 = familiar.name.clone();
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

// ============================================================
// コンテキストメニュー管理
// ============================================================

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
        if q_ui_interaction.iter().any(|i| *i != Interaction::None) {
            return;
        }

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
                    commands
                        .spawn((
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
                        ))
                        .with_children(|parent| {
                            parent
                                .spawn((
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
                                ))
                                .with_children(|button| {
                                    button.spawn((
                                        Text::new("Task"),
                                        TextFont {
                                            font_size: 14.0,
                                            ..default()
                                        },
                                        TextColor(Color::WHITE),
                                    ));
                                });
                        });
                }
            }
        }
    }
}
