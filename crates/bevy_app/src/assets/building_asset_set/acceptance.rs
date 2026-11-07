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
    #[serde(default)]
    pub view: BuildingArtView,
    #[serde(default)]
    pub variant: BuildingArtVariant,
    #[serde(default)]
    pub lighting: BuildingArtLighting,
    #[serde(default)]
    pub spa_mask_page: Option<u8>,
}

/// Separate seeded comparison; never an ordinary-world or release recipe.
#[derive(Clone, Copy, Debug, Default, Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum BuildingArtVariant {
    #[default]
    Gallery,
    RestAreaArtContext,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum BuildingArtLighting {
    #[default]
    Standard,
    Dim,
}

impl BuildingArtSession {
    pub fn is_rest_context(&self) -> bool {
        self.variant == BuildingArtVariant::RestAreaArtContext
    }

    fn validate_context(&self, present: impl Fn(&str) -> bool) -> Result<(), String> {
        if let Some(page) = self.spa_mask_page {
            if page > 3
                || self.identity.kind != BuildingAssetKind::SoulSpa
                || self.identity.authority != BuildingAssetAuthority::IsolatedCandidate
                || self.mode != "candidate"
                || self.variant != BuildingArtVariant::Gallery
                || self.lighting != BuildingArtLighting::Standard
                || self.view == BuildingArtView::Overview
                || [
                    "HW_BUILDING_M6_MODE",
                    "HW_BUILDING_ORDINARY_ART_SESSION",
                    "HW_BUILDING_PRODUCTION_OBSERVATIONS",
                    "HW_BUILDING_ASSET_RELEASES",
                    "HW_M2_ACCEPTANCE_PROBE",
                    "HW_BRIDGE_ACCEPTANCE_PROBE",
                    "HW_NATIVE_UI_ROOT",
                ]
                .into_iter()
                .any(present)
            {
                return Err(
                    "Spa mask page requires an isolated candidate-only static gallery".into(),
                );
            }
            return Ok(());
        }
        if !self.is_rest_context() {
            return if self.lighting == BuildingArtLighting::Standard {
                Ok(())
            } else {
                Err("non-default lighting requires rest-area-art-context".into())
            };
        }
        if self.identity.kind != BuildingAssetKind::RestArea
            || self.view == BuildingArtView::Overview
            || self.identity.authority != BuildingAssetAuthority::ArtPreview
            || !matches!(self.mode.as_str(), "feedback" | "art-preview")
            || [
                "HW_BUILDING_M6_MODE",
                "HW_BUILDING_ORDINARY_ART_SESSION",
                "HW_BUILDING_PRODUCTION_OBSERVATIONS",
                "HW_BUILDING_ASSET_RELEASES",
                "HW_M2_ACCEPTANCE_PROBE",
                "HW_BRIDGE_ACCEPTANCE_PROBE",
                "HW_NATIVE_UI_ROOT",
            ]
            .into_iter()
            .any(present)
        {
            return Err(
                "rest-area-art-context kind/authority or conflicting session differs".into(),
            );
        }
        Ok(())
    }
}

/// Explicit profiling camera framing; it never changes ordinary player input.
#[derive(Clone, Copy, Debug, Default, Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum BuildingArtView {
    #[default]
    Overview,
    Near,
    Normal,
    Far,
}

impl BuildingArtView {
    pub fn scale(self) -> f32 {
        match self {
            Self::Overview => 5.0,
            Self::Near => 0.5,
            Self::Normal => 1.0,
            Self::Far => 2.0,
        }
    }
}

