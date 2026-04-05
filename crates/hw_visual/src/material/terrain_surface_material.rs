use bevy::pbr::{ExtendedMaterial, MaterialExtension, OpaqueRendererMethod, StandardMaterial};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH, TILE_SIZE};

use super::section_material::SectionCut;

#[derive(Clone, Copy, Debug, ShaderType, Reflect)]
pub struct TerrainSurfaceUniform {
    pub cut_position: Vec4,
    pub cut_normal: Vec4,
    pub thickness: f32,
    pub cut_active: f32,
    pub map_world_width: f32,
    pub map_world_height: f32,
    pub uv_scale: f32,
    pub blend_strength: f32,
    pub macro_noise_scale: f32,
    pub overlay_scale: f32,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct TerrainSurfaceMaterialExt {
    #[uniform(100)]
    pub uniforms: TerrainSurfaceUniform,
    #[texture(101)]
    pub terrain_id_map: Option<Handle<Image>>,
    #[texture(102)]
    pub terrain_feature_map: Option<Handle<Image>>,
    #[texture(103)]
    #[sampler(104)]
    pub grass_albedo: Option<Handle<Image>>,
    #[texture(105)]
    #[sampler(106)]
    pub dirt_albedo: Option<Handle<Image>>,
    #[texture(107)]
    #[sampler(108)]
    pub sand_albedo: Option<Handle<Image>>,
    #[texture(109)]
    #[sampler(110)]
    pub river_albedo: Option<Handle<Image>>,
    #[texture(111)]
    #[sampler(112)]
    pub terrain_macro_noise: Option<Handle<Image>>,
    #[texture(113)]
    #[sampler(114)]
    pub grass_macro_overlay: Option<Handle<Image>>,
    #[texture(115)]
    #[sampler(116)]
    pub dirt_macro_overlay: Option<Handle<Image>>,
    #[texture(117)]
    #[sampler(118)]
    pub sand_macro_overlay: Option<Handle<Image>>,
    #[texture(119)]
    #[sampler(120)]
    pub terrain_blend_mask_soft: Option<Handle<Image>>,
    #[texture(121)]
    #[sampler(122)]
    pub river_flow_noise: Option<Handle<Image>>,
    #[texture(123)]
    #[sampler(124)]
    pub river_normal_like: Option<Handle<Image>>,
    #[texture(125)]
    #[sampler(126)]
    pub shoreline_detail: Option<Handle<Image>>,
    #[texture(127)]
    #[sampler(128)]
    pub terrain_feature_lut: Option<Handle<Image>>,
}

impl Default for TerrainSurfaceMaterialExt {
    fn default() -> Self {
        Self {
            uniforms: TerrainSurfaceUniform {
                cut_position: Vec4::ZERO,
                cut_normal: Vec3::NEG_Z.extend(0.0),
                thickness: TILE_SIZE * 5.0,
                cut_active: 0.0,
                map_world_width: MAP_WIDTH as f32 * TILE_SIZE,
                map_world_height: MAP_HEIGHT as f32 * TILE_SIZE,
                uv_scale: 1.0 / TILE_SIZE,
                blend_strength: 1.0,
                macro_noise_scale: 0.00045,
                overlay_scale: 0.0012,
            },
            terrain_id_map: None,
            terrain_feature_map: None,
            grass_albedo: None,
            dirt_albedo: None,
            sand_albedo: None,
            river_albedo: None,
            terrain_macro_noise: None,
            grass_macro_overlay: None,
            dirt_macro_overlay: None,
            sand_macro_overlay: None,
            terrain_blend_mask_soft: None,
            river_flow_noise: None,
            river_normal_like: None,
            shoreline_detail: None,
            terrain_feature_lut: None,
        }
    }
}

impl MaterialExtension for TerrainSurfaceMaterialExt {
    fn fragment_shader() -> ShaderRef {
        "shaders/terrain_surface_material.wgsl".into()
    }

    fn prepass_fragment_shader() -> ShaderRef {
        "shaders/terrain_surface_material_prepass.wgsl".into()
    }
}

pub type TerrainSurfaceMaterial = ExtendedMaterial<StandardMaterial, TerrainSurfaceMaterialExt>;

pub fn make_terrain_surface_material(
    extension: TerrainSurfaceMaterialExt,
) -> TerrainSurfaceMaterial {
    TerrainSurfaceMaterial {
        base: StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 1.0,
            reflectance: 0.0,
            opaque_render_method: OpaqueRendererMethod::Forward,
            ..default()
        },
        extension,
    }
}

pub fn sync_section_cut_to_terrain_surface_system(
    cut: Res<SectionCut>,
    mut materials: ResMut<Assets<TerrainSurfaceMaterial>>,
) {
    if !cut.is_changed() {
        return;
    }

    let cut_position = cut.position.extend(0.0);
    let cut_normal = cut.normal.normalize_or_zero().extend(0.0);
    let thickness = cut.thickness.max(0.0);
    let cut_active = if cut.active { 1.0 } else { 0.0 };

    for (_, material) in materials.iter_mut() {
        let uniforms = &mut material.extension.uniforms;
        uniforms.cut_position = cut_position;
        uniforms.cut_normal = cut_normal;
        uniforms.thickness = thickness;
        uniforms.cut_active = cut_active;
    }
}
