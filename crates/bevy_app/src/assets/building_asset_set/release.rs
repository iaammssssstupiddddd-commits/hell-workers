//! Explicit production bindings for released building asset sets.
//!
//! Production loading is opt-in even after a set has been promoted: the
//! coordinator supplies an exact identity and canonical locator through
//! `HW_BUILDING_ASSET_RELEASES`.  This keeps ordinary startup unchanged while
//! making ReleaseApproved M2, paired M3 and complete M5 sets usable by the normal pool and
//! presentation consumers.  No candidate or preview authority is accepted.

use std::path::{Component, Path};

use bevy::prelude::*;
use serde::Deserialize;

use super::{
    BuildingAssetAuthority, BuildingAssetKind, BuildingAssetPool, BuildingAssetRequest,
    BuildingAssetSetIdentity,
};

const RELEASE_ENV: &str = "HW_BUILDING_ASSET_RELEASES";
const ART_SESSION_ENV: &str = "HW_BUILDING_ART_SESSION";

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseBinding {
    identity: BuildingAssetSetIdentity,
    locator: String,
}

#[derive(Debug, Clone, Resource)]
struct BuildingAssetReleaseBindings(Vec<ReleaseBinding>);

/// Explicit profiling control: validates the same release inputs as candidate,
/// but never requests their manifests or dependencies. Door keeps its own pool.
#[cfg(feature = "profiling")]
#[derive(Debug, Clone, Resource)]
pub(crate) struct M6Comparison {
    pub candidate: bool,
    pub identities: Vec<BuildingAssetSetIdentity>,
}

#[cfg(feature = "profiling")]
fn comparison(mode: &str, bindings: &[ReleaseBinding]) -> Result<M6Comparison, String> {
    validate_bindings(bindings)?;
    if bindings.len() != 8 {
        return Err(
            "M6 requires exactly the eight non-Bridge building sets; Door is separate".into(),
        );
    }
    let candidate = match mode {
        "legacy-control" => false,
        "candidate" => true,
        _ => return Err("HW_BUILDING_M6_MODE must be legacy-control or candidate".into()),
    };
    Ok(M6Comparison {
        candidate,
        identities: bindings
            .iter()
            .map(|binding| binding.identity.clone())
            .collect(),
    })
}

fn canonical_locator(kind: BuildingAssetKind) -> String {
    format!("manifests/building-{}-v1.buildingset", kind.slug())
}

fn validate_binding(binding: &ReleaseBinding) -> Result<(), String> {
    let identity = &binding.identity;
    if identity.authority != BuildingAssetAuthority::ReleaseApproved
        || identity.generation == 0
        || identity.manifest_sha256.len() != 64
        || !identity
            .manifest_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err("production binding must carry a lowercase ReleaseApproved identity".into());
    }
    if binding.locator != canonical_locator(identity.kind)
        || Path::new(&binding.locator).is_absolute()
        || Path::new(&binding.locator)
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err("production binding locator differs from the canonical kind path".into());
    }
    Ok(())
}

fn validate_bindings(bindings: &[ReleaseBinding]) -> Result<(), String> {
    if !matches!(bindings.len(), 2 | 4 | 8 | 9) {
        return Err(
            "production bindings require M2, M2+M3, M2+M3+M5, or all nine asset sets".into(),
        );
    }
    let mut kinds = Vec::with_capacity(bindings.len());
    for binding in bindings {
        validate_binding(binding)?;
        if kinds.contains(&binding.identity.kind) {
            return Err("production bindings contain a duplicate kind".into());
        }
        kinds.push(binding.identity.kind);
    }
    if !kinds.contains(&BuildingAssetKind::Tank) || !kinds.contains(&BuildingAssetKind::MudMixer) {
        return Err("production bindings must include both Tank and MudMixer".into());
    }
    if bindings.len() >= 4
        && (!kinds.contains(&BuildingAssetKind::RestArea)
            || !kinds.contains(&BuildingAssetKind::SoulSpa))
    {
        return Err("production bindings must include the complete M3 pair before M5".into());
    }
    if bindings.len() >= 8
        && [
            BuildingAssetKind::WheelbarrowParking,
            BuildingAssetKind::SandPile,
            BuildingAssetKind::BonePile,
            BuildingAssetKind::OutdoorLamp,
        ]
        .iter()
        .any(|kind| !kinds.contains(kind))
    {
        return Err("production bindings must contain all four M5 kinds".into());
    }
    if kinds.contains(&BuildingAssetKind::Bridge) != (bindings.len() == 9) {
        return Err("Bridge production binding requires the complete nine-set group".into());
    }
    Ok(())
}

