use super::{TaskArea, TaskMode};
use crate::app_contexts::TaskContext;
use crate::entities::familiar::Familiar;
use crate::interface::selection::SelectedEntity;
use crate::interface::ui::UiInputState;
use crate::systems::jobs::Designation;
use crate::world::map::{WorldMap, WorldMapRead};
use crate::world::pathfinding::{self, PathfindingContext};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_ui::camera::MainCamera;

#[derive(SystemParam)]
pub struct AssignTaskInput<'w, 's> {
    buttons: Res<'w, ButtonInput<MouseButton>>,
    q_window: Query<'w, 's, &'static Window, With<bevy::window::PrimaryWindow>>,
    q_camera: Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<MainCamera>>,
    ui_input_state: Res<'w, UiInputState>,
}

#[derive(SystemParam)]
pub struct AssignTaskWorkerQuery<'w, 's> {
    q_designations: Query<
        'w,
        's,
        (Entity, &'static Transform, &'static Designation),
        Without<hw_core::relationships::ManagedBy>,
    >,
    q_familiars: Query<'w, 's, (Entity, &'static Transform), With<Familiar>>,
}

pub fn assign_task_system(
    input: AssignTaskInput,
    selected: Res<SelectedEntity>,
    mut task_context: ResMut<TaskContext>,
    mut commands: Commands,
    worker_queries: AssignTaskWorkerQuery,
    world_map: WorldMapRead,
    mut pf_context: Local<PathfindingContext>,
) {
    let AssignTaskInput {
        buttons,
        q_window,
        q_camera,
        ui_input_state,
    } = input;
    let AssignTaskWorkerQuery {
        q_designations,
        q_familiars,
    } = worker_queries;
    if ui_input_state.pointer_over_ui {
        return;
    }

    let TaskMode::AssignTask(Some(start_pos)) = task_context.0 else {
        return;
    };

    if !buttons.just_released(MouseButton::Left) {
        return;
    }

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
        task_context.0 = TaskMode::AssignTask(None);
        return;
    };

    let Ok((_, fam_transform)) = q_familiars.get(fam_entity) else {
        task_context.0 = TaskMode::AssignTask(None);
        return;
    };

    let selection_area = TaskArea::from_points(start_pos, world_pos);

    // パス検索の起点を「通行可能な地面」に補正する
    // 使い魔は空中を飛べるが、ワーカーは地面しか歩けないため。
    let Some(actual_start_grid) =
        world_map.get_nearest_walkable_grid(fam_transform.translation.truncate())
    else {
        task_context.0 = TaskMode::AssignTask(None);
        return;
    };

    for (entity, transform, _) in q_designations.iter() {
        let pos = transform.translation.truncate();
        if !selection_area.contains_with_margin(pos, 0.1) {
            continue;
        }

        // 地面周辺から到達可能かチェック（逆引き検索: タスクから地面へ）
        let target_grid = WorldMap::world_to_grid(pos);
        let is_reachable = pathfinding::can_reach_target(
            world_map.as_ref(),
            &mut pf_context,
            actual_start_grid,
            target_grid,
            world_map.is_walkable(target_grid.0, target_grid.1),
        );

        if !is_reachable {
            continue;
        }

        commands.entity(entity).insert((
            hw_core::relationships::ManagedBy(fam_entity),
            crate::systems::jobs::Priority(0),
        ));
    }

    task_context.0 = TaskMode::AssignTask(None);
}
