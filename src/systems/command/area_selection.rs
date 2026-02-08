use super::{AreaEditHandleKind, AreaSelectionIndicator, TaskArea, TaskMode};
use crate::constants::TILE_SIZE;
use crate::entities::damned_soul::Destination;
use crate::entities::familiar::{ActiveCommand, Familiar, FamiliarCommand};
use crate::game_state::{PlayMode, TaskContext};
use crate::interface::camera::MainCamera;
use crate::interface::selection::SelectedEntity;
use crate::interface::ui::UiInputState;
use crate::systems::jobs::{Designation, Rock, Tree, WorkType};
use crate::systems::logistics::ResourceItem;
use crate::world::map::WorldMap;
use bevy::prelude::*;
use bevy::window::{CursorIcon, PrimaryWindow, SystemCursorIcon};

#[derive(Clone, Copy, Debug)]
enum AreaEditOperation {
    Move,
    Resize(AreaEditHandleKind),
}

#[derive(Clone)]
struct AreaEditDrag {
    familiar_entity: Entity,
    operation: AreaEditOperation,
    original_area: TaskArea,
    drag_start: Vec2,
}

#[derive(Resource, Default)]
pub struct AreaEditSession {
    active_drag: Option<AreaEditDrag>,
}

impl AreaEditSession {
    pub fn is_dragging(&self) -> bool {
        self.active_drag.is_some()
    }

    pub fn operation_label(&self) -> Option<&'static str> {
        let drag = self.active_drag.as_ref()?;
        Some(match drag.operation {
            AreaEditOperation::Move => "Move",
            AreaEditOperation::Resize(AreaEditHandleKind::TopLeft) => "Resize TL",
            AreaEditOperation::Resize(AreaEditHandleKind::Top) => "Resize T",
            AreaEditOperation::Resize(AreaEditHandleKind::TopRight) => "Resize TR",
            AreaEditOperation::Resize(AreaEditHandleKind::Right) => "Resize R",
            AreaEditOperation::Resize(AreaEditHandleKind::BottomRight) => "Resize BR",
            AreaEditOperation::Resize(AreaEditHandleKind::Bottom) => "Resize B",
            AreaEditOperation::Resize(AreaEditHandleKind::BottomLeft) => "Resize BL",
            AreaEditOperation::Resize(AreaEditHandleKind::Left) => "Resize L",
            AreaEditOperation::Resize(AreaEditHandleKind::Center) => "Move",
        })
    }
}

#[derive(Clone)]
struct AreaEditHistoryEntry {
    familiar_entity: Entity,
    before: Option<TaskArea>,
    after: Option<TaskArea>,
}

#[derive(Resource, Default)]
pub struct AreaEditHistory {
    undo_stack: Vec<AreaEditHistoryEntry>,
    redo_stack: Vec<AreaEditHistoryEntry>,
}

impl AreaEditHistory {
    pub fn push(
        &mut self,
        familiar_entity: Entity,
        before: Option<TaskArea>,
        after: Option<TaskArea>,
    ) {
        if before.as_ref().map(|a| (a.min, a.max)) == after.as_ref().map(|a| (a.min, a.max)) {
            return;
        }

        const MAX_HISTORY: usize = 64;
        self.undo_stack.push(AreaEditHistoryEntry {
            familiar_entity,
            before,
            after,
        });
        if self.undo_stack.len() > MAX_HISTORY {
            let drop_count = self.undo_stack.len() - MAX_HISTORY;
            self.undo_stack.drain(0..drop_count);
        }
        self.redo_stack.clear();
    }
}

#[derive(Resource, Default)]
pub struct AreaEditClipboard {
    area: Option<TaskArea>,
}

impl AreaEditClipboard {
    pub fn has_area(&self) -> bool {
        self.area.is_some()
    }
}

#[derive(Resource, Default)]
pub struct AreaEditPresets {
    slots: [Option<Vec2>; 3],
}

impl AreaEditPresets {
    pub fn save_size(&mut self, slot: usize, size: Vec2) {
        if slot < self.slots.len() {
            self.slots[slot] = Some(size.abs());
        }
    }

