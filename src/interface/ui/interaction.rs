//! UIインタラクションモジュール
//!
//! ツールチップ、モードテキスト、タスクサマリー、およびボタン操作を管理します。

use crate::entities::damned_soul::DamnedSoul;
use crate::entities::familiar::{Familiar, FamiliarOperation};
use crate::game_state::{BuildContext, PlayMode, TaskContext, ZoneContext};
use crate::interface::ui::components::*;
use crate::relationships::TaskWorkers;
use crate::systems::soul_ai::task_execution::AssignedTask;
use bevy::prelude::*;
use std::time::Duration;

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
        Option<&TaskWorkers>,
    )>,
    q_buildings: Query<(
        &crate::systems::jobs::Building,
        Option<&crate::systems::logistics::Stockpile>,
        Option<&crate::relationships::StoredItems>,
    )>,
) {
    let Ok(window) = q_window.single() else {
        return;
    };
    let Ok(mut tooltip_node) = q_tooltip.single_mut() else {
        return;
    };
    let Ok(mut text) = q_text.single_mut() else {
        return;
    };

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
        } else if let Ok((building, stockpile_opt, stored_items_opt)) = q_buildings.get(entity) {
            let mut building_info = format!("Building: {:?}", building._kind);
            if let Some(stockpile) = stockpile_opt {
                let current = stored_items_opt.map(|si| si.len()).unwrap_or(0);
                let resource_name = stockpile
                    .resource_type
                    .map(|r| format!("{:?}", r))
                    .unwrap_or_else(|| "Items".to_string());
                building_info = format!(
                    "{}: {} ({}/{})",
                    building_info, resource_name, current, stockpile.capacity
                );
            }
            info_lines.push(building_info);
        }

        if let Ok((des, issued_by_opt, task_workers_opt)) = q_designations.get(entity) {
            info_lines.push(format!("Task: {:?}", des.work_type));

            if let Some(issued_by) = issued_by_opt {
                if let Ok(fam) = q_familiars.get(issued_by.0) {
                    info_lines.push(format!("Issued by: {}", fam.name));
                }
            }

            // TaskWorkers から作業者を表示
            if let Some(workers) = task_workers_opt {
                let worker_names: Vec<String> = workers
                    .iter()
                    .filter_map(|&soul_entity| {
                        q_souls.get(soul_entity).ok().map(|(_, _, identity_opt)| {
                            identity_opt
                                .map(|i| i.name.clone())
                                .unwrap_or("Unknown".to_string())
                        })
                    })
                    .collect();

                if !worker_names.is_empty() {
                    info_lines.push(format!("Assigned to: {}", worker_names.join(", ")));
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
    play_mode: Res<State<PlayMode>>,
    build_context: Res<BuildContext>,
    zone_context: Res<ZoneContext>,
    task_context: Res<TaskContext>,
    mut q_text: Query<&mut Text, With<ModeText>>,
) {
    if let Ok(mut text) = q_text.single_mut() {
        let mode_str = match *play_mode.get() {
            PlayMode::Normal => "Mode: Normal".to_string(),
            PlayMode::BuildingPlace => {
                if let Some(kind) = build_context.0 {
                    format!("Mode: Build ({:?})", kind)
                } else {
                    "Mode: Build".to_string()
                }
            }
            PlayMode::ZonePlace => {
                if let Some(kind) = zone_context.0 {
                    format!("Mode: Zone ({:?})", kind)
                } else {
                    "Mode: Zone".to_string()
                }
            }
            PlayMode::TaskDesignation => match task_context.0 {
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
                crate::systems::command::TaskMode::AreaSelection(_) => {
                    "Mode: Area Selection".to_string()
                }
                crate::systems::command::TaskMode::AssignTask(_) => "Mode: Assign Task".to_string(),
                _ => "Mode: Task".to_string(),
            },
        };
        text.0 = mode_str;
    }
}

pub fn task_summary_ui_system(
    q_designations: Query<&crate::systems::jobs::Priority, With<crate::systems::jobs::Designation>>,
    mut q_text: Query<&mut Text, With<TaskSummaryText>>,
) {
    if let Ok(mut text) = q_text.single_mut() {
        let total = q_designations.iter().count();
        let high = q_designations.iter().filter(|p| p.0 > 0).count();
        text.0 = format!("Tasks: {} ({} High)", total, high);
    }
}

#[derive(Default)]
pub struct FpsCounter {
    pub frame_count: u32,
    pub elapsed_time: Duration,
}

pub fn update_fps_display_system(
    time: Res<Time>,
    mut fps_counter: Local<FpsCounter>,
    mut q_text: Query<&mut Text, With<FpsText>>,
) {
    fps_counter.elapsed_time += time.delta();
    fps_counter.frame_count += 1;

    // 1秒ごとにFPSを更新
    if fps_counter.elapsed_time >= Duration::from_secs(1) {
        if let Ok(mut text) = q_text.single_mut() {
            let fps = fps_counter.frame_count as f32 / fps_counter.elapsed_time.as_secs_f32();
            text.0 = format!("FPS: {:.0}", fps);
            fps_counter.frame_count = 0;
            fps_counter.elapsed_time = Duration::ZERO;
        }
    }
}

/// UI ボタンの操作を管理する統合システム
pub fn ui_interaction_system(
    mut interaction_query: Query<
        (&Interaction, &MenuButton, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
    mut menu_state: ResMut<MenuState>,
    mut next_play_mode: ResMut<NextState<PlayMode>>,
    mut build_context: ResMut<BuildContext>,
    mut zone_context: ResMut<ZoneContext>,
    mut task_context: ResMut<TaskContext>,
    selected_entity: Res<crate::interface::selection::SelectedEntity>,
    mut q_familiar_ops: Query<&mut FamiliarOperation>,
    mut q_dialog: Query<&mut Node, With<OperationDialog>>,
    q_context_menu: Query<Entity, With<ContextMenu>>,
    mut commands: Commands,
    mut ev_max_soul_changed: MessageWriter<crate::events::FamiliarOperationMaxSoulChangedEvent>,
) {
    for (interaction, menu_button, mut color) in interaction_query.iter_mut() {
        // 視覚的フィードバック
        match *interaction {
            Interaction::Pressed => {
                *color = BackgroundColor(Color::srgb(0.5, 0.5, 0.5));
            }
            Interaction::Hovered => {
                *color = BackgroundColor(Color::srgb(0.4, 0.4, 0.4));
            }
            Interaction::None => {
                *color = BackgroundColor(Color::srgb(0.2, 0.2, 0.2));
            }
        }

        // 実際の処理
        if *interaction == Interaction::Pressed {
            // コンテキストメニューがあれば消す
            for entity in q_context_menu.iter() {
                commands.entity(entity).despawn();
            }

            match menu_button.0 {
                MenuAction::ToggleArchitect => {
                    let current = *menu_state;
                    *menu_state = match current {
                        MenuState::Architect => MenuState::Hidden,
                        _ => MenuState::Architect,
                    };
                    // 他モードをクリア
                    build_context.0 = None;
                    zone_context.0 = None;
                    next_play_mode.set(PlayMode::Normal);
                }
                MenuAction::ToggleOrders => {
                    let current = *menu_state;
                    *menu_state = match current {
                        MenuState::Orders => MenuState::Hidden,
                        _ => MenuState::Orders,
                    };
                    // 他モードをクリア
                    build_context.0 = None;
                    zone_context.0 = None;
                    task_context.0 = crate::systems::command::TaskMode::None;
                    next_play_mode.set(PlayMode::Normal);
                }
                MenuAction::ToggleZones => {
                    let current = *menu_state;
                    *menu_state = match current {
                        MenuState::Zones => MenuState::Hidden,
                        _ => MenuState::Zones,
                    };
                    // 他モードをクリア
                    build_context.0 = None;
                    zone_context.0 = None;
                    task_context.0 = crate::systems::command::TaskMode::None;
                    next_play_mode.set(PlayMode::Normal);
                }
                MenuAction::SelectBuild(kind) => {
                    // 他モードをクリア
                    zone_context.0 = None;
                    task_context.0 = crate::systems::command::TaskMode::None;
                    // Build設定
                    build_context.0 = Some(kind);
                    next_play_mode.set(PlayMode::BuildingPlace);
                    info!(
                        "UI: Build mode set to {:?}, PlayMode -> BuildingPlace",
                        kind
                    );
                }
                MenuAction::SelectZone(kind) => {
                    // 他モードをクリア
                    build_context.0 = None;
                    task_context.0 = crate::systems::command::TaskMode::None;
                    // Zone設定
                    zone_context.0 = Some(kind);
                    next_play_mode.set(PlayMode::ZonePlace);
                    info!("UI: Zone mode set to {:?}, PlayMode -> ZonePlace", kind);
                }
                MenuAction::SelectTaskMode(mode) => {
                    // 他モードをクリア
                    build_context.0 = None;
                    zone_context.0 = None;
                    // Task設定
                    task_context.0 = mode;
                    next_play_mode.set(PlayMode::TaskDesignation);
                    info!(
                        "UI: TaskMode set to {:?}, PlayMode -> TaskDesignation",
                        mode
                    );
                }
                MenuAction::SelectAreaTask => {
                    let mode = crate::systems::command::TaskMode::AreaSelection(None);
                    // 他モードをクリア
                    build_context.0 = None;
                    zone_context.0 = None;
                    // Task設定
                    task_context.0 = mode;
                    next_play_mode.set(PlayMode::TaskDesignation);
                    info!("UI: Area Selection Mode entered, PlayMode -> TaskDesignation");
                }
                MenuAction::OpenOperationDialog => {
                    if let Ok(mut dialog_node) = q_dialog.single_mut() {
                        dialog_node.display = Display::Flex;
                    }
                }
                MenuAction::CloseDialog => {
                    if let Ok(mut dialog_node) = q_dialog.single_mut() {
                        dialog_node.display = Display::None;
                    }
                }
                MenuAction::AdjustFatigueThreshold(delta) => {
                    if let Some(selected) = selected_entity.0 {
                        if let Ok(mut op) = q_familiar_ops.get_mut(selected) {
                            let new_val = (op.fatigue_threshold + delta).clamp(0.0, 1.0);
                            op.fatigue_threshold = (new_val * 10.0).round() / 10.0;
                        }
                    }
                }
                MenuAction::AdjustMaxControlledSoul(delta) => {
                    if let Some(selected) = selected_entity.0 {
                        if let Ok(mut op) = q_familiar_ops.get_mut(selected) {
                            let old_val = op.max_controlled_soul;
                            let new_val = (old_val as isize + delta).clamp(1, 8) as usize;
                            op.max_controlled_soul = new_val;

                            // 値が変更された場合のみイベントを発火
                            if old_val != new_val {
                                ev_max_soul_changed.write(
                                    crate::events::FamiliarOperationMaxSoulChangedEvent {
                                        familiar_entity: selected,
                                        old_value: old_val,
                                        new_value: new_val,
                                    },
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Operation Dialog のテキスト表示を更新するシステム
pub fn update_operation_dialog_system(
    selected_entity: Res<crate::interface::selection::SelectedEntity>,
    q_familiars: Query<(&Familiar, &FamiliarOperation)>,
    mut text_set: ParamSet<(
        Query<&mut Text, With<OperationDialogFamiliarName>>,
        Query<&mut Text, With<OperationDialogThresholdText>>,
        Query<&mut Text, With<OperationDialogMaxSoulText>>,
    )>,
) {
    if let Some(selected) = selected_entity.0 {
        if let Ok((familiar, op)) = q_familiars.get(selected) {
            if let Ok(mut name_text) = text_set.p0().single_mut() {
                name_text.0 = format!("Editing: {}", familiar.name);
            }
            if let Ok(mut threshold_text) = text_set.p1().single_mut() {
                let val_str = format!("{:.0}%", op.fatigue_threshold * 100.0);
                if threshold_text.0 != val_str {
                    threshold_text.0 = val_str;
                }
            }
            if let Ok(mut max_soul_text) = text_set.p2().single_mut() {
                let val_str = format!("{}", op.max_controlled_soul);
                if max_soul_text.0 != val_str {
                    max_soul_text.0 = val_str;
                }
            }
        }
    }
}
