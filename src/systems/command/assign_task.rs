use super::TaskMode;
use crate::entities::familiar::Familiar;
use crate::game_state::TaskContext;
use crate::interface::camera::MainCamera;
use crate::interface::selection::SelectedEntity;
use crate::systems::jobs::{Designation, IssuedBy};
use crate::systems::task_queue::{GlobalTaskQueue, PendingTask, TaskQueue};
use crate::world::map::WorldMap;
use crate::world::pathfinding::{self, PathfindingContext};
use bevy::prelude::*;

pub fn assign_task_system(
    buttons: Res<ButtonInput<MouseButton>>,
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    q_ui: Query<&Interaction, With<Button>>,
    selected: Res<SelectedEntity>,
    mut task_context: ResMut<TaskContext>,
    mut global_queue: ResMut<GlobalTaskQueue>,
    mut queue: ResMut<TaskQueue>,
    mut commands: Commands,
    q_designations: Query<(Entity, &Transform, &Designation), Without<IssuedBy>>,
    q_familiars: Query<(Entity, &Transform), With<Familiar>>,
    world_map: Res<WorldMap>,
    mut pf_context: ResMut<PathfindingContext>,
) {
    if q_ui.iter().any(|i| *i != Interaction::None) {
        return;
    }

    let TaskMode::AssignTask(Some(start_pos)) = task_context.0 else {
        return;
    };

    if !buttons.just_released(MouseButton::Left) {
        return;
    }

    info!("ASSIGN_TASK: Drag released, processing assignment...");

    let Ok((camera, camera_transform)) = q_camera.single() else {
        return;
    };
    let Ok(window) = q_window.single() else {
        return;
    };
    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };
    let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) else {
        return;
    };

    let Some(fam_entity) = selected.0 else {
        info!("ASSIGN_TASK: No entity selected");
        task_context.0 = TaskMode::AssignTask(None);
        return;
    };

    let Ok((_, fam_transform)) = q_familiars.get(fam_entity) else {
        info!(
            "ASSIGN_TASK: Selected entity {:?} is not a familiar",
            fam_entity
        );
        task_context.0 = TaskMode::AssignTask(None);
        return;
    };

    let min_x = f32::min(start_pos.x, world_pos.x);
    let max_x = f32::max(start_pos.x, world_pos.x);
    let min_y = f32::min(start_pos.y, world_pos.y);
    let max_y = f32::max(start_pos.y, world_pos.y);

    info!(
        "ASSIGN_TASK: Searching in area ({:.1},{:.1}) to ({:.1},{:.1})",
        min_x, min_y, max_x, max_y
    );

    // パス検索の起点を「通行可能な地面」に補正する
    // 使い魔は空中を飛べるが、ワーカーは地面しか歩けないため。
    let Some(actual_start_grid) = world_map.get_nearest_walkable_grid(fam_transform.translation.truncate()) else {
        info!("ASSIGN_TASK: Familiar is in very deep obstacles, skipping assignment...");
        task_context.0 = TaskMode::AssignTask(None);
        return;
    };

    let mut assigned_count = 0;

    for (entity, transform, designation) in q_designations.iter() {
        let pos = transform.translation.truncate();
        if pos.x >= min_x - 0.1
            && pos.x <= max_x + 0.1
            && pos.y >= min_y - 0.1
            && pos.y <= max_y + 0.1
        {
            // 地面周辺から到達可能かチェック（逆引き検索: タスクから地面へ）
            let target_grid = WorldMap::world_to_grid(pos);
            let is_reachable = if world_map.is_walkable(target_grid.0, target_grid.1) {
                pathfinding::find_path(&world_map, &mut *pf_context, target_grid, actual_start_grid).is_some()
            } else {
                // pathfinding.rs 内部で neighbor -> actual_start_grid の逆引きが行われる
                pathfinding::find_path_to_adjacent(&world_map, &mut *pf_context, actual_start_grid, target_grid).is_some()
            };

            if !is_reachable {
                info!(
                    "ASSIGN_TASK: Skipping task {:?} (unreachable from ground near Familiar)",
                    entity
                );
                continue;
            }

            commands.entity(entity).insert(IssuedBy(fam_entity));

            global_queue.remove(entity);
            queue.add(
                fam_entity,
                PendingTask {
                    entity,
                    work_type: designation.work_type,
                    priority: 0,
                },
            );

            assigned_count += 1;
            info!(
                "ASSIGN_TASK: Assigned {:?} ({:?}) to Familiar {:?}",
                entity, designation.work_type, fam_entity
            );
        }
    }

    if assigned_count > 0 {
        info!(
            "ASSIGN_TASK: Assigned {} task(s) to Familiar {:?}",
            assigned_count, fam_entity
        );
    } else {
        info!("ASSIGN_TASK: No valid tasks found in selected area");
    }

    task_context.0 = TaskMode::AssignTask(None);
}
