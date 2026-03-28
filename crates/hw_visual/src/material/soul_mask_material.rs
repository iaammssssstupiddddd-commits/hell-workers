use bevy::pbr::Material;
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct SoulMaskMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    pub alpha_mode: AlphaMode,
}

impl SoulMaskMaterial {
    pub fn solid_white() -> Self {
        Self {
            color: LinearRgba::WHITE,
            alpha_mode: AlphaMode::Blend,
        }
    }
}

impl Material for SoulMaskMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/soul_mask_material.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        self.alpha_mode
    }
}
