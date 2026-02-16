use super::apply::{
    apply_designation_in_area, apply_task_area_to_familiar, assign_unassigned_tasks_in_area,
    cancel_single_designation,
};
use super::geometry::{apply_area_edit_drag, detect_area_edit_operation, world_cursor_pos};
use super::state::{AreaEditHistory, AreaEditSession, Drag};
use crate::constants::TILE_SIZE;
use crate::entities::damned_soul::Destination;
use crate::entities::familiar::{ActiveCommand, Familiar, FamiliarCommand};
use crate::game_state::{PlayMode, TaskContext};
use crate::interface::camera::MainCamera;
use crate::interface::selection::SelectedEntity;
use crate::interface::ui::UiInputState;
use crate::relationships::{StoredItems, TaskWorkers};
use crate::systems::command::{AreaSelectionIndicator, TaskArea, TaskMode};
use crate::systems::jobs::{Blueprint, Designation, Rock, Tree};
use crate::systems::jobs::floor_construction::{
    FloorConstructionCancelRequested, FloorTileBlueprint,
};
use crate::systems::logistics::transport_request::{
    ManualTransportRequest, TransportRequest, TransportRequestFixedSource,
};
use crate::systems::logistics::{
    BelongsTo, BucketStorage, ResourceItem, Stockpile,
};
use crate::world::map::WorldMap;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use std::collections::HashSet;

fn should_exit_after_apply(keyboard: &ButtonInput<KeyCode>) -> bool {
    keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight)
}

fn reset_designation_mode(mode: TaskMode) -> TaskMode {
    match mode {
        TaskMode::DesignateChop(_) => TaskMode::DesignateChop(None),
        TaskMode::DesignateMine(_) => TaskMode::DesignateMine(None),
        TaskMode::DesignateHaul(_) => TaskMode::DesignateHaul(None),
        TaskMode::CancelDesignation(_) => TaskMode::CancelDesignation(None),
        _ => TaskMode::None,
    }
}

fn try_start_direct_edit_drag(
    task_context: TaskMode,
    selected: Option<Entity>,
    q_familiar_areas: &Query<&TaskArea, With<Familiar>>,
    world_pos: Vec2,
    snapped_pos: Vec2,
    area_edit_session: &mut AreaEditSession,
) -> bool {
    if !matches!(task_context, TaskMode::AreaSelection(None)) {
        return false;
    }

    let Some(fam_entity) = selected else {
        return false;
    };
    let Ok(existing_area) = q_familiar_areas.get(fam_entity) else {
        return false;
    };
    let Some(operation) = detect_area_edit_operation(existing_area, world_pos) else {
        return false;
    };

    area_edit_session.active_drag = Some(Drag {
        familiar_entity: fam_entity,
        operation,
        original_area: existing_area.clone(),
        drag_start: snapped_pos,
    });
    info!(
        "AREA_EDIT: Started direct {:?} for Familiar {:?}",
        operation, fam_entity
    );

    true
}

fn despawn_selection_indicators(
    q_selection_indicator: &Query<Entity, With<AreaSelectionIndicator>>,
    commands: &mut Commands,
) {
    for indicator_entity in q_selection_indicator.iter() {
        commands.entity(indicator_entity).try_despawn();
    }
}

