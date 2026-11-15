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
mod m2_ui;

/// Read-only execution-route check. Full identity/path validation still runs
/// before any building load request in `configure_building_art`.
pub fn m2_ui_pair_admitted_from_process() -> bool {
    m2_ui::pair_admitted(|key| {
        std::env::var_os(key).map(|value| value.to_string_lossy().into_owned())
    })
}

pub(crate) fn m2_ui_case(case: &str) -> bool {
    m2_ui::case_kind(case).is_some()
}

pub(crate) fn m2_ui_snapshot(world: &mut World) -> serde_json::Value {
    m2_ui::snapshot(world)
}

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
    #[serde(default)]
    pub lamp_supply_target: Option<u8>,
    #[serde(default)]
    pub window_contract: Option<String>,
    #[serde(default)]
    pub scale_factor: Option<f32>,
}

/// Separate seeded comparison; never an ordinary-world or release recipe.
#[derive(Clone, Copy, Debug, Default, Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum BuildingArtVariant {
    #[default]
    Gallery,
    RestAreaArtContext,
    M2ArtContext,
    M5ArtContext,
    LampSupplyArtContext,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum BuildingArtLighting {
    #[default]
    Standard,
    Dim,
}

impl BuildingArtSession {
    pub(crate) fn window_matches(&self, requested: Option<f32>) -> bool {
        match (&self.window_contract, self.scale_factor) {
            (None, None) => requested == Some(1.0),
            (Some(contract), Some(scale)) => {
                contract == "building-art-window-v1"
                    && matches!(scale, 1.0 | 1.5 | 2.0)
                    && requested == Some(scale)
                    && (scale == 1.0
                        || (self.variant == BuildingArtVariant::Gallery
                            && self.spa_mask_page.is_none()
                            && self.lamp_supply_target.is_none()))
            }
            _ => false,
        }
    }

    pub fn is_rest_context(&self) -> bool {
        self.variant == BuildingArtVariant::RestAreaArtContext
    }

    pub fn is_m2_context(&self) -> bool {
        self.variant == BuildingArtVariant::M2ArtContext
    }

    pub fn is_m5_context(&self) -> bool {
        matches!(
            self.variant,
            BuildingArtVariant::M5ArtContext | BuildingArtVariant::LampSupplyArtContext
        )
    }

    pub fn is_lamp_supply_context(&self) -> bool {
        self.variant == BuildingArtVariant::LampSupplyArtContext
    }

    pub fn is_context(&self) -> bool {
        self.is_rest_context() || self.is_m2_context() || self.is_m5_context()
    }

