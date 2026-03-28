use bevy::pbr::Material;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

#[derive(Clone, Copy, Debug, ShaderType)]
pub struct CharacterMaterialUniform {
    pub base_color: Vec4,
    pub secondary_color: Vec4,
    pub uv_scale: Vec2,
    pub uv_offset: Vec2,
    pub alpha_cutoff: f32,
    pub ghost_alpha: f32,
    pub rim_strength: f32,
    pub posterize_steps: f32,
    pub curve_strength: f32,
    pub material_kind: f32,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct CharacterMaterial {
    #[uniform(0)]
    pub params: CharacterMaterialUniform,
    #[texture(1)]
    #[sampler(2)]
    pub color_texture: Handle<Image>,
    pub alpha_mode: AlphaMode,
}

impl CharacterMaterial {
    pub fn face(
        color_texture: Handle<Image>,
        base_color: LinearRgba,
        uv_scale: Vec2,
        uv_offset: Vec2,
    ) -> Self {
        Self {
            params: CharacterMaterialUniform {
                base_color: base_color.to_vec4(),
                secondary_color: LinearRgba::WHITE.to_vec4(),
                uv_scale,
                uv_offset,
                alpha_cutoff: 0.01,
                ghost_alpha: 1.0,
                rim_strength: 0.0,
                posterize_steps: 1.0,
                curve_strength: 0.0,
                material_kind: 0.0,
            },
            color_texture,
            alpha_mode: AlphaMode::Blend,
        }
    }

    pub fn body(color_texture: Handle<Image>) -> Self {
        Self {
            params: CharacterMaterialUniform {
                base_color: LinearRgba::new(0.78, 0.9, 1.0, 1.0).to_vec4(),
                secondary_color: LinearRgba::new(0.42, 0.6, 0.8, 1.0).to_vec4(),
                uv_scale: Vec2::ONE,
                uv_offset: Vec2::ZERO,
                alpha_cutoff: 0.01,
                ghost_alpha: 1.0,
                rim_strength: 0.28,
                posterize_steps: 4.0,
                curve_strength: 1.0,
                material_kind: 1.0,
            },
            color_texture,
            alpha_mode: AlphaMode::Opaque,
        }
    }
}

impl Material for CharacterMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/character_material.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        self.alpha_mode
    }
}
