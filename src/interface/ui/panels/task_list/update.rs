//! タスクリストの動的更新、タブ切り替え、クリックハンドリング

use crate::interface::camera::MainCamera;
use crate::interface::ui::components::{
    InfoPanel, RightPanelMode, TaskListBody, TaskListItem, TaskListPanel, TaskListTabButton,
};
use crate::interface::ui::panels::info_panel::InfoPanelPinState;
use crate::relationships::TaskWorkers;
use crate::systems::jobs::{
    Blueprint, BuildingType, Designation, Priority, Rock, SandPile, Tree, WorkType,
};
use crate::systems::logistics::transport_request::{TransportRequest, TransportRequestKind};
use crate::systems::logistics::ResourceItem;
use bevy::prelude::*;
use std::collections::BTreeMap;

// ============================================================
// ViewModel for diff detection
// ============================================================

#[derive(Clone, PartialEq)]
struct TaskEntry {
    entity: Entity,
    description: String,
    priority: u32,
    worker_count: usize,
}

#[derive(Resource, Default)]
pub struct TaskListState {
    last_snapshot: Vec<(WorkType, Vec<TaskEntry>)>,
}

// ============================================================
// WorkType display helper
// ============================================================

fn work_type_label(wt: &WorkType) -> &'static str {
    match wt {
        WorkType::Chop => "Chop",
        WorkType::Mine => "Mine",
        WorkType::Build => "Build",
        WorkType::Haul => "Haul",
        WorkType::HaulToMixer => "Haul (Mixer)",
        WorkType::GatherWater => "Water",
        WorkType::CollectSand => "Sand",
        WorkType::Refine => "Refine",
        WorkType::HaulWaterToMixer => "Water (Mixer)",
        WorkType::WheelbarrowHaul => "Wheelbarrow",
    }
}


fn generate_task_description(
    wt: WorkType,
    entity: Entity,
    blueprint: Option<&Blueprint>,
    transport_req: Option<&TransportRequest>,
    resource_item: Option<&ResourceItem>,
    tree: Option<&Tree>,
    rock: Option<&Rock>,
    sand_pile: Option<&SandPile>,
) -> String {
    match wt {
        WorkType::Build => {
            if let Some(bp) = blueprint {
                match bp.kind {
                    BuildingType::Wall => "Construct Wall".to_string(),
                    BuildingType::Floor => "Construct Floor".to_string(),
                    BuildingType::Tank => "Construct Tank".to_string(),
                    BuildingType::MudMixer => "Construct Mixer".to_string(),
                    BuildingType::SandPile => "Construct SandPile".to_string(),
                    BuildingType::WheelbarrowParking => "Construct Parking".to_string(),
                }
            } else {
                format!("Construct {:?}", entity)
            }
        }
        WorkType::Mine => {
            if rock.is_some() {
                "Mine Rock".to_string()
            } else {
                "Mine".to_string()
            }
        }
        WorkType::Chop => {
            if tree.is_some() {
                "Chop Tree".to_string()
            } else {
                "Chop".to_string()
            }
        }
        WorkType::Haul => {
            if let Some(req) = transport_req {
                if req.kind == TransportRequestKind::DeliverToBlueprint {
                    format!("Haul {:?} to Build", req.resource_type)
                } else {
                    format!("Haul {:?} (Req)", req.resource_type)
                }
            } else if let Some(item) = resource_item {
                format!("Haul {:?}", item.0)
            } else {
                "Haul".to_string()
            }
        }
        WorkType::HaulToMixer => {
            if let Some(req) = transport_req {
                format!("Haul {:?} to Mixer", req.resource_type)
            } else {
                "Haul to Mixer".to_string()
            }
        }
        WorkType::HaulWaterToMixer => "Haul Water to Mixer".to_string(),
        WorkType::GatherWater => "Gather Water".to_string(),
        WorkType::CollectSand => {
            if sand_pile.is_some() {
                "Collect Sand".to_string()
            } else {
                "Collect Sand".to_string()
            }
        }
        WorkType::Refine => "Refine".to_string(),
        WorkType::WheelbarrowHaul => "Wheelbarrow Haul".to_string(),
    }
}

