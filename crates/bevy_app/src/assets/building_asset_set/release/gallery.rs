//! Explicit released-only nine-kind storyboard. Never admits a candidate.
use super::*;
use crate::assets::door_asset_set::{DoorAssetAuthority, DoorAssetSetIdentity};
use crate::plugins::startup::{PerfScenarioConfig, PerfScenarioSize, PerfWorkload};
use hw_core::quality::RttQualityPreset;
use std::path::PathBuf;

use super::super::{
    acceptance::BuildingArtView,
    gallery_capture::GalleryCaptureSession,
    group_art::{GROUP_ART_HEIGHT, GROUP_ART_WIDTH},
};

pub(crate) const RELEASE_GALLERY_ENV: &str = "HW_BUILDING_RELEASE_GALLERY_SESSION";
pub(crate) const RELEASE_GALLERY_CONTRACT: &str = "building-release-art-gallery-v1";
const DOOR_MANIFEST: &str = "4e3f9236db067772b9a60b37ea98437cb29e481f9ec9cbb4d6dcf6517374a449";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReleasedDoorIdentity {
    asset_set_generation: u64,
    authority: DoorAssetAuthority,
    manifest_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReleaseArtSession {
    pub schema_version: u8,
    pub contract_id: String,
    pub(super) bindings: Vec<ReleaseBinding>,
    pub door_identity: ReleasedDoorIdentity,
    pub nonce: String,
    pub status_path: PathBuf,
    pub view: BuildingArtView,
    pub quality: String,
    pub scale_factor: f32,
}

impl ReleaseArtSession {
    pub(crate) fn parse(raw: &str) -> Result<Self, String> {
        let value: Self = serde_json::from_str(raw).map_err(|error| error.to_string())?;
        if value.schema_version != 1
            || value.contract_id != RELEASE_GALLERY_CONTRACT
            || !matches!(value.view, BuildingArtView::Normal | BuildingArtView::Far)
            || value.quality != "high"
            || value.scale_factor != 1.0
            || value.nonce.len() != 32
            || !value
                .nonce
                .bytes()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
            || !value.status_path.is_absolute()
            || value
                .status_path
                .file_name()
                .is_none_or(|name| name != "probe.json")
            || value
                .status_path
                .components()
                .any(|part| !matches!(part, Component::RootDir | Component::Normal(_)))
            || value.door_identity.asset_set_generation != 7
            || value.door_identity.authority != DoorAssetAuthority::ReleaseApproved
            || value.door_identity.manifest_sha256 != DOOR_MANIFEST
        {
            return Err(
                "released gallery requires its exact full-eight/Door-g7 image contract".into(),
            );
        }
        // Existing production guard, including full eight-kind and canonical
        // locator checks. No comparison/candidate allowance is installed.
        comparison("candidate", &value.bindings)?;
        Ok(value)
    }

    pub(crate) fn check_door(&self, identity: &DoorAssetSetIdentity) -> Result<(), String> {
        if self.door_identity.asset_set_generation == identity.asset_set_generation
            && self.door_identity.authority == identity.authority
            && self.door_identity.manifest_sha256 == identity.manifest_sha256
        {
            Ok(())
        } else {
            Err("released gallery resolved another Door generation".into())
        }
    }

    pub(crate) fn target(&self, kind: BuildingAssetKind) -> bool {
        self.bindings
            .iter()
            .any(|binding| binding.identity.kind == kind)
    }

    pub(crate) fn window_matches(
        &self,
        width: Option<u32>,
        height: Option<u32>,
        scale: Option<f32>,
        quality: Option<RttQualityPreset>,
    ) -> bool {
        width == Some(GROUP_ART_WIDTH)
            && height == Some(GROUP_ART_HEIGHT)
            && scale == Some(1.0)
            && quality == Some(RttQualityPreset::High)
    }
}

fn separate(present: impl Fn(&str) -> bool) -> bool {
    ![
        "HW_BUILDING_GROUP_ART_SESSION",
        "HW_BUILDING_GROUP_COMPARISON",
        "HW_BUILDING_M6_MODE",
        "HW_BUILDING_M6_CANDIDATES",
        "HW_BUILDING_ASSET_RELEASES",
        "HW_BUILDING_ART_SESSION",
        "HW_BUILDING_ORDINARY_ART_SESSION",
        "HW_BUILDING_PRODUCTION_OBSERVATIONS",
        "HW_M2_ACCEPTANCE_PROBE",
        "HW_BRIDGE_ACCEPTANCE_PROBE",
        "HW_NATIVE_UI_ROOT",
    ]
    .into_iter()
    .any(present)
}

pub(super) fn configure(app: &mut App) -> Result<(), String> {
    let session = ReleaseArtSession::parse(
        &std::env::var(RELEASE_GALLERY_ENV).map_err(|error| error.to_string())?,
    )?;
    let config = PerfScenarioConfig::try_from_process().map_err(|error| error.to_string())?;
    let directory = session
        .status_path
        .parent()
        .ok_or("missing released gallery root")?;
    let native_root = std::env::current_dir()
        .map_err(|error| error.to_string())?
        .join("target/native-acceptance");
    if !separate(|key| std::env::var_os(key).is_some())
        || !config.enabled()
        || config.workload() != PerfWorkload::BuildingArtStatic
        || config.size() != PerfScenarioSize::Small
        || config.output_dir() != Some(directory.join("data").as_path())
        || !session.window_matches(
            config.requested_window_size().map(|size| size.0),
            config.requested_window_size().map(|size| size.1),
            config.requested_window_scale_factor(),
            config.requested_rtt_quality(),
        )
        || !directory.starts_with(native_root)
        || directory
            .canonicalize()
            .map_err(|error| error.to_string())?
            != directory
        || session.status_path.exists()
    {
        return Err("released gallery requires its separate exact native session and root".into());
    }
    let admitted = comparison("candidate", &session.bindings)?;
    app.init_resource::<BuildingAssetPool>()
        .insert_resource(admitted)
        .insert_resource(BuildingAssetReleaseBindings(session.bindings.clone()))
        .insert_resource(GalleryCaptureSession::Released(session))
        .add_systems(Startup, request_releases);
    Ok(())
}

#[cfg(test)]
pub(crate) fn test_session() -> ReleaseArtSession {
    let kinds = [
        BuildingAssetKind::Tank,
        BuildingAssetKind::MudMixer,
        BuildingAssetKind::RestArea,
        BuildingAssetKind::SoulSpa,
        BuildingAssetKind::WheelbarrowParking,
        BuildingAssetKind::SandPile,
        BuildingAssetKind::BonePile,
        BuildingAssetKind::OutdoorLamp,
    ];
    ReleaseArtSession {
        schema_version: 1,
        contract_id: RELEASE_GALLERY_CONTRACT.into(),
        bindings: kinds
            .into_iter()
            .map(|kind| ReleaseBinding {
                identity: BuildingAssetSetIdentity {
                    kind,
                    generation: 1,
                    authority: BuildingAssetAuthority::ReleaseApproved,
                    manifest_sha256: "a".repeat(64),
                },
                locator: canonical_locator(kind),
            })
            .collect(),
        door_identity: ReleasedDoorIdentity {
            asset_set_generation: 7,
            authority: DoorAssetAuthority::ReleaseApproved,
            manifest_sha256: DOOR_MANIFEST.into(),
        },
        nonce: "b".repeat(32),
        status_path: "/unused/probe.json".into(),
        view: BuildingArtView::Normal,
        quality: "high".into(),
        scale_factor: 1.,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn released_gallery_admits_only_full_eight_and_normal_door_without_candidate_policy() {
        let original = serde_json::to_value(test_session()).unwrap();
        assert!(ReleaseArtSession::parse(&original.to_string()).is_ok());
        for (field, value) in [
            ("schema_version", json!(true)),
            ("schema_version", json!(2)),
            ("view", json!("overview")),
            ("view", json!("near")),
            ("quality", json!("low")),
            ("scale_factor", json!(2)),
            ("scale_factor", json!(true)),
            ("nonce", json!("B".repeat(32))),
            ("status_path", json!("/tmp/../probe.json")),
            ("contract_id", json!("building-group-art-gallery-v1")),
            ("accepted", json!(true)),
        ] {
            let mut bad = original.clone();
            bad[field] = value;
            assert!(
                ReleaseArtSession::parse(&bad.to_string()).is_err(),
                "{field}"
            );
        }
        for authority in ["art_preview", "isolated_candidate"] {
            let mut bad = original.clone();
            bad["bindings"][0]["identity"]["authority"] = json!(authority);
            assert!(ReleaseArtSession::parse(&bad.to_string()).is_err());
        }
        for length in [0, 2, 4, 7] {
            let mut bad = original.clone();
            bad["bindings"].as_array_mut().unwrap().truncate(length);
            assert!(ReleaseArtSession::parse(&bad.to_string()).is_err());
        }
        for (field, value) in [
            ("asset_set_generation", json!(6)),
            ("authority", json!("art_preview")),
            ("manifest_sha256", json!("f".repeat(64))),
        ] {
            let mut bad = original.clone();
            bad["door_identity"][field] = value;
            assert!(ReleaseArtSession::parse(&bad.to_string()).is_err());
        }
        for (field, value) in [
            ("kind", json!("Bridge")),
            ("manifest_sha256", json!("A".repeat(64))),
        ] {
            let mut bad = original.clone();
            bad["bindings"][0]["identity"][field] = value;
            assert!(ReleaseArtSession::parse(&bad.to_string()).is_err());
        }
        let mut bad = original.clone();
        bad["bindings"][0]["locator"] = json!("../foreign");
        assert!(ReleaseArtSession::parse(&bad.to_string()).is_err());
        let mut bad = original.clone();
        bad["bindings"][1] = bad["bindings"][0].clone();
        assert!(ReleaseArtSession::parse(&bad.to_string()).is_err());
    }

    #[test]
    fn released_gallery_window_and_session_are_separate_from_measurement_and_candidates() {
        let session = test_session();
        assert!(session.window_matches(
            Some(1920),
            Some(1680),
            Some(1.),
            Some(RttQualityPreset::High)
        ));
        for (width, height, dpi, quality) in [
            (1280, 720, 1., RttQualityPreset::High),
            (1920, 1680, 2., RttQualityPreset::High),
            (1920, 1680, 1., RttQualityPreset::Low),
        ] {
            assert!(!session.window_matches(Some(width), Some(height), Some(dpi), Some(quality)));
        }
        assert!(separate(|_| false));
        for key in [
            "HW_BUILDING_GROUP_ART_SESSION",
            "HW_BUILDING_GROUP_COMPARISON",
            "HW_BUILDING_M6_MODE",
            "HW_BUILDING_M6_CANDIDATES",
            "HW_BUILDING_ASSET_RELEASES",
            "HW_NATIVE_UI_ROOT",
            "HW_BUILDING_ART_SESSION",
        ] {
            assert!(!separate(|value| value == key));
        }
    }
}
