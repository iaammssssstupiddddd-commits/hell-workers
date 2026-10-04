use bevy::prelude::*;

use super::residency::{TypedRoles, loaded};
use super::{
    BuildingAssetKind, BuildingAssetPoolFailure, BuildingAssetRequest, BuildingAssetSetError,
    BuildingAssetSetIdentity, BuildingAssetSetManifest,
};

struct Generation {
    identity: BuildingAssetSetIdentity,
    manifest: Handle<BuildingAssetSetManifest>,
    roles: Option<TypedRoles>,
    descriptor: Option<BuildingAssetDescriptor>,
}

impl Generation {
    fn ready(
        &mut self,
        server: &AssetServer,
        manifests: &Assets<BuildingAssetSetManifest>,
        meshes: &Assets<Mesh>,
        images: &Assets<Image>,
    ) -> Result<bool, BuildingAssetSetError> {
        if !loaded(server, &self.manifest)? {
            return Ok(false);
        }
        let Some(manifest) = manifests.get(&self.manifest) else {
            return Ok(false);
        };
        if manifest.identity != self.identity {
            return Err(BuildingAssetSetError(
                "loaded manifest identity differs".into(),
            ));
        }
        let roles = self
            .roles
            .get_or_insert_with(|| TypedRoles::request(manifest, server));
        roles.ready(server, meshes, images)
    }
}

#[derive(Default)]
struct KindPool {
    active: Option<Generation>,
    pending: Option<Generation>,
    highest_attempted: u64,
    failure: Option<BuildingAssetPoolFailure>,
}

/// Immutable, fully resident generation shared by every presentation consumer.
/// Handles and metadata are published together, never from a pending manifest.
#[derive(Clone)]
pub struct BuildingAssetDescriptor {
    pub manifest: BuildingAssetSetManifest,
    meshes: Vec<Handle<Mesh>>,
    images: Vec<Handle<Image>>,
}

impl BuildingAssetDescriptor {
    pub fn mesh(&self, role: &str) -> Option<&Handle<Mesh>> {
        self.manifest
            .identity
            .kind
            .mesh_roles()
            .iter()
            .position(|value| *value == role)
            .and_then(|index| self.meshes.get(index))
    }

    pub fn image(&self, role: &str) -> Option<&Handle<Image>> {
        self.manifest
            .identity
            .kind
            .image_roles()
            .iter()
            .position(|value| *value == role)
            .and_then(|index| self.images.get(index))
    }
}

/// Opt-in, kind-local ownership of at most one active and one pending set.
/// The root presentation boundary owns polling; requesting a set remains opt-in.
/// Pool replacement drops retired strong handles immediately; AssetServer/GPU
/// reclamation can occur later. No world entities or retired cache are retained.
#[derive(Resource, Default)]
pub struct BuildingAssetPool {
    kinds: [KindPool; 9],
    #[cfg(feature = "profiling")]
    acceptance_identity: Option<BuildingAssetSetIdentity>,
}

fn index(kind: BuildingAssetKind) -> usize {
    match kind {
        BuildingAssetKind::Tank => 0,
        BuildingAssetKind::MudMixer => 1,
        BuildingAssetKind::RestArea => 2,
        BuildingAssetKind::SoulSpa => 3,
        BuildingAssetKind::WheelbarrowParking => 4,
        BuildingAssetKind::SandPile => 5,
        BuildingAssetKind::BonePile => 6,
        BuildingAssetKind::OutdoorLamp => 7,
        BuildingAssetKind::Bridge => 8,
    }
}

impl BuildingAssetPool {
    #[cfg(feature = "profiling")]
    pub(crate) fn active_identities(&self) -> Vec<&BuildingAssetSetIdentity> {
        self.kinds
            .iter()
            .filter_map(|kind| kind.active.as_ref().map(|set| &set.identity))
            .collect()
    }

    /// References owned by this pool only, not global Arc counts, GPU residency
    /// or presentation/material cache handles. Retired generations are absent.
    #[cfg(feature = "profiling")]
    pub(crate) fn owned_handle_count(&self) -> usize {
        self.kinds
            .iter()
            .flat_map(|kind| [&kind.active, &kind.pending])
            .flatten()
            .map(|generation| {
                1 + generation
                    .roles
                    .as_ref()
                    .map_or(0, |roles| roles.meshes.len() + roles.images.len())
                    + generation
                        .descriptor
                        .as_ref()
                        .map_or(0, |set| set.meshes.len() + set.images.len())
            })
            .sum()
    }

    #[cfg(feature = "profiling")]
    pub(crate) fn generation_counts(&self) -> (usize, usize) {
        (
            self.kinds
                .iter()
                .filter(|kind| kind.active.is_some())
                .count(),
            self.kinds
                .iter()
                .filter(|kind| kind.pending.is_some())
                .count(),
        )
    }

    #[cfg(feature = "profiling")]
    pub(super) fn allow_acceptance_identity(&mut self, identity: BuildingAssetSetIdentity) {
        self.acceptance_identity = Some(identity);
    }

    pub(crate) fn presentation_allowed(&self, identity: &BuildingAssetSetIdentity) -> bool {
        if identity.authority == super::BuildingAssetAuthority::ReleaseApproved {
            return true;
        }
        #[cfg(feature = "profiling")]
        if self.acceptance_identity.as_ref() == Some(identity) {
            return true;
        }
        false
    }

