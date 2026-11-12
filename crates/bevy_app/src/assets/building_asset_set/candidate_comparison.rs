//! Explicit pre-release paused comparison. Never enables ordinary production.
use bevy::prelude::*;
use serde::Deserialize;

use super::{
    BuildingAssetAuthority, BuildingAssetKind, BuildingAssetPool, BuildingAssetRequest,
    BuildingAssetSetIdentity, M6Comparison,
};

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Binding {
    identity: BuildingAssetSetIdentity,
    locator: String,
}

#[derive(Resource)]
struct Bindings(Vec<Binding>);

fn validate(raw: &str, mode: &str) -> Result<(Vec<Binding>, M6Comparison), String> {
    let bindings: Vec<Binding> = serde_json::from_str(raw).map_err(|error| error.to_string())?;
    let expected = [
        BuildingAssetKind::Tank,
        BuildingAssetKind::MudMixer,
        BuildingAssetKind::RestArea,
        BuildingAssetKind::SoulSpa,
        BuildingAssetKind::WheelbarrowParking,
        BuildingAssetKind::SandPile,
        BuildingAssetKind::BonePile,
        BuildingAssetKind::OutdoorLamp,
    ];
    if bindings.len() != expected.len()
        || expected
            .iter()
            .any(|kind| bindings.iter().filter(|b| b.identity.kind == *kind).count() != 1)
        || bindings.iter().any(|b| {
            b.identity.authority != BuildingAssetAuthority::IsolatedCandidate
                || b.identity.generation == 0
                || b.identity.manifest_sha256.len() != 64
                || !b
                    .identity
                    .manifest_sha256
                    .bytes()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
                || b.locator
                    != format!(
                        "manifests/building-{}-v1.buildingset",
                        b.identity.kind.slug()
                    )
        })
    {
        return Err(
            "pre-release comparison requires exact eight canonical isolated candidates".into(),
        );
    }
    let candidate = match mode {
        "candidate" => true,
        "legacy-control" => false,
        _ => return Err("invalid pre-release comparison mode".into()),
    };
    let comparison = M6Comparison {
        candidate,
        pre_release: true,
        group: None,
        identities: bindings.iter().map(|b| b.identity.clone()).collect(),
    };
    Ok((bindings, comparison))
}

