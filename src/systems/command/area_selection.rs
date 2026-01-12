use super::{AreaSelectionIndicator, TaskArea, TaskMode};
use crate::entities::damned_soul::Destination;
use crate::entities::familiar::{ActiveCommand, Familiar, FamiliarCommand};
use crate::game_state::TaskContext;
use crate::interface::camera::MainCamera;
use crate::interface::selection::SelectedEntity;
use crate::systems::jobs::{Designation, DesignationCreatedEvent, IssuedBy, Rock, Tree, WorkType};
use crate::systems::logistics::ResourceItem;
use crate::systems::work::GlobalTaskQueue;
use bevy::prelude::*;

pub fn task_area_selection_system(
    buttons: Res<ButtonInput<MouseButton>>,
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    q_ui: Query<&Interaction, With<Button>>,
    selected: Res<SelectedEntity>,
    mut task_context: ResMut<TaskContext>,
    mut q_familiars: Query<(&mut ActiveCommand, &mut Destination), With<Familiar>>,
    q_targets: Query<(
        Entity,
        &Transform,
        Option<&Tree>,
        Option<&Rock>,
        Option<&ResourceItem>,
    )>,
    mut commands: Commands,
    mut ev_created: MessageWriter<DesignationCreatedEvent>,
    keyboard: Res<ButtonInput<KeyCode>>,
    q_unassigned: Query<(Entity, &Transform, &Designation), Without<IssuedBy>>,
    mut global_queue: ResMut<GlobalTaskQueue>,
    mut queue: ResMut<crate::systems::work::TaskQueue>,
    q_selection_indicator: Query<Entity, With<AreaSelectionIndicator>>,
) {
    if q_ui.iter().any(|i| *i != Interaction::None) {
        return;
    }

    if task_context.0 == TaskMode::None {
        return;
    }

    if buttons.just_pressed(MouseButton::Left) {
        let Ok((camera, camera_transform)) = q_camera.single() else {
            return;
        };
        let Ok(window) = q_window.single() else {
            return;
        };
        if let Some(cursor_pos) = window.cursor_position() {
            if let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) {
                match task_context.0 {
                    TaskMode::AreaSelection(None) => {
                        task_context.0 = TaskMode::AreaSelection(Some(world_pos))
                    }
                    TaskMode::DesignateChop(None) => {
                        task_context.0 = TaskMode::DesignateChop(Some(world_pos))
                    }
                    TaskMode::DesignateMine(None) => {
                        task_context.0 = TaskMode::DesignateMine(Some(world_pos))
                    }
                    TaskMode::DesignateHaul(None) => {
                        task_context.0 = TaskMode::DesignateHaul(Some(world_pos))
                    }
                    TaskMode::CancelDesignation(None) => {
                        task_context.0 = TaskMode::CancelDesignation(Some(world_pos))
                    }
                    TaskMode::AssignTask(None) => {
                        task_context.0 = TaskMode::AssignTask(Some(world_pos))
                    }
                    _ => {}
                }
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
                        let min_x = f32::min(start_pos.x, world_pos.x);
                        let max_x = f32::max(start_pos.x, world_pos.x);
                        let min_y = f32::min(start_pos.y, world_pos.y);
                        let max_y = f32::max(start_pos.y, world_pos.y);
                        let min = Vec2::new(min_x, min_y);
                        let max = Vec2::new(max_x, max_y);
                        let center = (min + max) / 2.0;

                        if let Some(fam_entity) = selected.0 {
                            if let Ok((mut active_command, mut familiar_dest)) =
                                q_familiars.get_mut(fam_entity)
                            {
                                commands.entity(fam_entity).insert(TaskArea { min, max });
                                familiar_dest.0 = center;
                                active_command.command = FamiliarCommand::Patrol;
                                info!(
                                    "AREA_ASSIGNMENT: Familiar {:?} assigned to rectangular area",
                                    fam_entity
                                );

                                let mut assigned_count = 0;
                                for (task_entity, task_transform, designation) in
                                    q_unassigned.iter()
                                {
                                    let pos = task_transform.translation.truncate();
                                    if pos.x >= min_x - 0.1
                                        && pos.x <= max_x + 0.1
                                        && pos.y >= min_y - 0.1
                                        && pos.y <= max_y + 0.1
                                    {
                                        commands.entity(task_entity).insert(IssuedBy(fam_entity));
                                        global_queue.remove(task_entity);
                                        queue.add(
                                            fam_entity,
                                            crate::systems::work::PendingTask {
                                                entity: task_entity,
                                                work_type: designation.work_type,
                                                priority: 0,
                                            },
                                        );
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
                        }
                        for indicator_entity in q_selection_indicator.iter() {
                            commands.entity(indicator_entity).despawn();
                        }
                        task_context.0 = TaskMode::None;
                    }
                    TaskMode::DesignateChop(Some(start_pos))
                    | TaskMode::DesignateMine(Some(start_pos))
                    | TaskMode::DesignateHaul(Some(start_pos))
                    | TaskMode::CancelDesignation(Some(start_pos)) => {
                        let min_x = f32::min(start_pos.x, world_pos.x);
                        let max_x = f32::max(start_pos.x, world_pos.x);
                        let min_y = f32::min(start_pos.y, world_pos.y);
                        let max_y = f32::max(start_pos.y, world_pos.y);

                        let work_type = match task_context.0 {
                            TaskMode::DesignateChop(_) => Some(WorkType::Chop),
                            TaskMode::DesignateMine(_) => Some(WorkType::Mine),
                            TaskMode::DesignateHaul(_) => Some(WorkType::Haul),
                            _ => None,
                        };

                        let priority = if keyboard.pressed(KeyCode::ShiftLeft)
                            || keyboard.pressed(KeyCode::ShiftRight)
                        {
                            1
                        } else {
                            0
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
                                                IssuedBy(issued_by),
                                                crate::systems::jobs::TaskSlots::new(1),
                                            ));
                                            info!(
                                                "DESIGNATION: Created {:?} for {:?} (assigned to {:?})",
                                                wt, target_entity, issued_by
                                            );
                                        } else {
                                            commands.entity(target_entity).insert((
                                                crate::systems::jobs::Designation { work_type: wt },
                                                crate::systems::jobs::TaskSlots::new(1),
                                            ));
                                            info!(
                                                "DESIGNATION: Created {:?} for {:?} (unassigned)",
                                                wt, target_entity
                                            );
                                        }
                                        ev_created.write(DesignationCreatedEvent {
                                            entity: target_entity,
                                            work_type: wt,
                                            issued_by: fam_entity,
                                            priority,
                                        });
                                    }
                                } else {
                                    commands
                                        .entity(target_entity)
                                        .remove::<crate::systems::jobs::Designation>();
                                    commands
                                        .entity(target_entity)
                                        .remove::<crate::systems::jobs::TaskSlots>();
                                    commands.entity(target_entity).remove::<IssuedBy>();
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
                let center = (start_pos + world_pos) / 2.0;
                let size = (start_pos - world_pos).abs();

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
