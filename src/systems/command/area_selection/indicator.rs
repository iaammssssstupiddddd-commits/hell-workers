use super::geometry::world_cursor_pos;
use crate::game_state::TaskContext;
use crate::interface::camera::MainCamera;
use crate::systems::command::{AreaSelectionIndicator, TaskMode};
use crate::world::map::WorldMap;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

pub fn area_selection_indicator_system(
    task_context: Res<TaskContext>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    q_window: Query<&Window, With<PrimaryWindow>>,
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

    if let Some(start_pos) = drag_start
        && let Some(world_pos) = world_cursor_pos(&q_window, &q_camera)
    {
        let end_pos = WorldMap::snap_to_grid_edge(world_pos);
        let center = (start_pos + end_pos) / 2.0;
        let size = (start_pos - end_pos).abs();

        let color = match task_context.0 {
            TaskMode::AreaSelection(_) => Color::srgba(1.0, 1.0, 1.0, 0.2),
            TaskMode::CancelDesignation(_) => Color::srgba(1.0, 0.2, 0.2, 0.3),
            _ => Color::srgba(0.2, 1.0, 0.2, 0.3),
        };

        if let Ok((_, mut transform, mut sprite, mut visibility)) = q_indicator.single_mut() {
            transform.translation = center.extend(0.6);
            sprite.custom_size = Some(size);
            sprite.color = color;
            *visibility = Visibility::Visible;
        } else {
            commands.spawn((
                AreaSelectionIndicator,
                Sprite {
                    color,
                    custom_size: Some(size),
                    ..default()
                },
                Transform::from_translation(center.extend(0.6)),
            ));
        }
        return;
    }

    if let Ok((_, _, _, mut visibility)) = q_indicator.single_mut() {
        *visibility = Visibility::Hidden;
    }
}
