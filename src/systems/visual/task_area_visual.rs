use crate::game_state::TaskContext;
use crate::interface::camera::MainCamera;
use crate::interface::selection::{HoveredEntity, SelectedEntity};
use crate::systems::command::{TaskArea, TaskMode};
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::sprite_render::Material2d;
use bevy::shader::ShaderRef;

/// タスクエリア用のカスタムマテリアル
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct TaskAreaMaterial {
    #[uniform(0)]
    pub color: LinearRgba, // 16 bytes
    #[uniform(0)]
    pub size: Vec2,        // 8 bytes (align 8)
    #[uniform(0)]
    pub time: f32,         // 4 bytes
    #[uniform(0)]
    pub state: u32,        // 4 bytes
}

impl Material2d for TaskAreaMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/task_area.wgsl".into()
    }

    fn alpha_mode(&self) -> bevy::sprite_render::AlphaMode2d {
        bevy::sprite_render::AlphaMode2d::Blend
    }

    fn depth_bias(&self) -> f32 {
        1.0 // 他の要素の上に確実に表示されるように
    }
}

/// タスクエリア表示用コンポーネント（メッシュエンティティ側に付与）
#[derive(Component)]
pub struct TaskAreaVisual {
    pub familiar: Entity,
}

pub fn update_task_area_material_system(
    time: Res<Time>,
    q_visuals: Query<(&TaskAreaVisual, &MeshMaterial2d<TaskAreaMaterial>)>,
    mut materials: ResMut<Assets<TaskAreaMaterial>>,
    q_familiars: Query<(Entity, &TaskArea)>,
    selected: Res<SelectedEntity>,
    hovered_entity: Res<HoveredEntity>,
    task_context: Res<TaskContext>,
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
) {
    let Ok(window) = q_window.single() else {
        return;
    };
    let Ok((camera, camera_transform)) = q_camera.single() else {
        return;
    };
    let cursor_pos = window
        .cursor_position()
        .and_then(|pos: Vec2| camera.viewport_to_world_2d(camera_transform, pos).ok());

    let editing_mode = matches!(task_context.0, TaskMode::AreaSelection(_));

    for (visual, material_handle) in q_visuals.iter() {
        if let Some(material) = materials.get_mut(&material_handle.0) {
            material.time = time.elapsed_secs();

            if let Ok((fam_entity, area)) = q_familiars.get(visual.familiar) {
                material.size = area.size();

                let is_selected = selected.0 == Some(fam_entity);

                // 強調条件: 境界線をホバーしているか、使い魔本体をホバーしているか
                let is_border_hovered =
                    cursor_pos.map_or(false, |pos| area.contains_border(pos, 6.0));
                let is_familiar_hovered = hovered_entity.0 == Some(fam_entity);
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
