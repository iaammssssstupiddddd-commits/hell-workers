pub mod character_material;
pub mod section_material;
pub mod soul_mask_material;
pub mod soul_shadow_material;

pub use character_material::{CharacterMaterial, soul_face_uv_offset, soul_face_uv_scale};
pub use section_material::{
    SectionCut, SectionMaterial, TERRAIN_GRASS_BRIGHTNESS_VARIATION_STRENGTH,
    TERRAIN_GRASS_UV_DISTORT_STRENGTH, make_section_material, make_section_material_textured,
    make_terrain_section_material, sync_section_cut_to_materials_system, with_alpha_mode,
};
pub use soul_mask_material::SoulMaskMaterial;
pub use soul_shadow_material::SoulShadowMaterial;
