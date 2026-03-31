use bevy::pbr::{ExtendedMaterial, MaterialExtension, OpaqueRendererMethod, StandardMaterial};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use hw_core::constants::TILE_SIZE;

#[derive(Clone, Copy, Debug, ShaderType, Reflect)]
pub struct SectionMaterialUniform {
    pub cut_position: Vec4,
    pub cut_normal: Vec4,
    pub thickness: f32,
    pub cut_active: f32,
    pub build_progress: f32,
    pub wall_height: f32,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct SectionMaterialExt {
    #[uniform(100)]
    pub uniforms: SectionMaterialUniform,
}

impl Default for SectionMaterialExt {
    fn default() -> Self {
        Self {
            uniforms: SectionMaterialUniform {
                cut_position: Vec4::ZERO,
                cut_normal: Vec3::NEG_Z.extend(0.0),
                thickness: TILE_SIZE * 5.0,
                cut_active: 0.0,
                build_progress: 1.0,
                wall_height: 0.0,
            },
        }
    }
}

impl MaterialExtension for SectionMaterialExt {
    fn fragment_shader() -> ShaderRef {
        "shaders/section_material.wgsl".into()
    }

    fn prepass_fragment_shader() -> ShaderRef {
        "shaders/section_material_prepass.wgsl".into()
    }
}

pub type SectionMaterial = ExtendedMaterial<StandardMaterial, SectionMaterialExt>;

pub fn make_section_material(base_color: LinearRgba) -> SectionMaterial {
    SectionMaterial {
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
        extension: SectionMaterialExt::default(),
    }
}

/// テクスチャ付き `SectionMaterial` を生成するヘルパー。
/// テレインタイルのように `Handle<Image>` をベースカラーとして使いたい場合に利用する。
pub fn make_section_material_textured(texture: Handle<Image>) -> SectionMaterial {
    SectionMaterial {
        base: StandardMaterial {
            base_color_texture: Some(texture),
            perceptual_roughness: 1.0,
            reflectance: 0.0,
            opaque_render_method: OpaqueRendererMethod::Forward,
            ..default()
        },
        extension: SectionMaterialExt::default(),
    }
}

pub fn with_alpha_mode(mut material: SectionMaterial, alpha_mode: AlphaMode) -> SectionMaterial {
    material.base.alpha_mode = alpha_mode;
    material
}

/// 矢視モードの切断スラブ設定。
#[derive(Resource, Clone, Copy, Debug)]
pub struct SectionCut {
    pub position: Vec3,
    pub normal: Vec3,
    pub thickness: f32,
    pub active: bool,
}

impl Default for SectionCut {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            normal: Vec3::NEG_Z,
            thickness: TILE_SIZE * 5.0,
            active: false,
        }
    }
}

pub fn sync_section_cut_to_materials_system(
    cut: Res<SectionCut>,
    mut materials: ResMut<Assets<SectionMaterial>>,
) {
    if !cut.is_changed() {
        return;
    }

    let cut_position = cut.position.extend(0.0);
    let cut_normal = cut.normal.normalize_or_zero().extend(0.0);
    let thickness = cut.thickness.max(0.0);
    let cut_active = if cut.active { 1.0 } else { 0.0 };

    for (_, material) in materials.iter_mut() {
        material.extension.uniforms.cut_position = cut_position;
        material.extension.uniforms.cut_normal = cut_normal;
        material.extension.uniforms.thickness = thickness;
        material.extension.uniforms.cut_active = cut_active;
    }
}