    pub fn get_size(&self, slot: usize) -> Option<Vec2> {
        self.slots.get(slot).and_then(|size| *size)
    }
}

fn apply_task_area_to_familiar(
    familiar_entity: Entity,
    area: Option<&TaskArea>,
    commands: &mut Commands,
    q_familiars: &mut Query<(&mut ActiveCommand, &mut Destination), With<Familiar>>,
) {
    if let Some(area) = area {
        commands.entity(familiar_entity).insert(area.clone());
        if let Ok((mut active_command, mut familiar_dest)) = q_familiars.get_mut(familiar_entity) {
            familiar_dest.0 = area.center();
            active_command.command = FamiliarCommand::Patrol;
        }
    } else {
        commands.entity(familiar_entity).remove::<TaskArea>();
        if let Ok((mut active_command, _)) = q_familiars.get_mut(familiar_entity) {
            active_command.command = FamiliarCommand::Idle;
        }
    }
}

fn hotkey_slot_index(keyboard: &ButtonInput<KeyCode>) -> Option<usize> {
    if keyboard.just_pressed(KeyCode::Digit1) {
        Some(0)
    } else if keyboard.just_pressed(KeyCode::Digit2) {
        Some(1)
    } else if keyboard.just_pressed(KeyCode::Digit3) {
        Some(2)
    } else {
        None
    }
}

fn area_from_center_and_size(center: Vec2, size: Vec2) -> TaskArea {
    let half = size.abs() * 0.5;
    TaskArea {
        min: center - half,
        max: center + half,
    }
}

fn world_cursor_pos(
    q_window: &Query<&Window, With<bevy::window::PrimaryWindow>>,
    q_camera: &Query<(&Camera, &GlobalTransform), With<MainCamera>>,
) -> Option<Vec2> {
    let Ok((camera, camera_transform)) = q_camera.single() else {
        return None;
    };
    let Ok(window) = q_window.single() else {
        return None;
    };
    let cursor_pos = window.cursor_position()?;
    camera
        .viewport_to_world_2d(camera_transform, cursor_pos)
        .ok()
}

fn detect_area_edit_operation(area: &TaskArea, world_pos: Vec2) -> Option<AreaEditOperation> {
    let threshold = TILE_SIZE * 0.55;
    let min = area.min;
    let max = area.max;
    let mid_x = (min.x + max.x) * 0.5;
    let mid_y = (min.y + max.y) * 0.5;

    let corners = [
        (AreaEditHandleKind::TopLeft, Vec2::new(min.x, max.y)),
        (AreaEditHandleKind::TopRight, Vec2::new(max.x, max.y)),
        (AreaEditHandleKind::BottomRight, Vec2::new(max.x, min.y)),
        (AreaEditHandleKind::BottomLeft, Vec2::new(min.x, min.y)),
    ];
    for (kind, point) in corners {
        if point.distance(world_pos) <= threshold {
            return Some(AreaEditOperation::Resize(kind));
        }
    }

    if (world_pos.y - max.y).abs() <= threshold && world_pos.x >= min.x && world_pos.x <= max.x {
        return Some(AreaEditOperation::Resize(AreaEditHandleKind::Top));
    }
    if (world_pos.x - max.x).abs() <= threshold && world_pos.y >= min.y && world_pos.y <= max.y {
        return Some(AreaEditOperation::Resize(AreaEditHandleKind::Right));
    }
    if (world_pos.y - min.y).abs() <= threshold && world_pos.x >= min.x && world_pos.x <= max.x {
        return Some(AreaEditOperation::Resize(AreaEditHandleKind::Bottom));
    }
    if (world_pos.x - min.x).abs() <= threshold && world_pos.y >= min.y && world_pos.y <= max.y {
        return Some(AreaEditOperation::Resize(AreaEditHandleKind::Left));
    }

    if Vec2::new(mid_x, mid_y).distance(world_pos) <= threshold || area.contains(world_pos) {
        return Some(AreaEditOperation::Move);
    }

    None
}