pub fn task_area_selection_system(
    buttons: Res<ButtonInput<MouseButton>>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    ui_input_state: Res<UiInputState>,
    selected: Res<SelectedEntity>,
    mut task_context: ResMut<TaskContext>,
    mut next_play_mode: ResMut<NextState<PlayMode>>,
    mut q_familiars: Query<(&mut ActiveCommand, &mut Destination), With<Familiar>>,
    q_familiar_areas: Query<&TaskArea, With<Familiar>>,
    mut q_target_sets: ParamSet<(
        Query<(
            Entity,
            &Transform,
            Option<&Tree>,
            Option<&Rock>,
            Option<&ResourceItem>,
            Option<&Designation>,
            Option<&TaskWorkers>,
            Option<&Blueprint>,
            Option<&BelongsTo>,
            Option<&TransportRequest>,
            Option<&TransportRequestFixedSource>,
            Option<&Stockpile>,
            Option<&StoredItems>,
            Option<&BucketStorage>,
            Option<&ManualTransportRequest>,
        )>,
        Query<(Entity, &Transform, &FloorTileBlueprint)>,
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

            commands
                .entity(active_drag.familiar_entity)
                .insert(updated_area.clone());
            if let Ok((mut active_command, mut familiar_dest)) =
                q_familiars.get_mut(active_drag.familiar_entity)
            {
                familiar_dest.0 = updated_area.center();
                active_command.command = FamiliarCommand::Patrol;
            }
        }

        if buttons.just_released(MouseButton::Left) {
            let applied_area = world_cursor_pos(&q_window, &q_camera)
                .map(WorldMap::snap_to_grid_edge)
                .map(|snapped| apply_area_edit_drag(&active_drag, snapped))
                .unwrap_or_else(|| active_drag.original_area.clone());

            if applied_area != active_drag.original_area {
                apply_task_area_to_familiar(
                    active_drag.familiar_entity,
                    Some(&applied_area),
                    &mut commands,
                    &mut q_familiars,
                );

                let assigned_count = assign_unassigned_tasks_in_area(
                    &mut commands,
                    active_drag.familiar_entity,
                    &applied_area,
                    &q_unassigned,
                );
                if assigned_count > 0 {
                    info!(
                        "AREA_EDIT: Also assigned {} unassigned task(s) to Familiar {:?}",
                        assigned_count, active_drag.familiar_entity
                    );
                }

                area_edit_history.push(
                    active_drag.familiar_entity,
                    Some(active_drag.original_area.clone()),
                    Some(applied_area),
                );
            }

            area_edit_session.active_drag = None;
            if should_exit_after_apply(&keyboard) {
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

    if buttons.just_pressed(MouseButton::Left)
        && let Some(world_pos) = world_cursor_pos(&q_window, &q_camera)
    {
        let snapped_pos = WorldMap::snap_to_grid_edge(world_pos);

        if try_start_direct_edit_drag(
            task_context.0,
            selected.0,
            &q_familiar_areas,
            world_pos,
            snapped_pos,
            &mut area_edit_session,
        ) {
            return;
        }

        match task_context.0 {
            TaskMode::AreaSelection(None) => task_context.0 = TaskMode::AreaSelection(Some(snapped_pos)),
            TaskMode::DesignateChop(None) => task_context.0 = TaskMode::DesignateChop(Some(snapped_pos)),
            TaskMode::DesignateMine(None) => task_context.0 = TaskMode::DesignateMine(Some(snapped_pos)),
            TaskMode::DesignateHaul(None) => task_context.0 = TaskMode::DesignateHaul(Some(snapped_pos)),
            TaskMode::CancelDesignation(None) => {
                task_context.0 = TaskMode::CancelDesignation(Some(snapped_pos))
            }
            TaskMode::AssignTask(None) => task_context.0 = TaskMode::AssignTask(Some(snapped_pos)),
            _ => {}
        }
    }

    if !buttons.just_released(MouseButton::Left) {
        return;
    }

    let Some(world_pos) = world_cursor_pos(&q_window, &q_camera) else {
        return;
    };

    match task_context.0 {
        TaskMode::AreaSelection(Some(start_pos)) => {
            let end_pos = WorldMap::snap_to_grid_edge(world_pos);

            if start_pos.distance(end_pos) < 0.1 {
                task_context.0 = TaskMode::None;
                next_play_mode.set(PlayMode::Normal);
                info!("AREA_ASSIGNMENT: No drag detected, exiting Area Edit mode");
                despawn_selection_indicators(&q_selection_indicator, &mut commands);
                return;
            }

            let new_area = TaskArea::from_points(start_pos, end_pos);
            if let Some(fam_entity) = selected.0 {
                let before_area = q_familiar_areas.get(fam_entity).ok().cloned();

                apply_task_area_to_familiar(
                    fam_entity,
                    Some(&new_area),
                    &mut commands,
                    &mut q_familiars,
                );
                info!(
                    "AREA_ASSIGNMENT: Familiar {:?} assigned to rectangular area",
                    fam_entity
                );

                let assigned_count = assign_unassigned_tasks_in_area(
                    &mut commands,
                    fam_entity,
                    &new_area,
                    &q_unassigned,
                );
                if assigned_count > 0 {
                    info!(
                        "AREA_ASSIGNMENT: Also assigned {} unassigned task(s) to Familiar {:?}",
                        assigned_count, fam_entity
                    );
                }

                area_edit_history.push(fam_entity, before_area, Some(new_area));
            }

            despawn_selection_indicators(&q_selection_indicator, &mut commands);

            if should_exit_after_apply(&keyboard) {
                task_context.0 = TaskMode::None;
                next_play_mode.set(PlayMode::Normal);
                info!("AREA_ASSIGNMENT: Applied and exited Area Edit mode");
            } else {
                task_context.0 = TaskMode::AreaSelection(None);
                info!("AREA_ASSIGNMENT: Applied and kept Area Edit mode");
            }
        }
        TaskMode::DesignateChop(Some(start_pos))
        | TaskMode::DesignateMine(Some(start_pos))
        | TaskMode::DesignateHaul(Some(start_pos)) => {
            let mode = task_context.0;
            let area = TaskArea::from_points(start_pos, WorldMap::snap_to_grid_edge(world_pos));
            let q_targets = q_target_sets.p0();
            let issued_by = selected
                .0
                .filter(|entity| q_familiars.contains(*entity));
            apply_designation_in_area(&mut commands, mode, &area, issued_by, &q_targets);
            task_context.0 = reset_designation_mode(mode);
        }
        TaskMode::CancelDesignation(Some(start_pos)) => {
            let end_pos = WorldMap::snap_to_grid_edge(world_pos);
            let drag_distance = start_pos.distance(end_pos);

            if drag_distance < TILE_SIZE * 0.5 {
                // クリックキャンセル: 最も近い Designation 持ちエンティティを個別キャンセル
                let mut closest: Option<(Entity, f32)> = None;
                {
                    let q_targets = q_target_sets.p0();
                    for (
                        entity,
                        transform,
                        _,
                        _,
                        _,
                        designation,
                        _task_workers,
                        _blueprint,
                        _,
                        _transport_request,
                        _fixed_source,
                        _,
                        _,
                        _,
                        _,
                    ) in q_targets.iter()
                    {
                        if designation.is_none() {
                            continue;
                        }
                        let pos = transform.translation.truncate();
                        let dist = pos.distance(start_pos);
                        if dist < TILE_SIZE {
                            if closest.is_none() || dist < closest.unwrap().1 {
                                closest = Some((entity, dist));
                            }
                        }
                    }
                }

                if let Some((target_entity, _)) = closest {
                    let q_targets = q_target_sets.p0();
                    if let Ok((
                        _,
                        _,
                        _,
                        _,
                        _,
                        _,
                        task_workers,
                        blueprint,
                        _,
                        transport_request,
                        fixed_source,
                        _,
                        _,
                        _,
                        _,
                    )) = q_targets.get(target_entity)
                    {
                        cancel_single_designation(
                            &mut commands,
                            target_entity,
                            task_workers,
                            blueprint.is_some(),
                            transport_request.is_some(),
                            fixed_source.map(|source| source.0),
                        );
                        info!(
                            "CANCEL: Click-cancelled designation on {:?}",
                            target_entity
                        );
                    }
                }

                // 床建築はエリア単位でのみキャンセルできるため、クリック時は
                // 近傍タイルから親サイトを解決してサイト全体キャンセルを要求する。
                let mut closest_floor_site: Option<(Entity, f32)> = None;
                let q_floor_tiles = q_target_sets.p1();
                for (_, transform, tile) in q_floor_tiles.iter() {
                    let dist = transform.translation.truncate().distance(start_pos);
                    if dist > TILE_SIZE {
                        continue;
                    }
                    match closest_floor_site {
                        Some((_, best_dist)) if best_dist <= dist => {}
                        _ => closest_floor_site = Some((tile.parent_site, dist)),
                    }
                }
                if let Some((site_entity, _)) = closest_floor_site {
                    commands
                        .entity(site_entity)
                        .insert(FloorConstructionCancelRequested);
                    info!(
                        "FLOOR_CANCEL: Requested site cancellation via click {:?}",
                        site_entity
                    );
                }
            } else {
                // エリアキャンセル
                let area = TaskArea::from_points(start_pos, end_pos);
                {
                    let q_targets = q_target_sets.p0();
                    apply_designation_in_area(
                        &mut commands,
                        TaskMode::CancelDesignation(Some(start_pos)),
                        &area,
                        selected.0,
                        &q_targets,
                    );
                }

                let mut requested_sites = HashSet::new();
                let q_floor_tiles = q_target_sets.p1();
                for (_, transform, tile) in q_floor_tiles.iter() {
                    let pos = transform.translation.truncate();
                    if area.contains(pos) {
                        requested_sites.insert(tile.parent_site);
                    }
                }
                for site_entity in requested_sites.iter().copied() {
                    commands
                        .entity(site_entity)
                        .insert(FloorConstructionCancelRequested);
                }
                if !requested_sites.is_empty() {
                    info!(
                        "FLOOR_CANCEL: Requested {} site(s) cancellation via area drag",
                        requested_sites.len()
                    );
                }
            }

            task_context.0 = TaskMode::CancelDesignation(None);
        }
        _ => {}
    }
}
