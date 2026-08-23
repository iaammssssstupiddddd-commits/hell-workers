use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

const CONSTRUCTION_MASK_SHADER: &str = "shaders/construction_mask_material.wgsl";

/// Shared transparent material for all floor/wall construction tile masks.
///
/// Per-instance state and progress are carried by `MeshTag`, so every tile can
/// retain automatic instancing without allocating a material asset per tile.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone, Default)]
pub struct ConstructionMaskMaterial {
    #[uniform(0)]
    _reserved: u32,
}

impl Material for ConstructionMaskMaterial {
    fn fragment_shader() -> ShaderRef {
        CONSTRUCTION_MASK_SHADER.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}