fn apply_area_edit_drag(active_drag: &AreaEditDrag, current_snapped: Vec2) -> TaskArea {
    let min_size = TILE_SIZE.max(1.0);
    let mut min = active_drag.original_area.min;
    let mut max = active_drag.original_area.max;

    match active_drag.operation {
        AreaEditOperation::Move => {
            let delta = current_snapped - active_drag.drag_start;
            min += delta;
            max += delta;
        }
        AreaEditOperation::Resize(handle) => match handle {
            AreaEditHandleKind::TopLeft => {
                min.x = current_snapped.x.min(max.x - min_size);
                max.y = current_snapped.y.max(min.y + min_size);
            }
            AreaEditHandleKind::Top => {
                max.y = current_snapped.y.max(min.y + min_size);
            }
            AreaEditHandleKind::TopRight => {
                max.x = current_snapped.x.max(min.x + min_size);
                max.y = current_snapped.y.max(min.y + min_size);
            }
            AreaEditHandleKind::Right => {
                max.x = current_snapped.x.max(min.x + min_size);
            }
            AreaEditHandleKind::BottomRight => {
                max.x = current_snapped.x.max(min.x + min_size);
                min.y = current_snapped.y.min(max.y - min_size);
            }
            AreaEditHandleKind::Bottom => {
                min.y = current_snapped.y.min(max.y - min_size);
            }
            AreaEditHandleKind::BottomLeft => {
                min.x = current_snapped.x.min(max.x - min_size);
                min.y = current_snapped.y.min(max.y - min_size);
            }
            AreaEditHandleKind::Left => {
                min.x = current_snapped.x.min(max.x - min_size);
            }
            AreaEditHandleKind::Center => {
                let delta = current_snapped - active_drag.drag_start;
                min += delta;
                max += delta;
            }
        },
    }

    TaskArea { min, max }
}

fn cursor_icon_for_operation(operation: AreaEditOperation, dragging: bool) -> CursorIcon {
    match operation {
        AreaEditOperation::Move => {
            if dragging {
                CursorIcon::System(SystemCursorIcon::Grabbing)
            } else {
                CursorIcon::System(SystemCursorIcon::Grab)
            }
        }
        AreaEditOperation::Resize(handle) => {
            let icon = match handle {
                AreaEditHandleKind::Top | AreaEditHandleKind::Bottom => SystemCursorIcon::NsResize,
                AreaEditHandleKind::Left | AreaEditHandleKind::Right => SystemCursorIcon::EwResize,
                AreaEditHandleKind::TopLeft | AreaEditHandleKind::BottomRight => {
                    SystemCursorIcon::NwseResize
                }
                AreaEditHandleKind::TopRight | AreaEditHandleKind::BottomLeft => {
                    SystemCursorIcon::NeswResize
                }
                AreaEditHandleKind::Center => SystemCursorIcon::Grab,
            };
            CursorIcon::System(icon)
        }
    }
}