// ============================================================
// タスクリスト更新システム
// ============================================================

pub fn task_list_update_system(
    mut commands: Commands,
    game_assets: Res<crate::assets::GameAssets>,
    theme: Res<crate::interface::ui::theme::UiTheme>,
    mode: Res<RightPanelMode>,
    mut state: ResMut<TaskListState>,
    designations: Query<(
        Entity,
        &Transform,
        &Designation,
        Option<&Priority>,
        Option<&TaskWorkers>,
        Option<&Blueprint>,
        Option<&TransportRequest>,
        Option<&ResourceItem>,
        Option<&Tree>,
        Option<&Rock>,
        Option<&SandPile>,
    )>,
    body_query: Query<Entity, With<TaskListBody>>,
    children_query: Query<&Children>,
) {
    if *mode != RightPanelMode::TaskList {
        return;
    }

    // Build snapshot grouped by WorkType
    let mut groups: BTreeMap<u8, (WorkType, Vec<TaskEntry>)> = BTreeMap::new();
    for (
        entity,
        _transform,
        designation,
        priority,
        workers,
        blueprint,
        transport_req,
        resource_item,
        tree,
        rock,
        sand_pile,
    ) in &designations
    {
        let wt = designation.work_type;
        let key = wt as u8;

        // Generate description
        let description = generate_task_description(
            wt,
            entity,
            blueprint,
            transport_req,
            resource_item,
            tree,
            rock,
            sand_pile,
        );

        let entry = TaskEntry {
            entity,
            description,
            priority: priority.map_or(0, |p| p.0),
            worker_count: workers.map_or(0, |w| w.iter().count()),
        };
        groups.entry(key).or_insert_with(|| (wt, Vec::new())).1.push(entry);
    }

    let snapshot: Vec<(WorkType, Vec<TaskEntry>)> = groups.into_values().collect();

    if snapshot == state.last_snapshot {
        return;
    }
    state.last_snapshot = snapshot.clone();

    // Rebuild UI
    let Ok(body_entity) = body_query.single() else {
        return;
    };

    // Despawn existing children
    if let Ok(children) = children_query.get(body_entity) {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }

    commands.entity(body_entity).with_children(|parent| {
        if snapshot.is_empty() {
            parent.spawn((
                Text::new("No designations"),
                TextFont {
                    font: game_assets.font_ui.clone(),
                    font_size: theme.typography.font_size_small,
                    ..default()
                },
                TextColor(theme.colors.empty_text),
            ));
            return;
        }

        for (work_type, entries) in &snapshot {
            // Group header
            parent
                .spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    margin: UiRect {
                        top: Val::Px(4.0),
                        bottom: Val::Px(2.0),
                        ..default()
                    },
                    column_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((
                        Text::new(format!(
                            "{} ({})",
                            work_type_label(work_type),
                            entries.len()
                        )),
                        TextFont {
                            font: game_assets.font_ui.clone(),
                            font_size: theme.typography.font_size_xs,
                            weight: FontWeight::SEMIBOLD,
                            ..default()
                        },
                        TextColor(theme.colors.text_secondary_semantic),
                    ));
                });

            for entry in entries {
                // Status indicator
                let status_text = if entry.worker_count > 0 {
                    format!("[RUN:{}]", entry.worker_count)
                } else {
                    "[WAIT]".to_string()
                };

                // Priority indicator
                let priority_display = if entry.priority >= 5 {
                    format!("★[P:{}] ", entry.priority)
                } else if (entry.priority as i32) < 0 {
                    format!("▼[P:{}] ", entry.priority)
                } else {
                    format!("[P:{}] ", entry.priority)
                };

                parent
                    .spawn((
                        Button,
                        Node {
                            width: Val::Percent(100.0),
                            padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                            ..default()
                        },
                        BackgroundColor(theme.colors.list_item_default),
                        TaskListItem(entry.entity),
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new(format!(
                                "{}{}{}",
                                priority_display, status_text, entry.description
                            )),
                            TextFont {
                                font: game_assets.font_ui.clone(),
                                font_size: theme.typography.font_size_small,
                                ..default()
                            },
                            TextColor(theme.colors.text_primary),
                        ));
                    });
            }
        }
    });
}

