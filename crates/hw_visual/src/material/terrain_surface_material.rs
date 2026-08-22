use bevy::material::OpaqueRendererMethod;
use bevy::pbr::{ExtendedMaterial, MaterialExtension, StandardMaterial};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use hw_core::constants::{
    MAP_HEIGHT, MAP_WIDTH, TILE_SIZE, topdown_shadow_style_blur, topdown_shadow_style_params,
    topdown_shadow_style_tint,
};

#[derive(Clone, Copy, Debug, ShaderType, Reflect)]
pub struct TerrainSurfaceUniform {
    pub map_world_width: f32,
    pub map_world_height: f32,
    pub uv_scale: f32,
    pub blend_strength: f32,
    pub macro_noise_scale: f32,
    pub overlay_scale: f32,
    pub lut_shore: Vec4,
    pub lut_inland: Vec4,
    pub lut_rock: Vec4,
    pub feature_lut_constants_ready: f32,
    /// `x`: effect mix, `y`: shadow threshold, `z`: softness, `w`: full-shadow darken
    pub shadow_style_params: Vec4,
    /// `rgb`: shadow tint target, `a`: tint strength
    pub shadow_style_tint: Vec4,
    /// `x`: blur radius in shadow texels, `yzw`: reserved
    pub shadow_style_blur: Vec4,
    /// `x`: tile size, `y`: local-light gain, `z`: enabled, `w`: reserved.
    pub indoor_light_params: Vec4,
}

impl Default for TerrainSurfaceUniform {
    fn default() -> Self {
        Self {
            map_world_width: MAP_WIDTH as f32 * TILE_SIZE,
            map_world_height: MAP_HEIGHT as f32 * TILE_SIZE,
            uv_scale: 1.0 / TILE_SIZE,
            blend_strength: 1.0,
            macro_noise_scale: 0.00045,
            overlay_scale: 0.0012,
            // terrain_feature_lut の 0.5 は tint/roughness 的に neutral 扱い。
            lut_shore: Vec4::splat(0.5),
            lut_inland: Vec4::splat(0.5),
            lut_rock: Vec4::splat(0.5),
            feature_lut_constants_ready: 0.0,
            shadow_style_params: topdown_shadow_style_params(),
            shadow_style_tint: topdown_shadow_style_tint(),
            shadow_style_blur: topdown_shadow_style_blur(),
            indoor_light_params: Vec4::new(TILE_SIZE, 1.0, 1.0, 0.0),
        }
    }
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
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
    #[texture(129)]
    #[sampler(130)]
    pub boundary_mask: Option<Handle<Image>>,
    #[texture(131)]
    #[sampler(132)]
    pub boundary_proximity_mask: Option<Handle<Image>>,
    #[texture(133)]
    #[sampler(134)]
    pub indoor_light_field: Option<Handle<Image>>,
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

// ── LOD1-lite variant ────────────────────────────────────────────────────────

/// LOD1-lite 用マテリアル拡張。バインドグループレイアウトは
/// `TerrainSurfaceMaterialExt` と同一で、フラグメントシェーダーに
/// `terrain_surface_material_lod1_lite.wgsl` を使用する。
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct TerrainSurfaceMaterialExtLod1Lite {
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
    #[texture(129)]
    #[sampler(130)]
    pub boundary_mask: Option<Handle<Image>>,
    #[texture(131)]
    #[sampler(132)]
    pub boundary_proximity_mask: Option<Handle<Image>>,
    #[texture(133)]
    #[sampler(134)]
    pub indoor_light_field: Option<Handle<Image>>,
}

impl MaterialExtension for TerrainSurfaceMaterialExtLod1Lite {
    fn fragment_shader() -> ShaderRef {
        "shaders/terrain_surface_material_lod1_lite.wgsl".into()
    }