pub fn task_area_selection_system(
    buttons: Res<ButtonInput<MouseButton>>,
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    ui_input_state: Res<UiInputState>,
    selected: Res<SelectedEntity>,
    mut task_context: ResMut<TaskContext>,
    mut next_play_mode: ResMut<NextState<PlayMode>>,
    mut q_familiars: Query<(&mut ActiveCommand, &mut Destination), With<Familiar>>,
    q_familiar_areas: Query<&TaskArea, With<Familiar>>,
    q_targets: Query<(
        Entity,
        &Transform,
        Option<&Tree>,
        Option<&Rock>,
        Option<&ResourceItem>,
    )>,
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    q_unassigned: Query<
        (Entity, &Transform, &Designation),
        Without<crate::relationships::ManagedBy>,
    >,
    q_selection_indicator: Query<Entity, With<AreaSelectionIndicator>>,
    mut area_edit_session: ResMut<AreaEditSession>,
    mut area_edit_history: ResMut<AreaEditHistory>,
) {
    if ui_input_state.pointer_over_ui {
        return;
    }

    if task_context.0 == TaskMode::None {
        area_edit_session.active_drag = None;
        return;
    }

    if let Some(active_drag) = area_edit_session.active_drag.clone() {
        if buttons.pressed(MouseButton::Left)
            && let Some(world_pos) = world_cursor_pos(&q_window, &q_camera)
        {
            let snapped_pos = WorldMap::snap_to_grid_edge(world_pos);
            let updated_area = apply_area_edit_drag(&active_drag, snapped_pos);
            let center = updated_area.center();

            commands
                .entity(active_drag.familiar_entity)
                .insert(updated_area.clone());
            if let Ok((mut active_command, mut familiar_dest)) =
                q_familiars.get_mut(active_drag.familiar_entity)
            {
                familiar_dest.0 = center;
                active_command.command = FamiliarCommand::Patrol;
            }
        }

        if buttons.just_released(MouseButton::Left) {
            let applied_area = world_cursor_pos(&q_window, &q_camera)
                .map(WorldMap::snap_to_grid_edge)
                .map(|snapped| apply_area_edit_drag(&active_drag, snapped))
                .unwrap_or_else(|| active_drag.original_area.clone());
            apply_task_area_to_familiar(
                active_drag.familiar_entity,
                Some(&applied_area),
                &mut commands,
                &mut q_familiars,
            );

            let min_x = applied_area.min.x;
            let max_x = applied_area.max.x;
            let min_y = applied_area.min.y;
            let max_y = applied_area.max.y;

            let mut assigned_count = 0;
            for (task_entity, task_transform, _designation) in q_unassigned.iter() {
                let pos = task_transform.translation.truncate();
                if pos.x >= min_x - 0.1
                    && pos.x <= max_x + 0.1
                    && pos.y >= min_y - 0.1
                    && pos.y <= max_y + 0.1
                {
                    commands.entity(task_entity).insert((
                        crate::relationships::ManagedBy(active_drag.familiar_entity),
                        crate::systems::jobs::Priority(0),
                    ));
                    assigned_count += 1;
                }
            }
            if assigned_count > 0 {
                info!(
                    "AREA_EDIT: Also assigned {} unassigned task(s) to Familiar {:?}",
                    assigned_count, active_drag.familiar_entity
                );
            }

            area_edit_history.push(
                active_drag.familiar_entity,
                Some(active_drag.original_area),
                Some(applied_area),
            );

            area_edit_session.active_drag = None;
            let exit_after_apply =
                keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
            if exit_after_apply {
                task_context.0 = TaskMode::None;
                next_play_mode.set(PlayMode::Normal);
                info!("AREA_EDIT: Applied and exited Area Edit mode");
            } else {
                task_context.0 = TaskMode::AreaSelection(None);
                info!("AREA_EDIT: Applied and kept Area Edit mode");
            }
            return;
        }

        if buttons.pressed(MouseButton::Left) {
            return;
        }

        area_edit_session.active_drag = None;
    }

    if buttons.just_pressed(MouseButton::Left) {
        if let Some(world_pos) = world_cursor_pos(&q_window, &q_camera) {
            // 開始位置はグリッドのエッジにスナップ
            let snapped_pos = WorldMap::snap_to_grid_edge(world_pos);

            if let TaskMode::AreaSelection(None) = task_context.0
                && let Some(fam_entity) = selected.0
                && let Ok(existing_area) = q_familiar_areas.get(fam_entity)
                && let Some(operation) = detect_area_edit_operation(existing_area, world_pos)
            {
                area_edit_session.active_drag = Some(AreaEditDrag {
                    familiar_entity: fam_entity,
                    operation,
                    original_area: existing_area.clone(),
                    drag_start: snapped_pos,
                });
                info!(
                    "AREA_EDIT: Started direct {:?} for Familiar {:?}",
                    operation, fam_entity
                );
                return;
            }

            match task_context.0 {
                TaskMode::AreaSelection(None) => {
                    task_context.0 = TaskMode::AreaSelection(Some(snapped_pos))
                }
                TaskMode::DesignateChop(None) => {
                    task_context.0 = TaskMode::DesignateChop(Some(snapped_pos))
                }
                TaskMode::DesignateMine(None) => {
                    task_context.0 = TaskMode::DesignateMine(Some(snapped_pos))
                }
                TaskMode::DesignateHaul(None) => {
                    task_context.0 = TaskMode::DesignateHaul(Some(snapped_pos))
                }
                TaskMode::CancelDesignation(None) => {
                    task_context.0 = TaskMode::CancelDesignation(Some(snapped_pos))
                }
                TaskMode::AssignTask(None) => {
                    task_context.0 = TaskMode::AssignTask(Some(snapped_pos))
                }
                _ => {}
            }
        }
    }

    if buttons.just_released(MouseButton::Left) {
        let Ok((camera, camera_transform)) = q_camera.single() else {
            return;
        };
        let Ok(window) = q_window.single() else {
            return;
        };

        if let Some(cursor_pos) = window.cursor_position() {
            if let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) {
                match task_context.0 {
                    TaskMode::AreaSelection(Some(start_pos)) => {
                        let end_pos = WorldMap::snap_to_grid_edge(world_pos);
                        let min_x = f32::min(start_pos.x, end_pos.x);
                        let max_x = f32::max(start_pos.x, end_pos.x);
                        let min_y = f32::min(start_pos.y, end_pos.y);
                        let max_y = f32::max(start_pos.y, end_pos.y);
                        let min = Vec2::new(min_x, min_y);
                        let max = Vec2::new(max_x, max_y);
                        let center = (min + max) / 2.0;

                        if let Some(fam_entity) = selected.0 {
                            let before_area = q_familiar_areas.get(fam_entity).ok().cloned();
                            let new_area = TaskArea { min, max };
                            if let Ok((mut active_command, mut familiar_dest)) =
                                q_familiars.get_mut(fam_entity)
                            {
                                commands.entity(fam_entity).insert(new_area.clone());
                                familiar_dest.0 = center;
                                active_command.command = FamiliarCommand::Patrol;
                                info!(
                                    "AREA_ASSIGNMENT: Familiar {:?} assigned to rectangular area",
                                    fam_entity
                                );

                                let mut assigned_count = 0;
                                for (task_entity, task_transform, _designation) in
                                    q_unassigned.iter()
                                {
                                    let pos = task_transform.translation.truncate();
                                    if pos.x >= min_x - 0.1
                                        && pos.x <= max_x + 0.1
                                        && pos.y >= min_y - 0.1
                                        && pos.y <= max_y + 0.1
                                    {
                                        commands.entity(task_entity).insert((
                                            crate::relationships::ManagedBy(fam_entity),
                                            crate::systems::jobs::Priority(0),
                                        ));
                                        assigned_count += 1;
                                    }
                                }
                                if assigned_count > 0 {
                                    info!(
                                        "AREA_ASSIGNMENT: Also assigned {} unassigned task(s) to Familiar {:?}",
                                        assigned_count, fam_entity
                                    );
                                }
                            }
                            area_edit_history.push(fam_entity, before_area, Some(new_area));
                        }
                        for indicator_entity in q_selection_indicator.iter() {
                            commands.entity(indicator_entity).despawn();
                        }
                        let exit_after_apply = keyboard.pressed(KeyCode::ShiftLeft)
                            || keyboard.pressed(KeyCode::ShiftRight);

                        if exit_after_apply {
                            task_context.0 = TaskMode::None;
                            next_play_mode.set(PlayMode::Normal);
                            info!("AREA_ASSIGNMENT: Applied and exited Area Edit mode");
                        } else {
                            // 連続編集をデフォルトにすることで、頻繁なエリア変更を高速化
                            task_context.0 = TaskMode::AreaSelection(None);
                            info!("AREA_ASSIGNMENT: Applied and kept Area Edit mode");
                        }
                    }
                    TaskMode::DesignateChop(Some(start_pos))
                    | TaskMode::DesignateMine(Some(start_pos))
                    | TaskMode::DesignateHaul(Some(start_pos))
                    | TaskMode::CancelDesignation(Some(start_pos)) => {
                        let end_pos = WorldMap::snap_to_grid_edge(world_pos);
                        let min_x = f32::min(start_pos.x, end_pos.x);
                        let max_x = f32::max(start_pos.x, end_pos.x);
                        let min_y = f32::min(start_pos.y, end_pos.y);
                        let max_y = f32::max(start_pos.y, end_pos.y);

                        let work_type = match task_context.0 {
                            TaskMode::DesignateChop(_) => Some(WorkType::Chop),
                            TaskMode::DesignateMine(_) => Some(WorkType::Mine),
                            TaskMode::DesignateHaul(_) => Some(WorkType::Haul),
                            _ => None,
                        };

                        let fam_entity = selected.0;

                        for (target_entity, transform, tree, rock, item) in q_targets.iter() {
                            let pos = transform.translation.truncate();
                            if pos.x >= min_x - 0.1
                                && pos.x <= max_x + 0.1
                                && pos.y >= min_y - 0.1
                                && pos.y <= max_y + 0.1
                            {
                                if let Some(wt) = work_type {
                                    let match_found = match wt {
                                        WorkType::Chop => tree.is_some(),
                                        WorkType::Mine => rock.is_some(),
                                        WorkType::Haul => item.is_some(),
                                        _ => false,
                                    };

                                    if match_found {
                                        if let Some(issued_by) = fam_entity {
                                            commands.entity(target_entity).insert((
                                                crate::systems::jobs::Designation { work_type: wt },
                                                crate::relationships::ManagedBy(issued_by),
                                                crate::systems::jobs::TaskSlots::new(1),
                                                crate::systems::jobs::Priority(0),
                                            ));
                                            info!(
                                                "DESIGNATION: Created {:?} for {:?} (assigned to {:?})",
                                                wt, target_entity, issued_by
                                            );
                                        } else {
                                            commands.entity(target_entity).insert((
                                                crate::systems::jobs::Designation { work_type: wt },
                                                crate::systems::jobs::TaskSlots::new(1),
                                                crate::systems::jobs::Priority(0),
                                            ));
                                            info!(
                                                "DESIGNATION: Created {:?} for {:?} (unassigned)",
                                                wt, target_entity
                                            );
                                        }
                                    }
                                } else {
                                    commands
                                        .entity(target_entity)
                                        .remove::<crate::systems::jobs::Designation>();
                                    commands
                                        .entity(target_entity)
                                        .remove::<crate::systems::jobs::TaskSlots>();
                                    commands
                                        .entity(target_entity)
                                        .remove::<crate::relationships::ManagedBy>();
                                }
                            }
                        }

                        task_context.0 = match task_context.0 {
                            TaskMode::DesignateChop(_) => TaskMode::DesignateChop(None),
                            TaskMode::DesignateMine(_) => TaskMode::DesignateMine(None),
                            TaskMode::DesignateHaul(_) => TaskMode::DesignateHaul(None),
                            TaskMode::CancelDesignation(_) => TaskMode::CancelDesignation(None),
                            _ => TaskMode::None,
                        };
                    }
                    _ => {}
                }
            }
        }
    }
}