// ============================================================
// タブ切り替えシステム
// ============================================================

pub fn right_panel_tab_system(
    mut mode: ResMut<RightPanelMode>,
    theme: Res<crate::interface::ui::theme::UiTheme>,
    interactions: Query<(&Interaction, &TaskListTabButton), Changed<Interaction>>,
    tab_buttons: Query<(Entity, &TaskListTabButton, &Children)>,
    mut text_colors: Query<&mut TextColor>,
    mut border_colors: Query<&mut BorderColor>,
) {
    // Handle clicks
    for (interaction, tab) in &interactions {
        if *interaction == Interaction::Pressed && *mode != tab.0 {
            *mode = tab.0;
        }
    }

    // Update visual state of all tab buttons
    if mode.is_changed() {
        for (button_entity, tab, children) in &tab_buttons {
            let is_active = tab.0 == *mode;

            // Update text color
            if let Some(child) = children.iter().next() {
                if let Ok(mut color) = text_colors.get_mut(child) {
                    color.0 = if is_active {
                        theme.colors.text_accent_semantic
                    } else {
                        theme.colors.text_secondary_semantic
                    };
                }
            }

            // Update border (underline)
            if let Ok(mut border) = border_colors.get_mut(button_entity) {
                *border = BorderColor::all(if is_active {
                    theme.colors.text_accent_semantic
                } else {
                    Color::NONE
                });
            }
        }
    }
}

// ============================================================
// パネル表示切替システム
// ============================================================

pub fn right_panel_visibility_system(
    mode: Res<RightPanelMode>,
    mut info_panels: Query<&mut Node, (With<InfoPanel>, Without<TaskListPanel>)>,
    mut task_panels: Query<&mut Node, (With<TaskListPanel>, Without<InfoPanel>)>,
    selected: Res<crate::interface::selection::SelectedEntity>,
    pin_state: Res<InfoPanelPinState>,
) {
    if !mode.is_changed() && !selected.is_changed() && !pin_state.is_changed() {
        return;
    }

    match *mode {
        RightPanelMode::Info => {
            // InfoPanel's own visibility is handled by info_panel_system
            // Just hide task list
            for mut node in &mut task_panels {
                if node.display != Display::None {
                    node.display = Display::None;
                }
            }
        }
        RightPanelMode::TaskList => {
            // Hide info panel, show task list
            for mut node in &mut info_panels {
                if node.display != Display::None {
                    node.display = Display::None;
                }
            }
            for mut node in &mut task_panels {
                if node.display != Display::Flex {
                    node.display = Display::Flex;
                }
            }
        }
    }
}

// ============================================================
// クリックハンドリング
// ============================================================

pub fn task_list_click_system(
    mut mode: ResMut<RightPanelMode>,
    mut pin_state: ResMut<InfoPanelPinState>,
    interactions: Query<(&Interaction, &TaskListItem), Changed<Interaction>>,
    target_transforms: Query<&GlobalTransform, Without<MainCamera>>,
    mut camera_query: Query<&mut Transform, With<MainCamera>>,
) {
    for (interaction, item) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }

        let target_entity = item.0;

        // Get target world position and move camera
        if let Ok(target_gt) = target_transforms.get(target_entity) {
            let target_pos = target_gt.translation();
            if let Ok(mut cam_transform) = camera_query.single_mut() {
                cam_transform.translation.x = target_pos.x;
                cam_transform.translation.y = target_pos.y;
            }
        }

        // Pin info panel to this entity and switch to Info mode
        pin_state.entity = Some(target_entity);
        *mode = RightPanelMode::Info;
    }
}
