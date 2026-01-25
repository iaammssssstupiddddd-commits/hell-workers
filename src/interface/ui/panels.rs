//! UIパネル・メニューモジュール
//!
//! メニューの表示制御、情報パネルの更新、コンテキストメニューの管理を行います。

use crate::entities::damned_soul::DamnedSoul;
use crate::entities::familiar::Familiar;
use crate::interface::ui::components::*;
use crate::systems::jobs::Blueprint;
use crate::systems::soul_ai::task_execution::AssignedTask;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

#[derive(SystemParam)]
pub struct InfoPanelParams<'w, 's> {
    pub q_header: Query<
        'w,
        's,
        &'static mut Text,
        (
            With<InfoPanelHeader>,
            Without<InfoPanelCommonText>,
            Without<InfoPanelStatMotivation>,
            Without<InfoPanelStatStress>,
            Without<InfoPanelStatFatigue>,
            Without<InfoPanelTaskText>,
            Without<InfoPanelInventoryText>,
        ),
    >,
    pub q_gender_icon: Query<
        'w,
        's,
        (&'static mut ImageNode, &'static mut Node),
        (With<InfoPanelGenderIcon>, Without<InfoPanel>),
    >,
    pub q_stat_motivation: Query<
        'w,
        's,
        &'static mut Text,
        (
            With<InfoPanelStatMotivation>,
            Without<InfoPanelHeader>,
            Without<InfoPanelCommonText>,
            Without<InfoPanelStatStress>,
            Without<InfoPanelStatFatigue>,
            Without<InfoPanelTaskText>,
            Without<InfoPanelInventoryText>,
        ),
    >,
    pub q_stat_stress: Query<
        'w,
        's,
        &'static mut Text,
        (
            With<InfoPanelStatStress>,
            Without<InfoPanelHeader>,
            Without<InfoPanelCommonText>,
            Without<InfoPanelStatMotivation>,
            Without<InfoPanelStatFatigue>,
            Without<InfoPanelTaskText>,
            Without<InfoPanelInventoryText>,
        ),
    >,
    pub q_stat_fatigue: Query<
        'w,
        's,
        &'static mut Text,
        (
            With<InfoPanelStatFatigue>,
            Without<InfoPanelHeader>,
            Without<InfoPanelCommonText>,
            Without<InfoPanelStatMotivation>,
            Without<InfoPanelStatStress>,
            Without<InfoPanelTaskText>,
            Without<InfoPanelInventoryText>,
        ),
    >,
    pub q_task: Query<
        'w,
        's,
        &'static mut Text,
        (
            With<InfoPanelTaskText>,
            Without<InfoPanelHeader>,
            Without<InfoPanelCommonText>,
            Without<InfoPanelStatMotivation>,
            Without<InfoPanelStatStress>,
            Without<InfoPanelStatFatigue>,
            Without<InfoPanelInventoryText>,
        ),
    >,
    pub q_inv: Query<
        'w,
        's,
        &'static mut Text,
        (
            With<InfoPanelInventoryText>,
            Without<InfoPanelHeader>,
            Without<InfoPanelCommonText>,
            Without<InfoPanelStatMotivation>,
            Without<InfoPanelStatStress>,
            Without<InfoPanelStatFatigue>,
            Without<InfoPanelTaskText>,
        ),
    >,
    pub q_common: Query<
        'w,
        's,
        &'static mut Text,
        (
            With<InfoPanelCommonText>,
            Without<InfoPanelHeader>,
            Without<InfoPanelStatMotivation>,
            Without<InfoPanelStatStress>,
            Without<InfoPanelStatFatigue>,
            Without<InfoPanelTaskText>,
            Without<InfoPanelInventoryText>,
        ),
    >,
}

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
    if let Ok(mut node) = q_architect.single_mut() {
        node.display = if matches!(*menu_state, MenuState::Architect) {
            Display::Flex
        } else {
            Display::None
        };
    }
    if let Ok(mut node) = q_zones.single_mut() {
        node.display = if matches!(*menu_state, MenuState::Zones) {
            Display::Flex
        } else {
            Display::None
        };
    }
    if let Ok(mut node) = q_orders.single_mut() {
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
    game_assets: Res<crate::assets::GameAssets>,
    selected: Res<crate::interface::selection::SelectedEntity>,
    mut q_panel: Query<&mut Node, (With<InfoPanel>, Without<InfoPanelGenderIcon>)>,
    mut params: InfoPanelParams,
    q_souls: Query<(
        &DamnedSoul,
        &AssignedTask,
        Option<&crate::systems::logistics::Inventory>,
        Option<&crate::entities::damned_soul::SoulIdentity>,
    )>,
    q_blueprints: Query<&Blueprint>,
    q_familiars: Query<(&Familiar, &crate::entities::familiar::FamiliarOperation)>,
    q_items: Query<&crate::systems::logistics::ResourceItem>,
    q_trees: Query<&crate::systems::jobs::Tree>,
    q_rocks: Query<&crate::systems::jobs::Rock>,
) {
    let Ok(mut panel_node) = q_panel.single_mut() else {
        return;
    };
    panel_node.display = Display::None;

    // Reset common text and gender icon
    if let Ok(mut common) = params.q_common.single_mut() {
        common.0 = "".to_string();
    }
    if let Ok((_icon, mut node)) = params.q_gender_icon.single_mut() {
        node.display = Display::None;
    }

    if let Some(entity) = selected.0 {
        if let Ok((soul, task, inventory_opt, identity_opt)) = q_souls.get(entity) {
            panel_node.display = Display::Flex;

            if let Ok(mut header) = params.q_header.single_mut() {
                header.0 = if let Some(identity) = identity_opt {
                    identity.name.clone()
                } else {
                    "Damned Soul".to_string()
                };
            }

            if let Some(identity) = identity_opt {
                if let Ok((mut icon, mut node)) = params.q_gender_icon.single_mut() {
                    node.display = Display::Flex;
                    icon.image = match identity.gender {
                        crate::entities::damned_soul::Gender::Male => game_assets.icon_male.clone(),
                        crate::entities::damned_soul::Gender::Female => {
                            game_assets.icon_female.clone()
                        }
                    };
                }
            }

            if let Ok(mut t) = params.q_stat_motivation.single_mut() {
                t.0 = format!("Motivation: {:.0}%", soul.motivation * 100.0);
            }
            if let Ok(mut t) = params.q_stat_stress.single_mut() {
                t.0 = format!("Stress: {:.0}%", soul.stress * 100.0);
            }
            if let Ok(mut t) = params.q_stat_fatigue.single_mut() {
                t.0 = format!("Fatigue: {:.0}%", soul.fatigue * 100.0);
            }

            let task_str = match task {
                AssignedTask::None => "Idle".to_string(),
                AssignedTask::Gather { phase, .. } => format!("Gather ({:?})", phase),
                AssignedTask::Haul { phase, .. } => format!("Haul ({:?})", phase),
                AssignedTask::HaulToBlueprint { phase, .. } => format!("HaulToBp ({:?})", phase),
                AssignedTask::Build { phase, .. } => format!("Build ({:?})", phase),
                AssignedTask::GatherWater { phase, .. } => format!("GatherWater ({:?})", phase),
            };
            if let Ok(mut t) = params.q_task.single_mut() {
                t.0 = format!("Task: {}", task_str);
            }

            let inv_str = if let Some(crate::systems::logistics::Inventory(Some(item_entity))) = inventory_opt {
                if let Ok(item) = q_items.get(*item_entity) {
                    format!("Carrying: {:?}", item.0)
                } else {
                    format!("Carrying: Entity {:?}", item_entity)
                }
            } else {
                "Carrying: None".to_string()
            };
            if let Ok(mut t) = params.q_inv.single_mut() {
                t.0 = inv_str;
            }
        } else if let Ok(mut common) = params.q_common.single_mut() {
            if let Ok(bp) = q_blueprints.get(entity) {
                panel_node.display = Display::Flex;
                if let Ok(mut header) = params.q_header.single_mut() {
                    header.0 = "Blueprint Info".to_string();
                }
                common.0 = format!("Type: {:?}\nProgress: {:.0}%", bp.kind, bp.progress * 100.0);
            } else if let Ok((familiar, op)) = q_familiars.get(entity) {
                panel_node.display = Display::Flex;
                if let Ok(mut header) = params.q_header.single_mut() {
                    header.0 = familiar.name.clone();
                }
                common.0 = format!(
                    "Type: {:?}\nRange: {:.0} tiles\nFatigue Threshold: {:.0}%",
                    familiar.familiar_type,
                    familiar.command_radius / 16.0,
                    op.fatigue_threshold * 100.0
                );
            } else if let Ok(item) = q_items.get(entity) {
                panel_node.display = Display::Flex;
                if let Ok(mut header) = params.q_header.single_mut() {
                    header.0 = "Resource Item".to_string();
                }
                common.0 = format!("Type: {:?}", item.0);
            } else if let Ok(_) = q_trees.get(entity) {
                panel_node.display = Display::Flex;
                if let Ok(mut header) = params.q_header.single_mut() {
                    header.0 = "Tree".to_string();
                }
                common.0 = "Natural resource: Wood".to_string();
            } else if let Ok(_) = q_rocks.get(entity) {
                panel_node.display = Display::Flex;
                if let Ok(mut header) = params.q_header.single_mut() {
                    header.0 = "Rock".to_string();
                }
                common.0 = "Natural resource: Stone".to_string();
            }
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
            commands.entity(entity).despawn();
        }

        let Ok((camera, camera_transform)) = q_camera.single() else {
            return;
        };
        let Ok(window) = q_window.single() else {
            return;
        };

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
                                width: Val::Px(100.0),
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
                            parent
                                .spawn((
                                    Button,
                                    Node {
                                        width: Val::Percent(100.0),
                                        height: Val::Px(30.0),
                                        justify_content: JustifyContent::Center,
                                        align_items: AlignItems::Center,
                                        margin: UiRect::top(Val::Px(2.0)),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgb(0.3, 0.3, 0.3)),
                                    MenuButton(MenuAction::OpenOperationDialog),
                                ))
                                .with_children(|button| {
                                    button.spawn((
                                        Text::new("Operation"),
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