pub fn task_area_edit_cursor_system(
    task_context: Res<TaskContext>,
    selected: Res<SelectedEntity>,
    ui_input_state: Res<UiInputState>,
    area_edit_session: Res<AreaEditSession>,
    q_task_areas: Query<&TaskArea, With<Familiar>>,
    q_window_entity: Query<Entity, With<PrimaryWindow>>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut q_cursor: Query<&mut CursorIcon, With<PrimaryWindow>>,
    mut commands: Commands,
) {
    if ui_input_state.pointer_over_ui {
        return;
    }

    let Ok(window_entity) = q_window_entity.single() else {
        return;
    };

    let desired = if !matches!(task_context.0, TaskMode::AreaSelection(_)) {
        CursorIcon::System(SystemCursorIcon::Default)
    } else if let Some(active_drag) = area_edit_session.active_drag.as_ref() {
        cursor_icon_for_operation(active_drag.operation, true)
    } else if let (Some(fam_entity), Some(world_pos)) =
        (selected.0, world_cursor_pos(&q_window, &q_camera))
    {
        if let Ok(area) = q_task_areas.get(fam_entity) {
            if let Some(operation) = detect_area_edit_operation(area, world_pos) {
                cursor_icon_for_operation(operation, false)
            } else {
                CursorIcon::System(SystemCursorIcon::Default)
            }
        } else {
            CursorIcon::System(SystemCursorIcon::Default)
        }
    } else {
        CursorIcon::System(SystemCursorIcon::Default)
    };

    if let Ok(mut icon) = q_cursor.get_mut(window_entity) {
        if *icon != desired {
            *icon = desired;
        }
    } else {
        commands.entity(window_entity).insert(desired);
    }
}

