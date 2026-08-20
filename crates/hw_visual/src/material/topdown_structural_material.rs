use bevy::material::OpaqueRendererMethod;
use bevy::pbr::{ExtendedMaterial, MaterialExtension, StandardMaterial};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use hw_core::constants::{
    TILE_SIZE, topdown_shadow_style_blur, topdown_shadow_style_params, topdown_shadow_style_tint,
};

#[derive(Clone, Copy, Debug, ShaderType, Reflect)]
pub struct TopDownStructuralUniform {
    pub build_progress: f32,
    pub wall_height: f32,
    /// `x`: effect mix, `y`: shadow threshold, `z`: softness, `w`: full-shadow darken.
    pub shadow_style_params: Vec4,
    /// `rgb`: shadow tint target, `a`: tint strength.
    pub shadow_style_tint: Vec4,
    /// `x`: blur radius in shadow texels, `yzw`: reserved.
    pub shadow_style_blur: Vec4,
    /// `x`: tile size, `y`: local-light gain, `z`: enabled, `w`: reserved.
    pub indoor_light_params: Vec4,
}

impl Default for TopDownStructuralUniform {
    fn default() -> Self {
        Self {
            build_progress: 1.0,
            wall_height: 0.0,
            shadow_style_params: topdown_shadow_style_params(),
            shadow_style_tint: topdown_shadow_style_tint(),
            shadow_style_blur: topdown_shadow_style_blur(),
            indoor_light_params: Vec4::new(TILE_SIZE, 1.0, 1.0, 0.0),
        }
    }
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct TopDownStructuralMaterialExt {
    #[uniform(100)]
    pub uniforms: TopDownStructuralUniform,
    /// Shared linear RGBA8 indoor Light Field. Every structural material
    /// handle points at the image owned by the root visual bridge.
    #[texture(111)]
    #[sampler(112)]
    pub indoor_light_field: Option<Handle<Image>>,
}

impl MaterialExtension for TopDownStructuralMaterialExt {
    fn fragment_shader() -> ShaderRef {
        "shaders/topdown_structural_material.wgsl".into()
    }

    fn prepass_fragment_shader() -> ShaderRef {
        "shaders/topdown_structural_material_prepass.wgsl".into()
    }
}

pub type TopDownStructuralMaterial =
    ExtendedMaterial<StandardMaterial, TopDownStructuralMaterialExt>;

pub fn make_topdown_structural_material(
    base_color: LinearRgba,
    indoor_light_field: Handle<Image>,
) -> TopDownStructuralMaterial {
    TopDownStructuralMaterial {
        base: StandardMaterial {
            base_color: Color::linear_rgba(
                base_color.red,
                base_color.green,
                base_color.blue,
                base_color.alpha,
            ),
            perceptual_roughness: 1.0,
            reflectance: 0.0,
            opaque_render_method: OpaqueRendererMethod::Forward,
            ..default()
        },
        extension: TopDownStructuralMaterialExt {
            uniforms: TopDownStructuralUniform::default(),
            indoor_light_field: Some(indoor_light_field),
        },
    }
}

pub fn with_topdown_alpha_mode(
    mut material: TopDownStructuralMaterial,
    alpha_mode: AlphaMode,
) -> TopDownStructuralMaterial {
    material.base.alpha_mode = alpha_mode;
    material
}
