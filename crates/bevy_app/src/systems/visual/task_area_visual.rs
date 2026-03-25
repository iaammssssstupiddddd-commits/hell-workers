pub use hw_visual::task_area_visual::{TaskAreaMaterial, TaskAreaVisual};

use crate::app_contexts::TaskContext;
use crate::interface::selection::{HoveredEntity, SelectedEntity};
use crate::systems::command::{TaskArea, TaskMode};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_ui::camera::MainCamera;

#[derive(SystemParam)]
pub struct TaskAreaContext<'w, 's> {
    pub selected: Res<'w, SelectedEntity>,
    pub hovered_entity: Res<'w, HoveredEntity>,
    pub task_context: Res<'w, TaskContext>,
    pub q_window: Query<'w, 's, &'static Window, With<bevy::window::PrimaryWindow>>,
    pub q_camera: Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<MainCamera>>,
}

pub fn update_task_area_material_system(
    time: Res<Time>,
    q_visuals: Query<(&TaskAreaVisual, &MeshMaterial2d<TaskAreaMaterial>)>,
    mut materials: ResMut<Assets<TaskAreaMaterial>>,
    q_familiars: Query<(Entity, &TaskArea)>,
    ctx: TaskAreaContext,
) {
    let Ok(window) = ctx.q_window.single() else {
        return;
    };
    let Ok((camera, camera_transform)) = ctx.q_camera.single() else {
        return;
    };
    let cursor_pos = window
        .cursor_position()
        .and_then(|pos: Vec2| camera.viewport_to_world_2d(camera_transform, pos).ok());

    let editing_mode = matches!(ctx.task_context.0, TaskMode::AreaSelection(_));

    for (visual, material_handle) in q_visuals.iter() {
        if let Some(material) = materials.get_mut(&material_handle.0) {
            material.time = time.elapsed_secs();

            if let Ok((fam_entity, area)) = q_familiars.get(visual.familiar) {
                material.size = area.size();

                let is_selected = ctx.selected.0 == Some(fam_entity);

                // 強調条件: 境界線をホバーしているか、使い魔本体をホバーしているか
                let is_border_hovered =
                    cursor_pos.is_some_and(|pos| area.contains_border(pos, 6.0));
                let is_familiar_hovered = ctx.hovered_entity.0 == Some(fam_entity);
                let is_hovered = is_border_hovered || is_familiar_hovered;

                let state = if editing_mode && is_selected {
                    3 // Editing
                } else if is_selected {
                    2 // Selected
                } else if is_hovered {
                    1 // Hover
                } else {
                    0 // Idle
                };
                material.state = state;
            }
        }
    }
}
