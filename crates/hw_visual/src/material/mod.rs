pub mod terrain_surface_material;
pub mod topdown_structural_material;

pub use terrain_surface_material::{
    TerrainFeatureLutUniformSyncState, TerrainSurfaceLutImageHandle, TerrainSurfaceMaterial,
    TerrainSurfaceMaterialExt, TerrainSurfaceMaterialExtLod1Lite, TerrainSurfaceMaterialExtLod2,
    TerrainSurfaceMaterialLod1Lite, TerrainSurfaceMaterialLod2, TerrainSurfaceUniform,
    make_terrain_surface_material, make_terrain_surface_material_lod1_lite,
    make_terrain_surface_material_lod2, sync_terrain_feature_lut_uniforms_system,
};
pub use topdown_structural_material::{
    TopDownStructuralMaterial, TopDownStructuralMaterialExt, TopDownStructuralUniform,
    make_topdown_structural_material, with_topdown_alpha_mode,
};