pub fn task_area_edit_history_shortcuts_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    task_context: Res<TaskContext>,
    mut selected_entity: ResMut<SelectedEntity>,
    mut area_edit_history: ResMut<AreaEditHistory>,
    mut area_edit_clipboard: ResMut<AreaEditClipboard>,
    mut area_edit_presets: ResMut<AreaEditPresets>,
    q_familiar_exists: Query<(), With<Familiar>>,
    q_task_areas: Query<&TaskArea, With<Familiar>>,
    mut q_familiars: Query<(&mut ActiveCommand, &mut Destination), With<Familiar>>,
    mut commands: Commands,
) {
    if !matches!(task_context.0, TaskMode::AreaSelection(_)) {
        return;
    }

    let ctrl_pressed =
        keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
    let alt_pressed = keyboard.pressed(KeyCode::AltLeft) || keyboard.pressed(KeyCode::AltRight);

    if alt_pressed && let Some(slot) = hotkey_slot_index(&keyboard) {
        let Some(selected) = selected_entity.0 else {
            return;
        };
        if q_familiar_exists.get(selected).is_err() {
            return;
        }
        let Some(preset_size) = area_edit_presets.get_size(slot) else {
            info!("AREA_EDIT: Preset {} is empty", slot + 1);
            return;
        };

        let before = q_task_areas.get(selected).ok().cloned();
        let center = if let Some(area) = before.as_ref() {
            area.center()
        } else if let Ok((_, dest)) = q_familiars.get_mut(selected) {
            dest.0
        } else {
            return;
        };

        let new_area = area_from_center_and_size(center, preset_size);
        apply_task_area_to_familiar(selected, Some(&new_area), &mut commands, &mut q_familiars);
        area_edit_history.push(selected, before, Some(new_area));
        info!(
            "AREA_EDIT: Applied preset {} to Familiar {:?}",
            slot + 1,
            selected
        );
        return;
    }

    if !ctrl_pressed {
        return;
    }

    if let Some(slot) = hotkey_slot_index(&keyboard) {
        if let Some(selected) = selected_entity.0
            && q_familiar_exists.get(selected).is_ok()
        {
            if let Ok(area) = q_task_areas.get(selected) {
                area_edit_presets.save_size(slot, area.size());
                info!(
                    "AREA_EDIT: Saved Familiar {:?} area size to preset {}",
                    selected,
                    slot + 1
                );
            } else {
                info!(
                    "AREA_EDIT: Familiar {:?} has no area, preset {} not updated",
                    selected,
                    slot + 1
                );
            }
        }
        return;
    }

    if keyboard.just_pressed(KeyCode::KeyC) {
        if let Some(selected) = selected_entity.0
            && q_familiar_exists.get(selected).is_ok()
        {
            area_edit_clipboard.area = q_task_areas.get(selected).ok().cloned();
            if area_edit_clipboard.area.is_some() {
                info!("AREA_EDIT: Copied TaskArea from Familiar {:?}", selected);
            } else {
                info!(
                    "AREA_EDIT: Familiar {:?} has no TaskArea, clipboard cleared",
                    selected
                );
            }
        }
        return;
    }

    if keyboard.just_pressed(KeyCode::KeyV) {
        let Some(selected) = selected_entity.0 else {
            return;
        };
        if q_familiar_exists.get(selected).is_err() {
            return;
        }
        let Some(copied_area) = area_edit_clipboard.area.clone() else {
            info!("AREA_EDIT: Paste requested but clipboard is empty");
            return;
        };

        let before = q_task_areas.get(selected).ok().cloned();
        apply_task_area_to_familiar(
            selected,
            Some(&copied_area),
            &mut commands,
            &mut q_familiars,
        );
        area_edit_history.push(selected, before, Some(copied_area));
        info!("AREA_EDIT: Pasted TaskArea to Familiar {:?}", selected);
        return;
    }

    let redo_via_shift_z = keyboard.just_pressed(KeyCode::KeyZ)
        && (keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight));

    if keyboard.just_pressed(KeyCode::KeyY) || redo_via_shift_z {
        if let Some(entry) = area_edit_history.redo_stack.pop() {
            apply_task_area_to_familiar(
                entry.familiar_entity,
                entry.after.as_ref(),
                &mut commands,
                &mut q_familiars,
            );
            selected_entity.0 = Some(entry.familiar_entity);
            area_edit_history.undo_stack.push(entry.clone());
            info!(
                "AREA_EDIT: Redo applied to Familiar {:?}",
                entry.familiar_entity
            );
        }
        return;
    }

    if keyboard.just_pressed(KeyCode::KeyZ)
        && let Some(entry) = area_edit_history.undo_stack.pop()
    {
        apply_task_area_to_familiar(
            entry.familiar_entity,
            entry.before.as_ref(),
            &mut commands,
            &mut q_familiars,
        );
        selected_entity.0 = Some(entry.familiar_entity);
        area_edit_history.redo_stack.push(entry.clone());
        info!(
            "AREA_EDIT: Undo applied to Familiar {:?}",
            entry.familiar_entity
        );
    }
}