    pub(crate) fn validate_context(&self, present: impl Fn(&str) -> bool) -> Result<(), String> {
        if !self.window_matches(self.scale_factor.or(Some(1.0))) {
            return Err("building-art window contract/scale factor differs".into());
        }
        if self.lamp_supply_target.is_some() && !self.is_lamp_supply_context() {
            return Err("Lamp UI target requires its separate rendering-only context".into());
        }
        if self.is_m2_context() {
            if !matches!(
                self.identity.kind,
                BuildingAssetKind::Tank | BuildingAssetKind::MudMixer
            ) || self.mode != "feedback"
                || self.identity.authority != BuildingAssetAuthority::ArtPreview
                || !matches!(self.view, BuildingArtView::Normal | BuildingArtView::Far)
                || self.spa_mask_page.is_some()
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
                    "M2 art context requires separate Tank/Mixer paused feedback ArtPreview".into(),
                );
            }
            return Ok(());
        }
        if self.is_m5_context() {
            let lamp_ui = self.is_lamp_supply_context();
            if !matches!(
                self.identity.kind,
                BuildingAssetKind::WheelbarrowParking
                    | BuildingAssetKind::SandPile
                    | BuildingAssetKind::BonePile
                    | BuildingAssetKind::OutdoorLamp
            ) || self.identity.authority != BuildingAssetAuthority::ArtPreview
                || if lamp_ui {
                    self.identity.kind != BuildingAssetKind::OutdoorLamp
                        || self.mode != "feedback"
                        || !matches!(self.lamp_supply_target, Some(0 | 2))
                        || !matches!(
                            self.view,
                            BuildingArtView::Normal | BuildingArtView::Overview
                        )
                } else {
                    !matches!(self.mode.as_str(), "feedback" | "art-preview")
                        || self.view != BuildingArtView::Normal
                }
                || self.lighting != BuildingArtLighting::Standard
                || self.spa_mask_page.is_some()
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
                    "M5 context requires a separate normal-view ArtPreview static comparison"
                        .into(),
                );
            }
            return Ok(());
        }
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
    if session.window_contract.is_some() || session.scale_factor.is_some() {
        use crate::plugins::startup::{PerfScenarioConfig, PerfWorkload};
        let config = PerfScenarioConfig::try_from_process().map_err(|e| e.to_string())?;
        if !config.enabled()
            || config.workload() != PerfWorkload::BuildingArtStatic
            || !session.window_matches(config.requested_window_scale_factor())
        {
            return Err("building-art window contract requires the matching static fixture".into());
        }
    }
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
    fn m2_context_admits_only_separate_paused_preview_comparisons() {
        let raw = serde_json::json!({"mode": "feedback", "identity": {
            "kind": "Tank", "generation": 1, "authority": "art_preview", "manifest_sha256": "a".repeat(64)},
            "locator": "manifests/building-tank-v1.buildingset", "status_path": "/unused/probe.json",
            "nonce": "a".repeat(32), "variant": "m2-art-context", "view": "normal", "lighting": "dim"});
        for kind in ["Tank", "MudMixer"] {
            let mut value = raw.clone();
            value["identity"]["kind"] = serde_json::json!(kind);
            let session: BuildingArtSession = serde_json::from_value(value).unwrap();
            assert!(session.is_m2_context() && session.is_context() && !session.is_rest_context());
            assert!(session.validate_context(|_| false).is_ok());
            for conflict in [
                "HW_M2_ACCEPTANCE_PROBE",
                "HW_BUILDING_ASSET_RELEASES",
                "HW_BUILDING_M6_MODE",
                "HW_NATIVE_UI_ROOT",
            ] {
                assert!(session.validate_context(|key| key == conflict).is_err());
            }
        }
        for (key, value) in [
            ("mode", serde_json::json!("candidate")),
            ("view", serde_json::json!("overview")),
            ("view", serde_json::json!("near")),
            ("spa_mask_page", serde_json::json!(0)),
            ("lamp_supply_target", serde_json::json!(0)),
        ] {
            let mut changed = raw.clone();
            changed[key] = value;
            assert!(
                serde_json::from_value::<BuildingArtSession>(changed)
                    .unwrap()
                    .validate_context(|_| false)
                    .is_err()
            );
        }
        for (key, value) in [
            ("kind", "RestArea"),
            ("kind", "Bridge"),
            ("authority", "isolated_candidate"),
        ] {
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
    fn lamp_supply_context_is_rendering_only_and_rejects_foreign_targets() {
        let raw = serde_json::json!({"mode": "feedback", "identity": {
            "kind": "OutdoorLamp", "generation": 1, "authority": "art_preview", "manifest_sha256": "a".repeat(64)},
            "locator": "manifests/building-outdoor-lamp-v1.buildingset", "status_path": "/unused/probe.json",
            "nonce": "a".repeat(32), "view": "normal", "variant": "lamp-supply-art-context",
            "lighting": "standard", "spa_mask_page": null, "lamp_supply_target": 0});
        for view in ["normal", "overview"] {
            for ordinal in [0, 2] {
                let mut good = raw.clone();
                good["view"] = serde_json::json!(view);
                good["lamp_supply_target"] = serde_json::json!(ordinal);
                let session: BuildingArtSession = serde_json::from_value(good).unwrap();
                assert!(session.validate_context(|_| false).is_ok());
                for mixed in [
                    "HW_NATIVE_UI_ROOT",
                    "HW_BUILDING_ORDINARY_ART_SESSION",
                    "HW_BUILDING_PRODUCTION_OBSERVATIONS",
                    "HW_BUILDING_M6_MODE",
                    "HW_BUILDING_ASSET_RELEASES",
                    "HW_M2_ACCEPTANCE_PROBE",
                    "HW_BRIDGE_ACCEPTANCE_PROBE",
                ] {
                    assert!(session.validate_context(|key| key == mixed).is_err());
                }
            }
        }
        for (pointer, value) in [
            ("/mode", serde_json::json!("art-preview")),
            ("/identity/kind", serde_json::json!("SandPile")),
            (
                "/identity/authority",
                serde_json::json!("isolated_candidate"),
            ),
            ("/view", serde_json::json!("near")),
            ("/view", serde_json::json!("far")),
            ("/variant", serde_json::json!("m5-art-context")),
            ("/lighting", serde_json::json!("dim")),
            ("/spa_mask_page", serde_json::json!(0)),
            ("/lamp_supply_target", serde_json::json!(null)),
            ("/lamp_supply_target", serde_json::json!(1)),
            ("/lamp_supply_target", serde_json::json!(3)),
        ] {
            let mut bad = raw.clone();
            *bad.pointer_mut(pointer).unwrap() = value;
            assert!(
                serde_json::from_value::<BuildingArtSession>(bad)
                    .unwrap()
                    .validate_context(|_| false)
                    .is_err()
            );
        }
    }

    #[test]
    fn m5_context_is_separate_preview_only_normal_view_and_rejects_mixing() {
        let raw = serde_json::json!({"mode": "feedback", "identity": {
            "kind": "OutdoorLamp", "generation": 1, "authority": "art_preview", "manifest_sha256": "a".repeat(64)},
            "locator": "manifests/building-outdoor-lamp-v1.buildingset", "status_path": "/unused/probe.json",
            "nonce": "a".repeat(32), "view": "normal", "variant": "m5-art-context", "lighting": "standard", "spa_mask_page": null});
        for kind in ["WheelbarrowParking", "SandPile", "BonePile", "OutdoorLamp"] {
            let mut good = raw.clone();
            good["identity"]["kind"] = serde_json::json!(kind);
            let session: BuildingArtSession = serde_json::from_value(good).unwrap();
            assert!(session.is_context() && session.is_m5_context() && !session.is_rest_context());
            assert!(session.validate_context(|_| false).is_ok());
        }
        let session: BuildingArtSession = serde_json::from_value(raw.clone()).unwrap();
        for key in [
            "HW_BUILDING_M6_MODE",
            "HW_BUILDING_ORDINARY_ART_SESSION",
            "HW_BUILDING_PRODUCTION_OBSERVATIONS",
            "HW_BUILDING_ASSET_RELEASES",
            "HW_M2_ACCEPTANCE_PROBE",
            "HW_BRIDGE_ACCEPTANCE_PROBE",
            "HW_NATIVE_UI_ROOT",
        ] {
            assert!(
                session
                    .validate_context(|candidate| candidate == key)
                    .is_err()
            );
        }
        for (path, value) in [
            ("/mode", serde_json::json!("candidate")),
            ("/identity/kind", serde_json::json!("Bridge")),
            ("/identity/kind", serde_json::json!("RestArea")),
            (
                "/identity/authority",
                serde_json::json!("isolated_candidate"),
            ),
            ("/view", serde_json::json!("near")),
            ("/view", serde_json::json!("overview")),
            ("/lighting", serde_json::json!("dim")),
            ("/spa_mask_page", serde_json::json!(0)),
        ] {
            let mut bad = raw.clone();
            *bad.pointer_mut(path)
                .unwrap_or_else(|| panic!("missing test key: {path}")) = value;
            assert!(
                serde_json::from_value::<BuildingArtSession>(bad)
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
