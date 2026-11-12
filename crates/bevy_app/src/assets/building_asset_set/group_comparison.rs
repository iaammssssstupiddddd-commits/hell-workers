//! Explicit group-only profiling admission. No ordinary startup or release authority.
use std::collections::HashSet;
use std::path::{Component, Path};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::{
    BuildingAssetAuthority, BuildingAssetKind as Kind, BuildingAssetPool, BuildingAssetRequest,
    BuildingAssetSetIdentity, M6Comparison,
};

pub(crate) const GROUP_ENV: &str = "HW_BUILDING_GROUP_COMPARISON";

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Group {
    M2,
    M3,
    M5,
}

impl Group {
    pub(crate) fn targets(self) -> &'static [Kind] {
        match self {
            Self::M2 => &[Kind::Tank, Kind::MudMixer],
            Self::M3 => &[Kind::RestArea, Kind::SoulSpa],
            Self::M5 => &[
                Kind::WheelbarrowParking,
                Kind::SandPile,
                Kind::BonePile,
                Kind::OutdoorLamp,
            ],
        }
    }

    fn fixed(self) -> &'static [Kind] {
        match self {
            Self::M2 => &[],
            Self::M3 => &[Kind::Tank, Kind::MudMixer],
            Self::M5 => &[Kind::Tank, Kind::MudMixer, Kind::RestArea, Kind::SoulSpa],
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Binding {
    identity: BuildingAssetSetIdentity,
    locator: String,
    /// Exact artifact paths, independently checked against the frozen manifests.
    paths: Vec<String>,
}

impl Binding {
    fn valid(&self, authority: BuildingAssetAuthority) -> bool {
        let id = &self.identity;
        let prefix = format!("building_sets/{}/{}/", id.kind.slug(), id.generation);
        id.authority == authority
            && id.generation > 0
            && id.manifest_sha256.len() == 64
            && id
                .manifest_sha256
                .bytes()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
            && self.locator == format!("manifests/building-{}-v1.buildingset", id.kind.slug())
            && self.paths.len() == id.kind.mesh_roles().len() + id.kind.image_roles().len()
            && self.paths.iter().collect::<HashSet<_>>().len() == self.paths.len()
            && self.paths.iter().all(|path| {
                path.len() <= 512
                    && path.starts_with(&prefix)
                    && !path.contains(['\\', '#', ':'])
                    && !path.chars().any(char::is_control)
                    && path.split('/').all(|part| !matches!(part, "" | "." | ".."))
                    && Path::new(path)
                        .components()
                        .all(|p| matches!(p, Component::Normal(_)))
                    && (path.ends_with(".glb") || path.ends_with(".png"))
            })
            && self.paths.iter().filter(|p| p.ends_with(".glb")).count()
                == id.kind.mesh_roles().len()
            && self.paths.iter().filter(|p| p.ends_with(".png")).count()
                == id.kind.image_roles().len()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Resource)]
#[serde(deny_unknown_fields)]
pub(crate) struct Declaration {
    pub(crate) group: Group,
    fixed: Vec<Binding>,
    candidates: Vec<Binding>,
}

impl Declaration {
    fn validate(&self) -> Result<(), String> {
        for (bindings, expected, authority) in [
            (
                &self.fixed,
                self.group.fixed(),
                BuildingAssetAuthority::ReleaseApproved,
            ),
            (
                &self.candidates,
                self.group.targets(),
                BuildingAssetAuthority::IsolatedCandidate,
            ),
        ] {
            if bindings.len() != expected.len()
                || expected
                    .iter()
                    .any(|kind| bindings.iter().filter(|b| b.identity.kind == *kind).count() != 1)
                || bindings.iter().any(|binding| !binding.valid(authority))
            {
                return Err(
                    "group comparison requires exact released prefix and isolated target bindings"
                        .into(),
                );
            }
        }
        Ok(())
    }

    pub(crate) fn loads(&self, identity: &BuildingAssetSetIdentity, candidate: bool) -> bool {
        self.fixed.iter().any(|b| b.identity == *identity)
            || (candidate && self.candidates.iter().any(|b| b.identity == *identity))
    }

    pub(crate) fn target_paths(&self) -> impl Iterator<Item = &str> {
        self.candidates.iter().flat_map(|binding| {
            std::iter::once(binding.locator.as_str())
                .chain(binding.paths.iter().map(String::as_str))
        })
    }
}

pub(super) fn parse(raw: &str, mode: &str) -> Result<M6Comparison, String> {
    let declaration: Declaration = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    declaration.validate()?;
    let candidate = match mode {
        "candidate" => true,
        "legacy-control" => false,
        _ => return Err("unknown group comparison mode".into()),
    };
    Ok(M6Comparison {
        candidate,
        identities: declaration
            .fixed
            .iter()
            .chain(&declaration.candidates)
            .map(|b| b.identity.clone())
            .collect(),
        pre_release: true,
        group: Some(declaration),
    })
}

fn separate(enabled: bool, static_workload: bool, present: impl Fn(&str) -> bool) -> bool {
    enabled
        && static_workload
        && ![
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
    use crate::plugins::startup::{PerfScenarioConfig, PerfWorkload};
    let config = PerfScenarioConfig::try_from_process().map_err(|e| e.to_string())?;
    if !separate(
        config.enabled(),
        config.workload() == PerfWorkload::BuildingArtStatic,
        |key| std::env::var_os(key).is_some(),
    ) {
        return Err("group comparison requires a separate static profiling workload".into());
    }
    let comparison = parse(
        &std::env::var(GROUP_ENV).map_err(|e| e.to_string())?,
        &std::env::var("HW_BUILDING_M6_MODE").map_err(|e| e.to_string())?,
    )?;
    app.init_resource::<BuildingAssetPool>();
    app.world_mut()
        .resource_mut::<BuildingAssetPool>()
        .allow_comparison_identities(comparison.loaded_identities().cloned().collect());
    app.insert_resource(comparison)
        .add_systems(Startup, request);
    Ok(())
}

fn request(
    comparison: Res<M6Comparison>,
    server: Res<AssetServer>,
    mut pool: ResMut<BuildingAssetPool>,
) {
    let declaration = comparison
        .group
        .as_ref()
        .expect("admitted group declaration");
    for binding in declaration.fixed.iter().chain(&declaration.candidates) {
        if declaration.loads(&binding.identity, comparison.candidate)
            && pool.request(binding.identity.clone(), &binding.locator, &server)
                != BuildingAssetRequest::Started
        {
            error!("group comparison request was not admitted");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Synthetic identities only; no approval or release record is created.
    fn binding(kind: Kind, authority: BuildingAssetAuthority) -> Binding {
        let prefix = format!("building_sets/{}/1/", kind.slug());
        Binding {
            identity: BuildingAssetSetIdentity {
                kind,
                authority,
                generation: 1,
                manifest_sha256: "a".repeat(64),
            },
            locator: format!("manifests/building-{}-v1.buildingset", kind.slug()),
            paths: kind
                .mesh_roles()
                .iter()
                .map(|role| format!("{prefix}{role}.glb"))
                .chain(
                    kind.image_roles()
                        .iter()
                        .map(|role| format!("{prefix}image-{role}.png")),
                )
                .collect(),
        }
    }

    fn fixture(group: Group) -> Declaration {
        Declaration {
            group,
            fixed: group
                .fixed()
                .iter()
                .map(|kind| binding(*kind, BuildingAssetAuthority::ReleaseApproved))
                .collect(),
            candidates: group
                .targets()
                .iter()
                .map(|kind| binding(*kind, BuildingAssetAuthority::IsolatedCandidate))
                .collect(),
        }
    }

    #[test]
    fn exact_groups_keep_fixed_prefix_loaded_in_both_modes_without_granting_target_control() {
        for (group, active, entities, meshes) in [
            (Group::M2, [0, 2], [5, 7], [2, 6]),
            (Group::M3, [2, 4], [7, 11], [6, 8]),
            (Group::M5, [4, 8], [11, 11], [8, 8]),
        ] {
            let declaration = fixture(group);
            let raw = serde_json::to_string(&declaration).unwrap();
            for (index, mode) in ["legacy-control", "candidate"].into_iter().enumerate() {
                let comparison = parse(&raw, mode).unwrap();
                assert_eq!(comparison.loaded_identities().count(), active[index]);
                assert_eq!(comparison.mesh_entities_per_copy(), entities[index]);
                assert_eq!(comparison.unique_target_meshes(), meshes[index]);
                assert_eq!(
                    comparison.foreground_kinds(),
                    usize::from(group == Group::M5 && index == 1) * 4
                );
                let mut pool = BuildingAssetPool::default();
                pool.allow_comparison_identities(comparison.loaded_identities().cloned().collect());
                for binding in &declaration.fixed {
                    assert!(
                        comparison
                            .loaded_identities()
                            .any(|id| *id == binding.identity)
                    );
                }
                for binding in &declaration.candidates {
                    assert_eq!(pool.presentation_allowed(&binding.identity), index == 1);
                    let mut foreign = binding.identity.clone();
                    foreign.generation += 1;
                    assert!(!pool.presentation_allowed(&foreign));
                }
            }
        }
    }

    #[test]
    fn missing_duplicate_foreign_authority_and_paths_never_become_a_group() {
        let original = fixture(Group::M3);
        let mut bad = original.clone();
        bad.fixed.clear();
        assert!(bad.validate().is_err());
        let mut bad = original.clone();
        bad.candidates[0] = bad.candidates[1].clone();
        assert!(bad.validate().is_err());
        let mut bad = original.clone();
        bad.fixed[0].identity.authority = BuildingAssetAuthority::IsolatedCandidate;
        assert!(bad.validate().is_err());
        let mut bad = original.clone();
        bad.candidates[0].identity.authority = BuildingAssetAuthority::ReleaseApproved;
        assert!(bad.validate().is_err());
        for path in [
            "/absolute.png",
            "building_sets/rest-area/1/../escape.png",
            "building_sets/rest-area/1/mesh.glb#Mesh0",
            "building_sets/rest-area/1/source://body.glb",
            "building_sets/rest-area/2/foreign.png",
        ] {
            let mut bad = original.clone();
            bad.candidates[0].paths[0] = path.into();
            assert!(bad.validate().is_err());
        }
        let mut bad = original.clone();
        bad.candidates[0].paths.pop();
        assert!(bad.validate().is_err());
        let mut bad = original.clone();
        bad.candidates[0].identity.kind = Kind::Bridge;
        assert!(bad.validate().is_err());
        let mut raw = serde_json::to_value(original).unwrap();
        raw["unknown"] = serde_json::json!(true);
        assert!(parse(&raw.to_string(), "candidate").is_err());
        assert!(parse("{}", "candidate").is_err());
        assert!(
            parse(
                &serde_json::to_string(&fixture(Group::M2)).unwrap(),
                "unknown"
            )
            .is_err()
        );
    }

    #[test]
    fn any_target_path_started_in_control_is_detected_before_pool_activation() {
        let declaration = fixture(Group::M2);
        let raw = serde_json::to_string(&declaration).unwrap();
        let comparison = parse(&raw, "legacy-control").unwrap();
        let mut app = App::new();
        app.add_plugins((TaskPoolPlugin::default(), AssetPlugin::default()));
        app.init_asset::<Image>();
        let server = app.world().resource::<AssetServer>();
        assert!(comparison.prefetched_target_path(server).is_none());
        let _handle: Handle<Image> = server.load(
            declaration.candidates[0]
                .paths
                .iter()
                .find(|p| p.ends_with(".png"))
                .unwrap()
                .clone(),
        );
        assert!(comparison.prefetched_target_path(server).is_some());
        assert_eq!(comparison.loaded_identities().count(), 0);
        assert!(
            parse(&raw, "candidate")
                .unwrap()
                .prefetched_target_path(server)
                .is_none()
        );
    }

    #[test]
    fn normal_startup_and_other_sessions_are_not_group_admission() {
        assert!(!separate(false, true, |_| false));
        assert!(!separate(true, false, |_| false));
        assert!(separate(true, true, |_| false));
        for key in [
            "HW_BUILDING_M6_CANDIDATES",
            "HW_BUILDING_ASSET_RELEASES",
            "HW_BUILDING_ART_SESSION",
            "HW_BUILDING_ORDINARY_ART_SESSION",
            "HW_BUILDING_PRODUCTION_OBSERVATIONS",
            "HW_M2_ACCEPTANCE_PROBE",
            "HW_BRIDGE_ACCEPTANCE_PROBE",
            "HW_NATIVE_UI_ROOT",
        ] {
            assert!(!separate(true, true, |present| present == key));
        }
    }
}
