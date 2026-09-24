//! Typed residency is deliberately separate from the manifest loader's byte checks.

use bevy::asset::{LoadState, RecursiveDependencyLoadState};
use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;

use super::{BuildingAssetSetError, BuildingAssetSetManifest};

pub(super) struct TypedRoles {
    meshes: Vec<Handle<Mesh>>,
    images: Vec<(Handle<Image>, Option<[u32; 2]>)>,
}

impl TypedRoles {
    pub(super) fn request(manifest: &BuildingAssetSetManifest, server: &AssetServer) -> Self {
        let mut meshes = Vec::new();
        let mut images = Vec::new();
        for record in &manifest.artifacts {
            if record.role.starts_with("mesh:") {
                // Bevy 0.19: Mesh0 is GltfMesh; Mesh0/Primitive0 is Mesh.
                meshes.push(
                    server.load(
                        GltfAssetLabel::Primitive {
                            mesh: 0,
                            primitive: 0,
                        }
                        .from_asset(record.path.clone()),
                    ),
                );
            } else {
                let role = record.role.strip_prefix("image:").unwrap_or_default();
                let dimensions = [&manifest.world_preview, &manifest.catalog_preview]
                    .into_iter()
                    .find(|preview| preview.image_role == role)
                    .map(|preview| preview.canvas_px);
                images.push((server.load(record.path.clone()), dimensions));
            }
        }
        Self { meshes, images }
    }

    pub(super) fn ready(
        &self,
        server: &AssetServer,
        meshes: &Assets<Mesh>,
        images: &Assets<Image>,
    ) -> Result<bool, BuildingAssetSetError> {
        let mut ready = true;
        // Check every role even when an earlier role is still loading: failure
        // must release the pending set promptly, not hide behind a late asset.
        for handle in &self.meshes {
            ready &= loaded(server, handle)? && meshes.get(handle).is_some();
        }
        for (handle, dimensions) in &self.images {
            let loaded = loaded(server, handle)?;
            if let Some(image) = images.get(handle) {
                if dimensions.is_some_and(|expected| image.size().to_array() != expected) {
                    return Err(BuildingAssetSetError(
                        "preview image dimensions differ".into(),
                    ));
                }
                ready &= loaded;
            } else {
                ready = false;
            }
        }
        Ok(ready)
    }
}

pub(super) fn loaded<A: Asset>(
    server: &AssetServer,
    handle: &Handle<A>,
) -> Result<bool, BuildingAssetSetError> {
    if let LoadState::Failed(error) = server.load_state(handle.id()) {
        return Err(BuildingAssetSetError(error.to_string()));
    }
    if let Some(RecursiveDependencyLoadState::Failed(error)) =
        server.get_recursive_dependency_load_state(handle.id())
    {
        return Err(BuildingAssetSetError(error.to_string()));
    }
    Ok(server.is_loaded_with_dependencies(handle.id()))
}
