use super::geometry::world_cursor_pos;
use crate::game_state::TaskContext;
use crate::interface::camera::MainCamera;
use crate::systems::command::AreaSelectionIndicator;
use crate::systems::visual::task_area_visual::TaskAreaMaterial;
use crate::world::map::WorldMap;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

pub fn area_selection_indicator_system(
    task_context: Res<TaskContext>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    mut q_indicator: Query<
        (
            &mut Transform,
            &MeshMaterial2d<TaskAreaMaterial>,
            &mut Visibility,
        ),
        With<AreaSelectionIndicator>,
    >,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<TaskAreaMaterial>>,
) {
    let drag_start = super::geometry::get_drag_start(task_context.0);

    if let Some(start_pos) = drag_start
        && let Some(world_pos) = world_cursor_pos(&q_window, &q_camera)
    {
        let end_pos: Vec2 = WorldMap::snap_to_grid_edge(world_pos);
        let center: Vec2 = (start_pos + end_pos) / 2.0;
        let size: Vec2 = (start_pos - end_pos).abs();
        let color = super::geometry::get_indicator_color(task_context.0);

        if let Some((mut transform, material_handle, mut visibility)) = q_indicator.iter_mut().next()
        {
            transform.translation = center.extend(0.6);
            transform.scale = size.extend(1.0);
            if let Some(material) = materials.get_mut(&material_handle.0) {
                material.color = color;
                material.size = size;
                material.state = 3; // Editing state (dashed border)
            }
            *visibility = Visibility::Visible;
        } else {
            commands.spawn((
                AreaSelectionIndicator,
                Mesh2d(meshes.add(Rectangle::default().mesh())),
                MeshMaterial2d(materials.add(TaskAreaMaterial {
                    color,
                    size,
                    time: 0.0,
                    state: 3,
                })),
                Transform::from_translation(center.extend(0.6)).with_scale(size.extend(1.0)),
                Visibility::Visible,
            ));
        }
        return;
    }

    if let Some((_, _, mut visibility)) = q_indicator.iter_mut().next() {
        *visibility = Visibility::Hidden;
    }
}