    fn prepass_fragment_shader() -> ShaderRef {
        "shaders/terrain_surface_material_prepass.wgsl".into()
    }
}

pub type TerrainSurfaceMaterialLod1Lite =
    ExtendedMaterial<StandardMaterial, TerrainSurfaceMaterialExtLod1Lite>;

pub fn make_terrain_surface_material_lod1_lite(
    extension: TerrainSurfaceMaterialExtLod1Lite,
) -> TerrainSurfaceMaterialLod1Lite {
    TerrainSurfaceMaterialLod1Lite {
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

// ── LOD2 variant ─────────────────────────────────────────────────────────────

/// LOD2 用マテリアル拡張。バインドグループレイアウトは `TerrainSurfaceMaterialExt` と同一だが、
/// フラグメントシェーダーに簡略版 (`terrain_surface_material_lod2.wgsl`) を使用する。
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct TerrainSurfaceMaterialExtLod2 {
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
    #[texture(129)]
    #[sampler(130)]
    pub boundary_mask: Option<Handle<Image>>,
    #[texture(131)]
    #[sampler(132)]
    pub boundary_proximity_mask: Option<Handle<Image>>,
    #[texture(133)]
    #[sampler(134)]
    pub indoor_light_field: Option<Handle<Image>>,
}

impl MaterialExtension for TerrainSurfaceMaterialExtLod2 {
    fn fragment_shader() -> ShaderRef {
        "shaders/terrain_surface_material_lod2.wgsl".into()
    }

    fn prepass_fragment_shader() -> ShaderRef {
        "shaders/terrain_surface_material_prepass.wgsl".into()
    }
}

pub type TerrainSurfaceMaterialLod2 =
    ExtendedMaterial<StandardMaterial, TerrainSurfaceMaterialExtLod2>;

pub fn make_terrain_surface_material_lod2(
    extension: TerrainSurfaceMaterialExtLod2,
) -> TerrainSurfaceMaterialLod2 {
    TerrainSurfaceMaterialLod2 {
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

#[derive(Resource, Clone)]
pub struct TerrainSurfaceLutImageHandle(pub Handle<Image>);

#[derive(Resource, Default)]
pub struct TerrainFeatureLutUniformSyncState {
    pub done: bool,
}

fn decode_lut_pixel(pixel: &[u8]) -> Vec4 {
    if pixel.len() < 4 {
        return Vec4::splat(0.5);
    }
    LinearRgba::from(Color::srgba_u8(pixel[0], pixel[1], pixel[2], pixel[3])).to_vec4()
}

fn extract_feature_lut_constants(image: &Image) -> Option<(Vec4, Vec4, Vec4)> {
    let data = image.data.as_ref()?;
    let idx = |n: usize| n.checked_mul(4);
    let shore_start = idx(1)?;
    let inland_start = idx(2)?;
    let rock_start = idx(3)?;
    let shore = decode_lut_pixel(data.get(shore_start..shore_start + 4)?);
    let inland = decode_lut_pixel(data.get(inland_start..inland_start + 4)?);
    let rock = decode_lut_pixel(data.get(rock_start..rock_start + 4)?);
    Some((shore, inland, rock))
}

fn apply_lut_constants(
    uniforms: &mut TerrainSurfaceUniform,
    shore: Vec4,
    inland: Vec4,
    rock: Vec4,
) {
    uniforms.lut_shore = shore;
    uniforms.lut_inland = inland;
    uniforms.lut_rock = rock;
    uniforms.feature_lut_constants_ready = 1.0;
}

pub fn sync_terrain_feature_lut_uniforms_system(
    lut_handle: Option<Res<TerrainSurfaceLutImageHandle>>,
    images: Res<Assets<Image>>,
    mut mats_lod1: ResMut<Assets<TerrainSurfaceMaterial>>,
    mut mats_lod1_lite: ResMut<Assets<TerrainSurfaceMaterialLod1Lite>>,
    mut mats_lod2: ResMut<Assets<TerrainSurfaceMaterialLod2>>,
    mut state: ResMut<TerrainFeatureLutUniformSyncState>,
) {
    if state.done {
        return;
    }
    let Some(lut_handle) = lut_handle else {
        return;
    };
    let Some(image) = images.get(&lut_handle.0) else {
        return;
    };
    let Some((shore, inland, rock)) = extract_feature_lut_constants(image) else {
        return;
    };

    for (_, material) in mats_lod1.iter_mut() {
        apply_lut_constants(&mut material.extension.uniforms, shore, inland, rock);
    }
    for (_, material) in mats_lod1_lite.iter_mut() {
        apply_lut_constants(&mut material.extension.uniforms, shore, inland, rock);
    }
    for (_, material) in mats_lod2.iter_mut() {
        apply_lut_constants(&mut material.extension.uniforms, shore, inland, rock);
    }

    state.done = true;
}

#[cfg(test)]
mod tests {
    #[test]
    fn visual_receivers_bind_but_do_not_sample_light_field_at_startup() {
        let structural_source =
            include_str!("../../../../assets/shaders/topdown_structural_material.wgsl");
        assert!(structural_source.contains("@binding(111) var indoor_light_field"));
        assert!(structural_source.contains("@binding(112) var indoor_light_sampler"));
        assert!(!structural_source.contains("let local_light = sample_indoor_light_field("));

        let terrain_sources = [
            include_str!("../../../../assets/shaders/terrain_surface_material.wgsl"),
            include_str!("../../../../assets/shaders/terrain_surface_material_lod1_lite.wgsl"),
            include_str!("../../../../assets/shaders/terrain_surface_material_lod2.wgsl"),
        ];

        for source in terrain_sources {
            // Keep the shared P06 resource topology, but do not reactivate the fragment
            // reads: they make normal startup terrain or structures produce no scene pixels.
            assert!(source.contains("@binding(133) var indoor_light_field"));
            assert!(source.contains("@binding(134) var indoor_light_sampler"));
            assert!(!source.contains("let local_light = sample_indoor_light_field("));
        }
    }
}
