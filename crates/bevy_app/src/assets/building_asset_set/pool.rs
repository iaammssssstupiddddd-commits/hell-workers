use bevy::prelude::*;

use super::residency::{TypedRoles, loaded};
use super::{
    BuildingAssetKind, BuildingAssetSetError, BuildingAssetSetIdentity, BuildingAssetSetManifest,
};

/// Admission result, not an assertion that the requested generation is ready.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildingAssetRequest {
    Started,
    /// A generation is already preparing; it is never displaced implicitly.
    PendingOccupied,
    /// Generations are immutable and strictly increasing per kind, including failures.
    AlreadyAttempted,
}

#[derive(Debug)]
pub struct BuildingAssetPoolFailure {
    pub identity: BuildingAssetSetIdentity,
    pub reason: String,
}

struct Generation {
    identity: BuildingAssetSetIdentity,
    manifest: Handle<BuildingAssetSetManifest>,
    roles: Option<TypedRoles>,
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

/// Opt-in, kind-local ownership of at most one active and one pending set.
/// No plugin, startup request, system registration or presentation handles are
/// exported. The future presentation boundary must own when `poll` is called.
/// Pool replacement drops retired strong handles immediately; AssetServer/GPU
/// reclamation can occur later. No world entities or retired cache are retained.
#[derive(Resource, Default)]
pub struct BuildingAssetPool {
    kinds: [KindPool; 8],
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
    }
}

impl BuildingAssetPool {
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
            let Some(mut pending) = pool.pending.take() else {
                continue;
            };
            match pending.ready(server, manifests, meshes, images) {
                Ok(false) => pool.pending = Some(pending),
                Ok(true) => {
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