fn separate_workload(enabled: bool, static_workload: bool, present: impl Fn(&str) -> bool) -> bool {
    enabled
        && static_workload
        && ![
            "HW_BUILDING_ASSET_RELEASES",
            "HW_BUILDING_GROUP_COMPARISON",
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
    use crate::plugins::startup::{PerfScenarioConfig, PerfWorkload};
    let workload = PerfScenarioConfig::try_from_process().map_err(|error| error.to_string())?;
    if !separate_workload(
        workload.enabled(),
        workload.workload() == PerfWorkload::BuildingArtStatic,
        |key| std::env::var_os(key).is_some(),
    ) {
        return Err("pre-release comparison requires a separate static profiling workload".into());
    }
    let raw = std::env::var("HW_BUILDING_M6_CANDIDATES").map_err(|e| e.to_string())?;
    let mode = std::env::var("HW_BUILDING_M6_MODE").map_err(|e| e.to_string())?;
    let (bindings, comparison) = validate(&raw, &mode)?;
    if comparison.candidate {
        app.init_resource::<BuildingAssetPool>();
        app.world_mut()
            .resource_mut::<BuildingAssetPool>()
            .allow_comparison_identities(comparison.identities.clone());
        app.insert_resource(Bindings(bindings))
            .add_systems(Startup, request);
    }
    app.insert_resource(comparison);
    Ok(())
}

fn request(bindings: Res<Bindings>, server: Res<AssetServer>, mut pool: ResMut<BuildingAssetPool>) {
    for binding in &bindings.0 {
        let result = pool.request(binding.identity.clone(), &binding.locator, &server);
        if result != BuildingAssetRequest::Started {
            error!(?result, "candidate comparison request was not admitted");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_startup_other_workloads_and_conflicting_sessions_are_rejected() {
        assert!(separate_workload(true, true, |_| false));
        assert!(!separate_workload(false, true, |_| false));
        assert!(!separate_workload(true, false, |_| false));
        for conflict in [
            "HW_BUILDING_ASSET_RELEASES",
            "HW_BUILDING_GROUP_COMPARISON",
            "HW_BUILDING_ART_SESSION",
            "HW_BUILDING_ORDINARY_ART_SESSION",
            "HW_BUILDING_PRODUCTION_OBSERVATIONS",
            "HW_M2_ACCEPTANCE_PROBE",
            "HW_BRIDGE_ACCEPTANCE_PROBE",
            "HW_NATIVE_UI_ROOT",
        ] {
            assert!(!separate_workload(true, true, |key| key == conflict));
        }
    }

    fn fixture() -> serde_json::Value {
        serde_json::json!([
            "Tank",
            "MudMixer",
            "RestArea",
            "SoulSpa",
            "WheelbarrowParking",
            "SandPile",
            "BonePile",
            "OutdoorLamp"
        ])
        .as_array()
        .unwrap()
        .iter()
        .map(|kind| {
            let kind: BuildingAssetKind = serde_json::from_value(kind.clone()).unwrap();
            serde_json::json!({"identity": {"kind": kind, "generation": 1,
                    "authority": "isolated_candidate", "manifest_sha256": "a".repeat(64)},
                    "locator": format!("manifests/building-{}-v1.buildingset", kind.slug())})
        })
        .collect::<Vec<_>>()
        .into()
    }

    #[test]
    fn exact_comparison_candidates_never_grant_control_presentation() {
        let raw = fixture().to_string();
        let (_, candidate) = validate(&raw, "candidate").unwrap();
        let (_, control) = validate(&raw, "legacy-control").unwrap();
        assert_eq!(candidate.identities, control.identities);
        assert!(candidate.pre_release && control.pre_release);
        assert!(candidate.candidate && !control.candidate);
        let mut pool = BuildingAssetPool::default();
        assert!(
            candidate
                .identities
                .iter()
                .all(|id| !pool.presentation_allowed(id))
        );
        pool.allow_comparison_identities(candidate.identities.clone());
        assert!(
            candidate
                .identities
                .iter()
                .all(|id| pool.presentation_allowed(id))
        );
        let mut foreign = candidate.identities[0].clone();
        foreign.generation += 1;
        assert!(!pool.presentation_allowed(&foreign));
    }

    #[test]
    fn missing_duplicate_bridge_authority_locator_and_identity_drift_are_rejected() {
        let valid = fixture();
        for authority in ["art_preview", "release_approved"] {
            let mut bad = valid.clone();
            bad[0]["identity"]["authority"] = authority.into();
            assert!(validate(&bad.to_string(), "candidate").is_err());
        }
        for (field, value) in [
            ("kind", serde_json::json!("Bridge")),
            ("generation", serde_json::json!(0)),
            ("manifest_sha256", serde_json::json!("A".repeat(64))),
        ] {
            let mut bad = valid.clone();
            bad[0]["identity"][field] = value;
            assert!(validate(&bad.to_string(), "legacy-control").is_err());
        }
        let mut bad = valid.clone();
        bad.as_array_mut().unwrap().pop();
        assert!(validate(&bad.to_string(), "candidate").is_err());
        let mut bad = valid.clone();
        bad[0] = bad[1].clone();
        assert!(validate(&bad.to_string(), "candidate").is_err());
        let mut bad = valid.clone();
        bad[0]["locator"] = "../candidate.buildingset".into();
        assert!(validate(&bad.to_string(), "candidate").is_err());
        assert!(validate(&valid.to_string(), "fallback-on-error").is_err());
    }
}
