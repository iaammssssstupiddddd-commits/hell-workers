use super::{TaskArea, TaskMode};
use crate::entities::familiar::Familiar;
use crate::game_state::TaskContext;
use crate::interface::camera::MainCamera;
use crate::interface::selection::SelectedEntity;
use crate::interface::ui::UiInputState;
use crate::systems::jobs::Designation;
use crate::world::map::WorldMap;
use crate::world::pathfinding::{self, PathfindingContext};
use bevy::prelude::*;

pub fn assign_task_system(
    buttons: Res<ButtonInput<MouseButton>>,
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    ui_input_state: Res<UiInputState>,
    selected: Res<SelectedEntity>,
    mut task_context: ResMut<TaskContext>,
    mut commands: Commands,
    q_designations: Query<
        (Entity, &Transform, &Designation),
        Without<crate::relationships::ManagedBy>,
    >,
    q_familiars: Query<(Entity, &Transform), With<Familiar>>,
    world_map: Res<WorldMap>,
    mut pf_context: Local<PathfindingContext>,
) {
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
        let is_reachable = if world_map.is_walkable(target_grid.0, target_grid.1) {
            pathfinding::find_path(&world_map, &mut pf_context, target_grid, actual_start_grid)
                .is_some()
        } else {
            // pathfinding.rs 内部で neighbor -> actual_start_grid の逆引きが行われる
            pathfinding::find_path_to_adjacent(
                &world_map,
                &mut pf_context,
                actual_start_grid,
                target_grid,
            )
            .is_some()
        };

        if !is_reachable {
            continue;
        }

        commands.entity(entity).insert((
            crate::relationships::ManagedBy(fam_entity),
            crate::systems::jobs::Priority(0),
        ));
    }

    task_context.0 = TaskMode::AssignTask(None);
}
