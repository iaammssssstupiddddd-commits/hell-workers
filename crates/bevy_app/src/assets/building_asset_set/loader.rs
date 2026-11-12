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
    #[cfg(feature = "profiling")]
    comparison_candidates: Vec<BuildingAssetSetIdentity>,
}

impl FromWorld for BuildingAssetSetLoader {
    fn from_world(world: &mut World) -> Self {
        Self {
            #[cfg(feature = "profiling")]
            comparison_candidates: world
                .get_resource::<super::M6Comparison>()
                .filter(|comparison| comparison.pre_release && comparison.candidate)
                .map(|comparison| comparison.identities.clone())
                .unwrap_or_default(),
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
            self.allows(&manifest.identity),
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

impl BuildingAssetSetLoader {
    fn allows(&self, identity: &BuildingAssetSetIdentity) -> bool {
        #[cfg(feature = "profiling")]
        if self.comparison_candidates.contains(identity) {
            return true;
        }
        self.policy.allows(identity)
    }
}

#[cfg(all(test, feature = "profiling"))]
mod comparison_tests {
    use super::*;

    #[test]
    fn comparison_policy_is_captured_and_control_does_not_admit_candidates() {
        let identity = BuildingAssetSetIdentity {
            kind: BuildingAssetKind::Tank,
            generation: 1,
            authority: BuildingAssetAuthority::IsolatedCandidate,
            manifest_sha256: "a".repeat(64),
        };
        let mut world = World::new();
        let mut comparison = super::super::M6Comparison {
            candidate: true,
            pre_release: true,
            identities: vec![identity.clone()],
        };
        world.insert_resource(comparison.clone());
        let loader = BuildingAssetSetLoader::from_world(&mut world);
        assert!(loader.allows(&identity));
        comparison.candidate = false;
        world.insert_resource(comparison.clone());
        assert!(loader.allows(&identity));
        assert!(!BuildingAssetSetLoader::from_world(&mut world).allows(&identity));
        comparison.candidate = true;
        comparison.pre_release = false;
        world.insert_resource(comparison);
        assert!(!BuildingAssetSetLoader::from_world(&mut world).allows(&identity));
        let mut foreign = identity;
        foreign.generation += 1;
        assert!(!loader.allows(&foreign));
    }
}
