use bevy::pbr::Material;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

pub const SOUL_FACE_ATLAS_COLUMNS: f32 = 3.0;
pub const SOUL_FACE_ATLAS_ROWS: f32 = 2.0;
pub const SOUL_FACE_CELL_SIZE_PX: f32 = 256.0;
pub const SOUL_FACE_BASE_CROP_OFFSET_X_PX: f32 = 24.0;
pub const SOUL_FACE_BASE_CROP_OFFSET_Y_PX: f32 = 32.0;
pub const SOUL_FACE_BASE_CROP_SIZE_PX: f32 = 152.0;
pub const SOUL_FACE_TEXTURE_MAGNIFICATION: f32 = 1.4;

pub fn soul_face_uv_scale() -> Vec2 {
    Vec2::new(
        SOUL_FACE_BASE_CROP_SIZE_PX
            / SOUL_FACE_TEXTURE_MAGNIFICATION
            / SOUL_FACE_CELL_SIZE_PX
            / SOUL_FACE_ATLAS_COLUMNS,
        SOUL_FACE_BASE_CROP_SIZE_PX
            / SOUL_FACE_TEXTURE_MAGNIFICATION
            / SOUL_FACE_CELL_SIZE_PX
            / SOUL_FACE_ATLAS_ROWS,
    )
}

pub fn soul_face_uv_offset(col: f32, row: f32) -> Vec2 {
    Vec2::new(
        (col * SOUL_FACE_CELL_SIZE_PX
            + SOUL_FACE_BASE_CROP_OFFSET_X_PX
            + (SOUL_FACE_BASE_CROP_SIZE_PX
                - SOUL_FACE_BASE_CROP_SIZE_PX / SOUL_FACE_TEXTURE_MAGNIFICATION)
                * 0.5)
            / SOUL_FACE_CELL_SIZE_PX
            / SOUL_FACE_ATLAS_COLUMNS,
        (row * SOUL_FACE_CELL_SIZE_PX
            + SOUL_FACE_BASE_CROP_OFFSET_Y_PX
            + (SOUL_FACE_BASE_CROP_SIZE_PX
                - SOUL_FACE_BASE_CROP_SIZE_PX / SOUL_FACE_TEXTURE_MAGNIFICATION)
                * 0.5)
            / SOUL_FACE_CELL_SIZE_PX
            / SOUL_FACE_ATLAS_ROWS,
    )
}

#[derive(Clone, Copy, Debug, ShaderType)]
pub struct CharacterMaterialUniform {
    pub base_color: Vec4,
    pub secondary_color: Vec4,
    pub sun_light_dir: Vec4,
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
    fn topdown_sun_light_dir() -> Vec4 {
        hw_core::constants::topdown_sun_direction_world().extend(0.0)
    }

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
                sun_light_dir: Self::topdown_sun_light_dir(),
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
                sun_light_dir: Self::topdown_sun_light_dir(),
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

    pub fn set_face_uv_offset(&mut self, uv_offset: Vec2) {
        self.params.uv_offset = uv_offset;
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
