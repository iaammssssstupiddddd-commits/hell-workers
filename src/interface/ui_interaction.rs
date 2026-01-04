//! UIインタラクションモジュール
//!
//! ツールチップ、モードテキスト、タスクサマリー、およびボタン操作を管理します。

use crate::entities::damned_soul::DamnedSoul;
use crate::entities::familiar::Familiar;
use crate::interface::ui_setup::*;
use crate::systems::work::AssignedTask;
use bevy::prelude::*;

// ============================================================
// システム実装
// ============================================================

pub fn hover_tooltip_system(
    hovered: Res<crate::interface::selection::HoveredEntity>,
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut q_tooltip: Query<&mut Node, With<HoverTooltip>>,
    mut q_text: Query<&mut Text, With<HoverTooltipText>>,
    q_souls: Query<(
        &DamnedSoul,
        &AssignedTask,
        Option<&crate::entities::damned_soul::SoulIdentity>,
    )>,
    q_familiars: Query<&Familiar>,
    q_items: Query<&crate::systems::logistics::ResourceItem>,
    q_trees: Query<&crate::systems::jobs::Tree>,
    q_rocks: Query<&crate::systems::jobs::Rock>,
    q_designations: Query<(
        &crate::systems::jobs::Designation,
        Option<&crate::systems::jobs::IssuedBy>,
        Option<&crate::systems::logistics::ClaimedBy>,
    )>,
) {
    let window = q_window.single();
    let mut tooltip_node = q_tooltip.get_single_mut().unwrap();
    let mut text = q_text.get_single_mut().unwrap();

    if let Some(entity) = hovered.0 {
        let mut info_lines = Vec::new();

        if q_trees.get(entity).is_ok() {
            info_lines.push("Target: Tree".to_string());
        } else if q_rocks.get(entity).is_ok() {
            info_lines.push("Target: Rock".to_string());
        } else if let Ok(item) = q_items.get(entity) {
            info_lines.push(format!("Item: {:?}", item.0));
        } else if let Ok(fam) = q_familiars.get(entity) {
            info_lines.push(format!("Familiar: {}", fam.name));
        } else if let Ok((soul, _, identity_opt)) = q_souls.get(entity) {
            let name = identity_opt
                .map(|i| i.name.clone())
                .unwrap_or("Soul".to_string());
            info_lines.push(format!("Soul: {}", name));
            info_lines.push(format!("Motivation: {:.0}%", soul.motivation * 100.0));
        }

        if let Ok((des, issued_by_opt, claimed_by_opt)) = q_designations.get(entity) {
            info_lines.push(format!("Task: {:?}", des.work_type));

            if let Some(issued_by) = issued_by_opt {
                if let Ok(fam) = q_familiars.get(issued_by.0) {
                    info_lines.push(format!("Issued by: {}", fam.name));
                }
            }

            if let Some(claimed_by) = claimed_by_opt {
                if let Ok((_, _, identity_opt)) = q_souls.get(claimed_by.0) {
                    let name = identity_opt
                        .map(|i| i.name.clone())
                        .unwrap_or("Unknown".to_string());
                    info_lines.push(format!("Assigned to: {}", name));
                }
            }
        }

        if !info_lines.is_empty() {
            text.0 = info_lines.join("\n");
            tooltip_node.display = Display::Flex;

            if let Some(cursor_pos) = window.cursor_position() {
                tooltip_node.left = Val::Px(cursor_pos.x + 15.0);
                tooltip_node.top = Val::Px(cursor_pos.y + 15.0);
            }
        } else {
            tooltip_node.display = Display::None;
        }
    } else {
        tooltip_node.display = Display::None;
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
                crate::systems::command::TaskMode::DesignateChop(None) => {
                    "Mode: Chop (Drag to select)".to_string()
                }
                crate::systems::command::TaskMode::DesignateChop(Some(_)) => {
                    "Mode: Chop (Dragging...)".to_string()
                }
                crate::systems::command::TaskMode::DesignateMine(None) => {
                    "Mode: Mine (Drag to select)".to_string()
                }
                crate::systems::command::TaskMode::DesignateMine(Some(_)) => {
                    "Mode: Mine (Dragging...)".to_string()
                }
                crate::systems::command::TaskMode::DesignateHaul(None) => {
                    "Mode: Haul (Drag to select)".to_string()
                }
                crate::systems::command::TaskMode::DesignateHaul(Some(_)) => {
                    "Mode: Haul (Dragging...)".to_string()
                }
                crate::systems::command::TaskMode::CancelDesignation(None) => {
                    "Mode: Cancel (Drag to select)".to_string()
                }
                crate::systems::command::TaskMode::CancelDesignation(Some(_)) => {
                    "Mode: Cancel (Dragging...)".to_string()
                }
                crate::systems::command::TaskMode::SelectBuildTarget => {
                    "Mode: Build Select".to_string()
                }
                crate::systems::command::TaskMode::AreaSelection(None) => {
                    "Mode: Area (Click start)".to_string()
                }
                crate::systems::command::TaskMode::AreaSelection(Some(_)) => {
                    "Mode: Area (Click end)".to_string()
                }
                crate::systems::command::TaskMode::AssignTask(None) => {
                    "Mode: Assign Task (Drag to select)".to_string()
                }
                crate::systems::command::TaskMode::AssignTask(Some(_)) => {
                    "Mode: Assign Task (Dragging...)".to_string()
                }
            }
        };
        text.0 = mode_str;
    }
}

pub fn task_summary_ui_system(
    task_queue: Res<crate::systems::work::TaskQueue>,
    mut q_text: Query<&mut Text, With<TaskSummaryText>>,
) {
    if let Ok(mut text) = q_text.get_single_mut() {
        let mut total = 0;
        let mut high = 0;
        for tasks in task_queue.by_familiar.values() {
            total += tasks.len();
            high += tasks.iter().filter(|t| t.priority > 0).count();
        }
        text.0 = format!("Tasks: {} ({} High)", total, high);
    }
}

pub fn ui_interaction_system(
    mut interaction_query: Query<
        (&Interaction, &MenuButton, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
    mut menu_state: ResMut<MenuState>,
    mut build_mode: ResMut<crate::interface::selection::BuildMode>,
    mut zone_mode: ResMut<crate::systems::logistics::ZoneMode>,
    mut task_mode: ResMut<crate::systems::command::TaskMode>,
    q_context_menu: Query<Entity, With<ContextMenu>>,
    mut commands: Commands,
) {
    for (interaction, menu_button, mut color) in interaction_query.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
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