    /// The locator must be loaded through the registered policy-enforcing
    /// BuildingAssetSetLoader. Exact identity is checked before typed requests.
    /// Failed or retired generations require a new generation number to retry.
    pub fn request(
        &mut self,
        identity: BuildingAssetSetIdentity,
        path: &str,
        server: &AssetServer,
    ) -> BuildingAssetRequest {
        let pool = &mut self.kinds[index(identity.kind)];
        if identity.generation <= pool.highest_attempted {
            return BuildingAssetRequest::AlreadyAttempted;
        }
        if pool.pending.is_some() {
            return BuildingAssetRequest::PendingOccupied;
        }
        pool.highest_attempted = identity.generation;
        pool.pending = Some(Generation {
            identity,
            manifest: server.load(path.to_owned()),
            roles: None,
            descriptor: None,
        });
        BuildingAssetRequest::Started
    }

    /// Pending failure leaves active untouched. All required typed assets must
    /// be loaded AND resident before the entire generation replaces active.
    pub fn poll(
        &mut self,
        server: &AssetServer,
        manifests: &Assets<BuildingAssetSetManifest>,
        meshes: &Assets<Mesh>,
        images: &Assets<Image>,
    ) {
        for pool in &mut self.kinds {
            // Explicit asset removal / reload failure invalidates the whole set.
            // Never publish a preview while its geometry is no longer resident.
            if pool.active.as_mut().is_some_and(|active| {
                !matches!(active.ready(server, manifests, meshes, images), Ok(true))
            }) {
                pool.active = None;
            }
            let Some(mut pending) = pool.pending.take() else {
                continue;
            };
            match pending.ready(server, manifests, meshes, images) {
                Ok(false) => pool.pending = Some(pending),
                Ok(true) => {
                    let manifest = manifests.get(&pending.manifest).expect("ready manifest");
                    let roles = pending.roles.as_ref().expect("ready typed roles");
                    pending.descriptor = Some(BuildingAssetDescriptor {
                        manifest: manifest.clone(),
                        meshes: roles.meshes.clone(),
                        images: roles
                            .images
                            .iter()
                            .map(|(handle, _)| handle.clone())
                            .collect(),
                    });
                    pool.active = Some(pending);
                    pool.failure = None;
                }
                Err(error) => {
                    pool.failure = Some(BuildingAssetPoolFailure {
                        identity: pending.identity,
                        reason: error.to_string(),
                    });
                    // The manifest and every typed handle drop here.
                }
            }
        }
    }

    /// Revocation is identity-bound so a late invalidation cannot remove a
    /// newer active set. None means fallback; other kinds and pending survive.
    pub fn invalidate_active(&mut self, identity: &BuildingAssetSetIdentity) -> bool {
        let pool = &mut self.kinds[index(identity.kind)];
        if pool
            .active
            .as_ref()
            .is_some_and(|active| &active.identity == identity)
        {
            pool.active = None;
            true
        } else {
            false
        }
    }

    pub fn active(&self, kind: BuildingAssetKind) -> Option<&BuildingAssetSetIdentity> {
        self.kinds[index(kind)]
            .active
            .as_ref()
            .map(|set| &set.identity)
    }

    /// Only active metadata/handles escape the pool. The caller still selects
    /// the authority appropriate to its presentation context.
    pub fn descriptor(&self, kind: BuildingAssetKind) -> Option<&BuildingAssetDescriptor> {
        self.kinds[index(kind)].active.as_ref()?.descriptor.as_ref()
    }

    #[cfg(all(test, feature = "profiling"))]
    pub(crate) fn install_clay_presentation_fixture(
        &mut self,
        kind: BuildingAssetKind,
        generation: u64,
    ) -> BuildingAssetDescriptor {
        let descriptor = self.install_presentation_fixture(
            kind,
            generation,
            super::BuildingAssetAuthority::ArtPreview,
        );
        self.allow_acceptance_identity(descriptor.manifest.identity.clone());
        descriptor
    }

    #[cfg(test)]
    pub(crate) fn install_presentation_fixture(
        &mut self,
        kind: BuildingAssetKind,
        generation: u64,
        authority: super::BuildingAssetAuthority,
    ) -> BuildingAssetDescriptor {
        let mut manifest = super::tests::fixture(kind, authority);
        manifest.identity.generation = generation;
        manifest.world_preview.anchor_px = [128.0, 192.0];
        // Different kinds must not alias in a mixed-world consumer regression.
        let kind_namespace = (index(kind) as u128) << 96;
        let meshes = kind
            .mesh_roles()
            .iter()
            .enumerate()
            .map(|(index, _)| {
                bevy::asset::uuid::Uuid::from_u128(
                    kind_namespace + 100 + generation as u128 * 100 + index as u128,
                )
                .into()
            })
            .collect();
        let images = kind
            .image_roles()
            .iter()
            .enumerate()
            .map(|(index, _)| {
                bevy::asset::uuid::Uuid::from_u128(
                    kind_namespace + 1000 + generation as u128 * 100 + index as u128,
                )
                .into()
            })
            .collect();
        let descriptor = BuildingAssetDescriptor {
            manifest,
            meshes,
            images,
        };
        self.kinds[index(kind)].active = Some(Generation {
            identity: descriptor.manifest.identity.clone(),
            manifest: Handle::default(),
            roles: None,
            descriptor: Some(descriptor.clone()),
        });
        descriptor
    }

    pub fn pending(&self, kind: BuildingAssetKind) -> Option<&BuildingAssetSetIdentity> {
        self.kinds[index(kind)]
            .pending
            .as_ref()
            .map(|set| &set.identity)
    }

    pub fn failure(&self, kind: BuildingAssetKind) -> Option<&BuildingAssetPoolFailure> {
        self.kinds[index(kind)].failure.as_ref()
    }
}

#[cfg(test)]
mod tests;
