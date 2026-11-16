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
    /// `x`: tile size, `y`: local-light gain, `z`: enabled,
    /// `w`: normal flattening [0,1]; 2 is the closed view-space pilot,
    /// 3 is admitted world-preview body paint with RtT vertical compensation;
    /// 4 is object-attached UV ink with a flat diffuse reference normal.
    /// Projection modes use shadow_style_params as canvas/anchor and tint as matte.
    pub indoor_light_params: Vec4,
}

/// CPU-side meaning of the existing packed surface mode; the GPU ABI is unchanged.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StructuralSurfaceMode {
    Lit { flatten: f32 },
    PilotProjection { canvas: Vec4, matte: Vec4 },
    PreviewProjection { canvas: Vec4, matte: Vec4 },
    UvInk,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StructuralShadowStyle {
    pub params: Vec4,
    pub tint: Vec4,
}

impl TopDownStructuralUniform {
    pub fn shadow_style(&self) -> StructuralShadowStyle {
        StructuralShadowStyle {
            params: self.shadow_style_params,
            tint: self.shadow_style_tint,
        }
    }

    /// Projection reuses the shadow fields, so callers must supply the original style.
    pub fn set_surface_mode(&mut self, mode: StructuralSurfaceMode, shadow: StructuralShadowStyle) {
        let (encoded, params, tint) = match mode {
            StructuralSurfaceMode::Lit { flatten } => (flatten, shadow.params, shadow.tint),
            StructuralSurfaceMode::PilotProjection { canvas, matte } => (2.0, canvas, matte),
            StructuralSurfaceMode::PreviewProjection { canvas, matte } => (3.0, canvas, matte),
            StructuralSurfaceMode::UvInk => (4.0, shadow.params, shadow.tint),
        };
        self.indoor_light_params.w = encoded;
        self.shadow_style_params = params;
        self.shadow_style_tint = tint;
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_modes_preserve_unrelated_fields_and_restore_original_shadow() {
        let mut uniform = TopDownStructuralUniform {
            build_progress: 0.37,
            wall_height: 42.0,
            shadow_style_params: Vec4::new(0.1, 0.2, 0.3, 0.4),
            shadow_style_tint: Vec4::new(0.5, 0.6, 0.7, 0.8),
            shadow_style_blur: Vec4::splat(0.9),
            indoor_light_params: Vec4::new(10.0, 2.0, 1.0, 0.0),
        };
        let shadow = uniform.shadow_style();
        let canvas = Vec4::new(100.0, 120.0, 0.4, 0.6);
        let matte = Vec4::new(0.2, 0.3, 0.4, 1.0);
        for (mode, expected, projected) in [
            (StructuralSurfaceMode::Lit { flatten: 0.0 }, 0.0, false),
            (StructuralSurfaceMode::Lit { flatten: 0.82 }, 0.82, false),
            (
                StructuralSurfaceMode::PilotProjection { canvas, matte },
                2.0,
                true,
            ),
            (
                StructuralSurfaceMode::PreviewProjection { canvas, matte },
                3.0,
                true,
            ),
            (StructuralSurfaceMode::UvInk, 4.0, false),
            (StructuralSurfaceMode::Lit { flatten: 0.0 }, 0.0, false),
        ] {
            uniform.set_surface_mode(mode, shadow);
            assert_eq!(
                uniform.indoor_light_params,
                Vec4::new(10.0, 2.0, 1.0, expected)
            );
            assert_eq!(
                uniform.shadow_style_params,
                if projected { canvas } else { shadow.params }
            );
            assert_eq!(
                uniform.shadow_style_tint,
                if projected { matte } else { shadow.tint }
            );
            assert_eq!(uniform.shadow_style_blur, Vec4::splat(0.9));
            assert_eq!(uniform.build_progress, 0.37);
            assert_eq!(uniform.wall_height, 42.0);
        }
    }
}