pub fn configure_building_art(app: &mut App) -> Result<(), String> {
    let Ok(value) = std::env::var("HW_BUILDING_ART_SESSION") else {
        return Ok(());
    };
    let session: BuildingArtSession = serde_json::from_str(&value).map_err(|e| e.to_string())?;
    session.validate_context(|key| std::env::var_os(key).is_some())?;
    let authority = match session.mode.as_str() {
        "feedback" | "art-preview" => BuildingAssetAuthority::ArtPreview,
        "candidate" => BuildingAssetAuthority::IsolatedCandidate,
        _ => return Err("unknown building-art evidence mode".into()),
    };
    if session.identity.authority != authority
        || session.identity.kind == BuildingAssetKind::Bridge
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
    fn spa_pages_are_candidate_only_bounded_and_cannot_mix_sessions() {
        let raw = serde_json::json!({"mode":"candidate", "identity":{"kind":"SoulSpa", "generation":2,
            "authority":"isolated_candidate", "manifest_sha256":"a".repeat(64)},
            "locator":"manifests/building-soul-spa-v1.buildingset", "status_path":"/unused/spa-page.json",
            "nonce":"a".repeat(32), "view":"near", "spa_mask_page":0});
        for page in 0..4 {
            let mut value = raw.clone();
            value["spa_mask_page"] = serde_json::json!(page);
            let session: BuildingArtSession = serde_json::from_value(value).unwrap();
            assert!(session.validate_context(|_| false).is_ok());
            for key in [
                "HW_BUILDING_M6_MODE",
                "HW_BUILDING_ORDINARY_ART_SESSION",
                "HW_NATIVE_UI_ROOT",
                "HW_BUILDING_PRODUCTION_OBSERVATIONS",
                "HW_BUILDING_ASSET_RELEASES",
                "HW_M2_ACCEPTANCE_PROBE",
                "HW_BRIDGE_ACCEPTANCE_PROBE",
            ] {
                assert!(session.validate_context(|name| name == key).is_err());
            }
        }
        for page in [
            serde_json::json!(true),
            serde_json::json!(1.0),
            serde_json::json!(-1),
            serde_json::json!(4),
        ] {
            let mut value = raw.clone();
            value["spa_mask_page"] = page;
            assert!(match serde_json::from_value::<BuildingArtSession>(value) {
                Ok(session) => session.validate_context(|_| false).is_err(),
                Err(_) => true,
            });
        }
        for (key, value) in [
            ("mode", "feedback"),
            ("variant", "rest-area-art-context"),
            ("lighting", "dim"),
            ("view", "overview"),
        ] {
            let mut changed = raw.clone();
            changed[key] = serde_json::json!(value);
            assert!(
                serde_json::from_value::<BuildingArtSession>(changed)
                    .unwrap()
                    .validate_context(|_| false)
                    .is_err()
            );
        }
        for (key, value) in [("kind", "Tank"), ("authority", "art_preview")] {
            let mut changed = raw.clone();
            changed["identity"][key] = serde_json::json!(value);
            assert!(
                serde_json::from_value::<BuildingArtSession>(changed)
                    .unwrap()
                    .validate_context(|_| false)
                    .is_err()
            );
        }
    }

    #[test]
    fn rest_context_is_explicit_preview_only_and_rejects_mixed_sessions() {
        let raw = serde_json::json!({"mode": "feedback", "identity": {
            "kind": "RestArea", "generation": 1, "authority": "art_preview", "manifest_sha256": "a".repeat(64)},
            "locator": "manifests/building-rest-area-v1.buildingset", "status_path": "/unused/probe.json",
            "nonce": "a".repeat(32), "view": "normal", "variant": "rest-area-art-context", "lighting": "dim"});
        let session: BuildingArtSession = serde_json::from_value(raw.clone()).unwrap();
        assert!(session.validate_context(|_| false).is_ok());
        for key in [
            "HW_BUILDING_M6_MODE",
            "HW_BUILDING_ORDINARY_ART_SESSION",
            "HW_NATIVE_UI_ROOT",
            "HW_BUILDING_PRODUCTION_OBSERVATIONS",
            "HW_BUILDING_ASSET_RELEASES",
            "HW_M2_ACCEPTANCE_PROBE",
            "HW_BRIDGE_ACCEPTANCE_PROBE",
        ] {
            assert!(
                session
                    .validate_context(|candidate| candidate == key)
                    .is_err()
            );
        }
        for (key, value) in [("variant", "unknown"), ("lighting", "unknown")] {
            let mut bad = raw.clone();
            bad[key] = serde_json::json!(value);
            assert!(serde_json::from_value::<BuildingArtSession>(bad).is_err());
        }
        for (key, value) in [
            ("mode", "candidate"),
            ("view", "overview"),
            ("variant", "gallery"),
        ] {
            let mut bad = raw.clone();
            bad[key] = serde_json::json!(value);
            assert!(
                serde_json::from_value::<BuildingArtSession>(bad)
                    .unwrap()
                    .validate_context(|_| false)
                    .is_err()
            );
        }
        for (key, value) in [("kind", "Tank"), ("authority", "isolated_candidate")] {
            let mut bad = raw.clone();
            bad["identity"][key] = serde_json::json!(value);
            assert!(
                serde_json::from_value::<BuildingArtSession>(bad)
                    .unwrap()
                    .validate_context(|_| false)
                    .is_err()
            );
        }
        let mut legacy = raw;
        legacy.as_object_mut().unwrap().remove("variant");
        legacy.as_object_mut().unwrap().remove("lighting");
        let session: BuildingArtSession = serde_json::from_value(legacy).unwrap();
        assert_eq!(session.variant, BuildingArtVariant::Gallery);
        assert_eq!(session.lighting, BuildingArtLighting::Standard);
        assert!(session.validate_context(|_| false).is_ok());
    }

    #[test]
    fn camera_views_are_bounded_and_unknown_views_fail_closed() {
        for (name, scale) in [
            ("overview", 5.0),
            ("near", 0.5),
            ("normal", 1.0),
            ("far", 2.0),
        ] {
            let view: BuildingArtView = serde_json::from_str(&format!("\"{name}\"")).unwrap();
            assert_eq!(view.scale(), scale);
        }
        assert!(serde_json::from_str::<BuildingArtView>("\"arbitrary\"").is_err());
        assert_eq!(BuildingArtView::default(), BuildingArtView::Overview);
    }

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