/// Configure explicit production requests without granting approval.
///
/// The environment value is a JSON array of `{identity, locator}` objects,
/// one per kind.  Missing configuration is a no-op, preserving ordinary
/// startup and the existing fallback presentation.  Art-preview sessions and
/// production requests are mutually exclusive so one startup cannot race two
/// authority paths for the same kind.
pub fn configure_building_asset_releases(app: &mut App) -> Result<(), String> {
    #[cfg(feature = "profiling")]
    super::observations::configure(app)?;
    let m6 = std::env::var("HW_BUILDING_M6_MODE").ok();
    if m6.is_some() && !cfg!(feature = "profiling") {
        return Err("M6 comparison requires the profiling feature".into());
    }
    let Ok(raw) = std::env::var(RELEASE_ENV) else {
        if m6.is_some() {
            return Err("M6 comparison requires explicit release bindings even for control".into());
        }
        return Ok(());
    };
    if std::env::var_os(ART_SESSION_ENV).is_some() {
        return Err(format!(
            "{RELEASE_ENV} cannot be combined with {ART_SESSION_ENV}"
        ));
    }
    let bindings: Vec<ReleaseBinding> =
        serde_json::from_str(&raw).map_err(|error| error.to_string())?;
    validate_bindings(&bindings)?;
    #[cfg(feature = "profiling")]
    if let Some(mode) = m6 {
        let config = comparison(&mode, &bindings)?;
        let candidate = config.candidate;
        app.insert_resource(config);
        if !candidate {
            return Ok(());
        }
    }
    app.insert_resource(BuildingAssetReleaseBindings(bindings))
        .add_systems(Startup, request_releases);
    Ok(())
}

