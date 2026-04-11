pub mod character_material;
pub mod section_material;
pub mod soul_mask_material;
pub mod soul_shadow_material;
pub mod terrain_surface_material;

pub use character_material::{CharacterMaterial, soul_face_uv_offset, soul_face_uv_scale};
pub use section_material::{
    SectionCut, SectionMaterial, TERRAIN_DIRT_BRIGHTNESS_VARIATION_STRENGTH,
    TERRAIN_DIRT_DOMAIN_WARP_STRENGTH, TERRAIN_GRASS_BRIGHTNESS_VARIATION_STRENGTH,
    TERRAIN_GRASS_DOMAIN_WARP_STRENGTH, TERRAIN_GRASS_UV_DISTORT_STRENGTH, TERRAIN_KIND_DIRT,
    TERRAIN_KIND_GRASS, TERRAIN_KIND_RIVER, TERRAIN_KIND_SAND,
    TERRAIN_SAND_BRIGHTNESS_VARIATION_STRENGTH, TERRAIN_SAND_DOMAIN_WARP_STRENGTH,
    TerrainMaterialMaps, make_section_material, make_section_material_textured,
    make_terrain_section_material, sync_section_cut_to_materials_system, with_alpha_mode,
};
pub use soul_mask_material::SoulMaskMaterial;
pub use soul_shadow_material::SoulShadowMaterial;
pub use terrain_surface_material::{
    TerrainFeatureLutUniformSyncState, TerrainSurfaceLutImageHandle, TerrainSurfaceMaterial,
    TerrainSurfaceMaterialExt, TerrainSurfaceMaterialExtLod1Lite, TerrainSurfaceMaterialExtLod2,
    TerrainSurfaceMaterialLod1Lite, TerrainSurfaceMaterialLod2, TerrainSurfaceUniform,
    make_terrain_surface_material, make_terrain_surface_material_lod1_lite,
    make_terrain_surface_material_lod2, sync_section_cut_to_terrain_surface_lod1_lite_system,
    sync_section_cut_to_terrain_surface_lod2_system, sync_section_cut_to_terrain_surface_system,
    sync_terrain_feature_lut_uniforms_system,
};