pub fn area_selection_indicator_system(
    task_context: Res<TaskContext>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut q_indicator: Query<
        (Entity, &mut Transform, &mut Sprite, &mut Visibility),
        With<AreaSelectionIndicator>,
    >,
    mut commands: Commands,
) {
    let drag_start = match task_context.0 {
        TaskMode::AreaSelection(s) => s,
        TaskMode::DesignateChop(s) => s,
        TaskMode::DesignateMine(s) => s,
        TaskMode::DesignateHaul(s) => s,
        TaskMode::CancelDesignation(s) => s,
        _ => None,
    };

    if let Some(start_pos) = drag_start {
        let Ok((camera, camera_transform)) = q_camera.single() else {
            return;
        };
        let Ok(window) = q_window.single() else {
            return;
        };

        if let Some(cursor_pos) = window.cursor_position() {
            if let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) {
                let end_pos = WorldMap::snap_to_grid_edge(world_pos);
                let center = (start_pos + end_pos) / 2.0;
                let size = (start_pos - end_pos).abs();

                let color = match task_context.0 {
                    TaskMode::AreaSelection(_) => Color::srgba(1.0, 1.0, 1.0, 0.2),
                    TaskMode::CancelDesignation(_) => Color::srgba(1.0, 0.2, 0.2, 0.3),
                    _ => Color::srgba(0.2, 1.0, 0.2, 0.3),
                };

                if let Ok((_, mut transform, mut sprite, mut visibility)) = q_indicator.single_mut()
                {
                    transform.translation = center.extend(0.6);
                    sprite.custom_size = Some(size);
                    sprite.color = color;
                    *visibility = Visibility::Visible;
                } else {
                    commands.spawn((
                        AreaSelectionIndicator,
                        Sprite {
                            color: color,
                            custom_size: Some(size),
                            ..default()
                        },
                        Transform::from_translation(center.extend(0.6)),
                    ));
                }
            }
        }
    } else {
        if let Ok((_, _, _, mut visibility)) = q_indicator.single_mut() {
            *visibility = Visibility::Hidden;
        }
    }
}