fn request_releases(
    bindings: Res<BuildingAssetReleaseBindings>,
    server: Res<AssetServer>,
    mut pool: ResMut<BuildingAssetPool>,
) {
    for binding in &bindings.0 {
        let result = pool.request(binding.identity.clone(), &binding.locator, &server);
        if result != BuildingAssetRequest::Started {
            error!(
                kind = ?binding.identity.kind,
                generation = binding.identity.generation,
                ?result,
                "production release request was not admitted"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding(kind: &str) -> String {
        let slug = match kind {
            "Tank" => "tank",
            "Bridge" => "bridge",
            "MudMixer" => "mud-mixer",
            "RestArea" => "rest-area",
            "SoulSpa" => "soul-spa",
            "WheelbarrowParking" => "wheelbarrow-parking",
            "SandPile" => "sand-pile",
            "BonePile" => "bone-pile",
            "OutdoorLamp" => "outdoor-lamp",
            value => value,
        };
        serde_json::json!({
            "identity": {
                "kind": kind,
                "generation": 1,
                "authority": "release_approved",
                "manifest_sha256": "a".repeat(64),
            },
            "locator": format!("manifests/building-{slug}-v1.buildingset"),
        })
        .to_string()
    }

    #[test]
    fn rejects_extra_kind_before_startup_registration() {
        let raw = format!(
            "[{},{},{}]",
            binding("Tank"),
            binding("MudMixer"),
            binding("RestArea")
        );
        let parsed: Vec<ReleaseBinding> = serde_json::from_str(&raw).expect("valid binding JSON");
        assert!(validate_bindings(&parsed).is_err());
    }

    #[test]
    fn requires_exactly_one_of_each_m2_kind() {
        let raw = format!("[{},{}]", binding("Tank"), binding("Tank"));
        let parsed: Vec<ReleaseBinding> = serde_json::from_str(&raw).expect("valid binding JSON");
        assert!(validate_bindings(&parsed).is_err());
    }

    #[test]
    fn accepts_complete_release_groups_without_granting_authority() {
        for kinds in [
            vec![
                "Tank",
                "MudMixer",
                "RestArea",
                "SoulSpa",
                "WheelbarrowParking",
                "SandPile",
                "BonePile",
                "OutdoorLamp",
                "Bridge",
            ],
            vec!["Tank", "MudMixer"],
            vec!["SoulSpa", "Tank", "RestArea", "MudMixer"],
            vec![
                "Tank",
                "MudMixer",
                "RestArea",
                "SoulSpa",
                "WheelbarrowParking",
                "SandPile",
                "BonePile",
                "OutdoorLamp",
            ],
        ] {
            let raw = format!(
                "[{}]",
                kinds
                    .iter()
                    .map(|kind| binding(kind))
                    .collect::<Vec<_>>()
                    .join(",")
            );
            let parsed: Vec<ReleaseBinding> = serde_json::from_str(&raw).unwrap();
            assert!(validate_bindings(&parsed).is_ok());
            for index in 0..parsed.len() {
                let mut invalid = parsed.clone();
                invalid[index].identity.authority = BuildingAssetAuthority::ArtPreview;
                assert!(validate_bindings(&invalid).is_err());
                invalid[index].identity.authority = BuildingAssetAuthority::IsolatedCandidate;
                assert!(validate_bindings(&invalid).is_err());
                invalid = parsed.clone();
                invalid[index].locator.push_str(".other");
                assert!(validate_bindings(&invalid).is_err());
            }
        }
    }

    #[test]
    fn rejects_partial_m5_and_m5_substitution_for_m3() {
        for kinds in [
            vec!["Tank", "MudMixer", "SandPile", "OutdoorLamp"],
            vec!["Tank", "MudMixer", "RestArea", "SoulSpa", "SandPile"],
            vec![
                "Tank",
                "MudMixer",
                "RestArea",
                "SoulSpa",
                "WheelbarrowParking",
                "SandPile",
                "BonePile",
            ],
        ] {
            let raw = format!(
                "[{}]",
                kinds
                    .iter()
                    .map(|kind| binding(kind))
                    .collect::<Vec<_>>()
                    .join(",")
            );
            let parsed: Vec<ReleaseBinding> = serde_json::from_str(&raw).unwrap();
            assert!(validate_bindings(&parsed).is_err());
        }
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn m6_control_requires_the_same_exact_eight_release_bindings_and_excludes_bridge() {
        let kinds = [
            "Tank",
            "MudMixer",
            "RestArea",
            "SoulSpa",
            "WheelbarrowParking",
            "SandPile",
            "BonePile",
            "OutdoorLamp",
        ];
        let mut bindings: Vec<ReleaseBinding> = kinds
            .into_iter()
            .map(|kind| serde_json::from_str(&binding(kind)).unwrap())
            .collect();
        let control = comparison("legacy-control", &bindings).unwrap();
        let candidate = comparison("candidate", &bindings).unwrap();
        assert!(!control.candidate);
        assert!(candidate.candidate);
        assert_eq!(control.identities, candidate.identities);
        assert!(comparison("fallback-on-error", &bindings).is_err());
        bindings.push(serde_json::from_str(&binding("Bridge")).unwrap());
        assert!(comparison("candidate", &bindings).is_err());
        bindings.pop();
        bindings[0].identity.authority = BuildingAssetAuthority::ArtPreview;
        assert!(comparison("legacy-control", &bindings).is_err());
    }
}
