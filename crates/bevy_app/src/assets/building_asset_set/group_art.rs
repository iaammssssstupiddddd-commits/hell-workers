//! A separate, paused group gallery; never a performance or release admission.

use std::path::{Component, PathBuf};

use bevy::prelude::*;
use hw_core::quality::RttQualityPreset;
use serde::{Deserialize, Serialize};

use super::{
    BuildingAssetKind, BuildingAssetPool, M6Comparison,
    acceptance::BuildingArtView,
    group_comparison::{self, Declaration, Group},
};

pub(crate) const GROUP_ART_ENV: &str = "HW_BUILDING_GROUP_ART_SESSION";
pub(crate) const GROUP_ART_CONTRACT: &str = "building-group-art-gallery-v1";
pub(crate) const GROUP_ART_WIDTH: u32 = 1920;
pub(crate) const GROUP_ART_HEIGHT: u32 = 1680;

#[derive(Clone, Debug, Resource, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GroupArtSession {
    pub schema_version: u8,
    pub contract_id: String,
    pub bindings: Declaration,
    pub nonce: String,
    pub status_path: PathBuf,
    pub view: BuildingArtView,
    pub quality: String,
    pub scale_factor: f32,
}

impl GroupArtSession {
    pub(crate) fn parse(raw: &str) -> Result<Self, String> {
        let session: Self = serde_json::from_str(raw).map_err(|e| e.to_string())?;
        if session.schema_version != 1
            || session.contract_id != GROUP_ART_CONTRACT
            || !matches!(session.view, BuildingArtView::Normal | BuildingArtView::Far)
            || !matches!(session.quality.as_str(), "high" | "medium" | "low")
            || !matches!(session.scale_factor, 1.0 | 1.5 | 2.0)
            || session.nonce.len() != 32
            || !session
                .nonce
                .bytes()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
            || !session.status_path.is_absolute()
            || session
                .status_path
                .file_name()
                .is_none_or(|p| p != "probe.json")
            || session
                .status_path
                .components()
                .any(|c| !matches!(c, Component::RootDir | Component::Normal(_)))
        {
            return Err("group gallery requires an exact paused art/window/nonce contract".into());
        }
        session.comparison()?;
        Ok(session)
    }

    pub(crate) fn comparison(&self) -> Result<M6Comparison, String> {
        group_comparison::parse(
            &serde_json::to_string(&self.bindings).map_err(|e| e.to_string())?,
            "candidate",
        )
    }

    pub(crate) fn group(&self) -> Group {
        self.bindings.group
    }

    pub(crate) fn target(&self, kind: BuildingAssetKind) -> bool {
        self.group().targets().contains(&kind)
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
            && scale == Some(self.scale_factor)
            && quality.is_some_and(|q| q.as_str() == self.quality)
    }
}

fn separate(present: impl Fn(&str) -> bool) -> bool {
    ![
        "HW_BUILDING_RELEASE_GALLERY_SESSION",
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
    use crate::plugins::startup::{PerfScenarioConfig, PerfScenarioSize, PerfWorkload};
    let session =
        GroupArtSession::parse(&std::env::var(GROUP_ART_ENV).map_err(|e| e.to_string())?)?;
    let config = PerfScenarioConfig::try_from_process().map_err(|e| e.to_string())?;
    let directory = session.status_path.parent().ok_or("missing gallery root")?;
    let data = directory.join("data");
    let native_root = std::env::current_dir()
        .map_err(|e| e.to_string())?
        .join("target/native-acceptance");
    if !separate(|key| std::env::var_os(key).is_some())
        || !config.enabled()
        || config.workload() != PerfWorkload::BuildingArtStatic
        || config.size() != PerfScenarioSize::Small
        || config.output_dir() != Some(data.as_path())
        || !session.window_matches(
            config.requested_window_size().map(|size| size.0),
            config.requested_window_size().map(|size| size.1),
            config.requested_window_scale_factor(),
            config.requested_rtt_quality(),
        )
        || !directory.starts_with(native_root)
        || directory.canonicalize().map_err(|e| e.to_string())? != directory
        || session.status_path.exists()
    {
        return Err("group gallery requires its separate exact native static session/root".into());
    }
    let comparison = session.comparison()?;
    app.init_resource::<BuildingAssetPool>();
    app.world_mut()
        .resource_mut::<BuildingAssetPool>()
        .allow_comparison_identities(comparison.loaded_identities().cloned().collect());
    app.insert_resource(comparison)
        .insert_resource(session.clone())
        .insert_resource(super::gallery_capture::GalleryCaptureSession::Group(
            session,
        ))
        .add_systems(Startup, group_comparison::request);
    Ok(())
}

#[cfg(test)]
pub(crate) fn test_session() -> GroupArtSession {
    GroupArtSession {
        schema_version: 1,
        contract_id: GROUP_ART_CONTRACT.into(),
        bindings: group_comparison::tests::fixture(Group::M2),
        nonce: "a".repeat(32),
        status_path: "/unused/probe.json".into(),
        view: BuildingArtView::Normal,
        quality: "high".into(),
        scale_factor: 2.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_gallery_contract_never_grants_ordinary_or_performance_scope() {
        let original = serde_json::to_value(test_session()).unwrap();
        assert!(GroupArtSession::parse(&original.to_string()).is_ok());
        for (field, value) in [
            ("view", serde_json::json!("overview")),
            ("view", serde_json::json!("near")),
            ("scale_factor", serde_json::json!(1.25)),
            ("quality", serde_json::json!("ultra")),
            ("nonce", serde_json::json!("A".repeat(32))),
            ("status_path", serde_json::json!("/tmp/../probe.json")),
            ("contract_id", serde_json::json!("building-art")),
        ] {
            let mut bad = original.clone();
            bad[field] = value;
            assert!(GroupArtSession::parse(&bad.to_string()).is_err(), "{field}");
        }
        let session = test_session();
        assert!(session.window_matches(
            Some(1920),
            Some(1680),
            Some(2.0),
            Some(RttQualityPreset::High)
        ));
        assert!(!session.window_matches(
            Some(1280),
            Some(720),
            Some(2.0),
            Some(RttQualityPreset::High)
        ));
        assert!(!session.window_matches(
            Some(1920),
            Some(1680),
            Some(1.0),
            Some(RttQualityPreset::High)
        ));
        assert!(!session.window_matches(
            Some(1920),
            Some(1680),
            Some(2.0),
            Some(RttQualityPreset::Low)
        ));
        assert!(separate(|_| false));
        for key in [
            "HW_BUILDING_ART_SESSION",
            "HW_BUILDING_RELEASE_GALLERY_SESSION",
            "HW_BUILDING_GROUP_COMPARISON",
            "HW_BUILDING_M6_MODE",
            "HW_NATIVE_UI_ROOT",
            "HW_BUILDING_ASSET_RELEASES",
            "HW_M2_ACCEPTANCE_PROBE",
        ] {
            assert!(!separate(|value| value == key));
        }
    }
}
