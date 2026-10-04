use bevy::asset::{AssetLoader, LoadContext, io::Reader};
use bevy::prelude::*;

use super::schema::*;
use super::validation::{require, validate_payload, validate_receipt};
use super::{BuildingAssetSetError, decode_buildingset};

/// Root-owned opt-in, never deserialized from an asset's .meta settings.
/// Capture it before loader registration. Changing this Resource afterwards
/// does not silently authorize an already running loader.
#[derive(Resource, Default, Clone)]
pub struct BuildingAssetLoadPolicy {
    pub allowed_candidate: Option<BuildingAssetSetIdentity>,
}

impl BuildingAssetLoadPolicy {
    pub fn allows(&self, identity: &BuildingAssetSetIdentity) -> bool {
        match identity.authority {
            BuildingAssetAuthority::ReleaseApproved => true,
            BuildingAssetAuthority::ArtPreview if !cfg!(feature = "profiling") => false,
            _ => self.allowed_candidate.as_ref() == Some(identity),
        }
    }
}

#[derive(TypePath)]
pub struct BuildingAssetSetLoader {
    policy: BuildingAssetLoadPolicy,
}

impl FromWorld for BuildingAssetSetLoader {
    fn from_world(world: &mut World) -> Self {
        Self {
            policy: world
                .get_resource::<BuildingAssetLoadPolicy>()
                .cloned()
                .unwrap_or_default(),
        }
    }
}

impl AssetLoader for BuildingAssetSetLoader {
    type Asset = BuildingAssetSetManifest;
    type Settings = ();
    type Error = BuildingAssetSetError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _: &(),
        context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let manifest = decode_buildingset(&bytes)?;
        require(
            self.policy.allows(&manifest.identity),
            "building asset authority is not enabled for this exact identity",
        )?;
        if let Some(record) = &manifest.receipt {
            let payload = context
                .read_asset_bytes(record.path.clone())
                .await
                .map_err(|error| BuildingAssetSetError(error.to_string()))?;
            validate_payload(record, &payload)?;
            validate_receipt(&manifest, &payload)?;
        }
        for record in &manifest.artifacts {
            let payload = context
                .read_asset_bytes(record.path.clone())
                .await
                .map_err(|error| BuildingAssetSetError(error.to_string()))?;
            validate_payload(record, &payload)?;
        }
        Ok(manifest)
    }

    fn extensions(&self) -> &[&str] {
        &["buildingset"]
    }
}
