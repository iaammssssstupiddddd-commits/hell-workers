use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::sprite_render::Material2d;

/// タスクエリア用のカスタムマテリアル
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct TaskAreaMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    #[uniform(0)]
    pub size: Vec2,
    #[uniform(0)]
    pub time: f32,
    #[uniform(0)]
    pub state: u32,
}

impl Material2d for TaskAreaMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/task_area.wgsl".into()
    }

    fn alpha_mode(&self) -> bevy::sprite_render::AlphaMode2d {
        bevy::sprite_render::AlphaMode2d::Blend
    }

    fn depth_bias(&self) -> f32 {
        1.0
    }
}

/// タスクエリア表示用コンポーネント（メッシュエンティティ側に付与）
#[derive(Component)]
pub struct TaskAreaVisual {
    pub familiar: Entity,
}
