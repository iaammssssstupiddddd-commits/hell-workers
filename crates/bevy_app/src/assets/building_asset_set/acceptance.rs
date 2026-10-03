//! Explicit profiling-only admission for the building-art native recipe.
//! Ordinary startup never chooses a locator or issues a load request.

use std::path::{Component, PathBuf};

use bevy::prelude::*;
use serde::Deserialize;

use super::*;

pub(super) mod bridge_probe;
mod m2_fixture;
mod m2_lifecycle;
mod m2_probe;

#[derive(Resource, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildingArtSession {
    pub mode: String,
    pub identity: BuildingAssetSetIdentity,
    pub locator: String,
    pub status_path: PathBuf,
    pub nonce: String,
}

pub fn configure_building_art(app: &mut App) -> Result<(), String> {
    let Ok(value) = std::env::var("HW_BUILDING_ART_SESSION") else {
        return Ok(());
    };
    let session: BuildingArtSession = serde_json::from_str(&value).map_err(|e| e.to_string())?;
    let authority = match session.mode.as_str() {
        "feedback" | "art-preview" => BuildingAssetAuthority::ArtPreview,
        "candidate" => BuildingAssetAuthority::IsolatedCandidate,
        _ => return Err("unknown building-art evidence mode".into()),
    };
    if session.identity.authority != authority
        || session.identity.generation == 0
        || session.identity.manifest_sha256.len() != 64
        || !session
            .identity
            .manifest_sha256
            .bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        || session.locator
            != format!(
                "manifests/building-{}-v1.buildingset",
                session.identity.kind.slug()
            )
        || session.nonce.len() != 32
        || !session.nonce.bytes().all(|c| c.is_ascii_hexdigit())
        || !session.status_path.is_absolute()
        || session
            .status_path
            .components()
            .any(|c| matches!(c, Component::ParentDir))
    {
        return Err("building-art session identity/path differs".into());
    }
    bridge_probe::configure(app, &session)?;
    m2_probe::configure(app, &session)?;
    let mut pool = BuildingAssetPool::default();
    pool.allow_acceptance_identity(session.identity.clone());
    app.insert_resource(BuildingAssetLoadPolicy {
        allowed_candidate: Some(session.identity.clone()),
    })
    .insert_resource(pool)
    .insert_resource(session)
    .add_systems(Startup, request);
    Ok(())
}

fn request(
    session: Res<BuildingArtSession>,
    server: Res<AssetServer>,
    mut pool: ResMut<BuildingAssetPool>,
) {
    assert_eq!(
        pool.request(session.identity.clone(), &session.locator, &server),
        BuildingAssetRequest::Started
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiling_presentation_requires_the_exact_opted_in_identity() {
        let identity = BuildingAssetSetIdentity {
            kind: BuildingAssetKind::Tank,
            generation: 1,
            authority: BuildingAssetAuthority::ArtPreview,
            manifest_sha256: "a".repeat(64),
        };
        let mut pool = BuildingAssetPool::default();
        assert!(!pool.presentation_allowed(&identity));
        pool.allow_acceptance_identity(identity.clone());
        assert!(pool.presentation_allowed(&identity));
        for changed in [
            BuildingAssetSetIdentity {
                kind: BuildingAssetKind::MudMixer,
                ..identity.clone()
            },
            BuildingAssetSetIdentity {
                generation: 2,
                ..identity.clone()
            },
            BuildingAssetSetIdentity {
                authority: BuildingAssetAuthority::IsolatedCandidate,
                ..identity.clone()
            },
            BuildingAssetSetIdentity {
                manifest_sha256: "b".repeat(64),
                ..identity.clone()
            },
        ] {
            assert!(!pool.presentation_allowed(&changed));
        }
    }
}
