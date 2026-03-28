use bevy::pbr::Material;
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct SoulShadowMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
}

impl Default for SoulShadowMaterial {
    fn default() -> Self {
        Self {
            color: LinearRgba::WHITE,
        }
    }
}

impl Material for SoulShadowMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/soul_shadow_material.wgsl".into()
    }

    fn prepass_fragment_shader() -> ShaderRef {
        "shaders/soul_shadow_prepass.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Opaque
    }
}
