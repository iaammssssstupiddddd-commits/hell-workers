use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::render::render_resource::RenderPipelineDescriptor;
use bevy::shader::ShaderRef;
use bevy::sprite_render::Material2d;
use bevy::ui_render::prelude::{UiMaterial, UiMaterialKey};

/// World 空間用 Dream 泡マテリアル（Mesh2d + Material2d）
///
/// ソフトグロー・虹色屈折・スペキュラ・リム発光・ノイズ変形をシェーダーで描画する。
/// 質量（mass）に応じて変形の強さが変わる。
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct DreamBubbleMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    #[uniform(0)]
    pub time: f32,
    #[uniform(0)]
    pub alpha: f32,
    #[uniform(0)]
    pub mass: f32,
}

impl Material2d for DreamBubbleMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/dream_bubble.wgsl".into()
    }

    fn alpha_mode(&self) -> bevy::sprite_render::AlphaMode2d {
        bevy::sprite_render::AlphaMode2d::Blend
    }
}

/// UI 空間用 Dream 泡マテリアル（MaterialNode + UiMaterial）
///
/// 質量に応じて形状が変化する：
/// - mass < 3.0: 1泡（ノイズ変形のみ）
/// - mass < 6.0: 2泡クラスター
/// - mass >= 6.0: 3泡クラスター
///
/// 各サブ泡はそれぞれ独立した輪郭線（リム発光）を持つ。
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct DreamBubbleUiMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    #[uniform(0)]
    pub time: f32,
    #[uniform(0)]
    pub alpha: f32,
    #[uniform(0)]
    pub mass: f32,
    #[uniform(0)]
    pub velocity_dir: Vec2,
}

impl UiMaterial for DreamBubbleUiMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/dream_bubble_ui.wgsl".into()
    }

    fn specialize(_descriptor: &mut RenderPipelineDescriptor, _key: UiMaterialKey<Self>) {}
}
