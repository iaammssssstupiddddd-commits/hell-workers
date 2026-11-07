//! Explicit ordinary-world ArtPreview feedback. No fixture, approval or gameplay writes.
use super::snapshot_io::require_fresh_output;
use super::{
    BuildingAssetAuthority, BuildingAssetKind, BuildingAssetLoadPolicy, BuildingAssetPool,
    BuildingAssetRequest, BuildingAssetSetIdentity,
};
use crate::systems::visual::building_presentation::EquipmentRoot;
use bevy::prelude::*;
use hw_jobs::{Building, BuildingType};
use hw_visual::{Building3dVisual, TopDownStructuralMaterial};
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::{Component, Path, PathBuf};

const ENV: &str = "HW_BUILDING_ORDINARY_ART_SESSION";
pub(super) const SCOPE: &str = "ordinary-rest-art-feedback-v1";
const CONFLICTS: &[&str] = &[
    "HW_BUILDING_ART_SESSION",
    "HW_BUILDING_PRODUCTION_OBSERVATIONS",
    "HW_BUILDING_ASSET_RELEASES",
    "HW_BUILDING_M6_MODE",
    "HW_M2_ACCEPTANCE_PROBE",
    "HW_M2_ACCEPTANCE_LEG",
    "HW_M2_ACCEPTANCE_REVISION",
    "HW_BRIDGE_ACCEPTANCE_PROBE",
];

#[derive(Clone, Resource, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Session {
    mode: String,
    pub identity: BuildingAssetSetIdentity,
    locator: String,
    pub nonce: String,
    pub output: PathBuf,
}

fn validate(session: &Session, root: &Path, nonce: &str, case: &str) -> Result<(), String> {
    let hex = |text: &str, length| {
        text.len() == length
            && text
                .bytes()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    };
    if session.mode != "feedback"
        || session.identity.kind != BuildingAssetKind::RestArea
        || session.identity.authority != BuildingAssetAuthority::ArtPreview
        || session.identity.generation == 0
        || !hex(&session.identity.manifest_sha256, 64)
        || session.locator != "manifests/building-rest-area-v1.buildingset"
        || !hex(&session.nonce, 32)
        || session.nonce != nonce
        || case != "rest-area-lifecycle"
        || !root.is_absolute()
        || root
            .components()
            .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
        || session.output != root.join("production-cancellation.json")
    {
        return Err(
            "ordinary art feedback requires exact RestArea preview and UI lifecycle context".into(),
        );
    }
    Ok(())
}

pub(super) fn configure(app: &mut App) -> Result<(), String> {
    let Some(raw) = std::env::var_os(ENV) else {
        return Ok(());
    };
    validate_conflicts(|key| std::env::var_os(key).is_some())?;
    let session: Session = serde_json::from_str(
        &raw.into_string()
            .map_err(|_| "invalid art session encoding")?,
    )
    .map_err(|error| error.to_string())?;
    let root = PathBuf::from(
        std::env::var_os("HW_NATIVE_UI_ROOT").ok_or("ordinary art requires UI root")?,
    );
    let nonce = std::env::var("HW_NATIVE_UI_NONCE").map_err(|error| error.to_string())?;
    let case = std::env::var("HW_NATIVE_UI_CASE").map_err(|error| error.to_string())?;
    install_session(app, session, &root, &nonce, &case)
}

fn validate_conflicts(present: impl Fn(&str) -> bool) -> Result<(), String> {
    if CONFLICTS.iter().any(|key| present(key)) {
        return Err(
            "ordinary art feedback cannot share static, seeded or production sessions".into(),
        );
    }
    Ok(())
}

fn install_session(
    app: &mut App,
    session: Session,
    root: &Path,
    nonce: &str,
    case: &str,
) -> Result<(), String> {
    validate(&session, root, nonce, case)?;
    if std::fs::canonicalize(root).map_err(|error| error.to_string())? != root || !root.is_dir() {
        return Err("ordinary art UI root must be a canonical existing directory".into());
    }
    require_fresh_output(&session.output).map_err(|error| error.to_string())?;
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

fn request(session: Res<Session>, server: Res<AssetServer>, mut pool: ResMut<BuildingAssetPool>) {
    assert_eq!(
        pool.request(session.identity.clone(), &session.locator, &server),
        BuildingAssetRequest::Started
    );
}

/// Null outside the dedicated session. Requested, resident and consumed identities
/// remain distinct; a fallback or incomplete load is never labeled ready evidence.
pub(super) fn snapshot(world: &mut World) -> Result<Value, String> {
    let Some(session) = world.get_resource::<Session>().cloned() else {
        return Ok(Value::Null);
    };
    let pool = world.resource::<BuildingAssetPool>();
    if pool.failure(session.identity.kind).is_some() {
        return Err("ordinary art generation failed".into());
    }
    let descriptor = pool.descriptor(session.identity.kind).cloned();
    let active = pool.active(session.identity.kind).cloned();
    let pending = pool.pending(session.identity.kind).cloned();
    if active
        .as_ref()
        .is_some_and(|identity| identity != &session.identity)
        || pending
            .as_ref()
            .is_some_and(|identity| identity != &session.identity)
    {
        return Err("ordinary art active/pending generation differs".into());
    }
    let expected = descriptor.as_ref().map(|set| json!({
        "identity": set.manifest.identity,
        "body_mesh": set.mesh("body").map(|handle| format!("{:?}", handle.id())),
        "albedo": set.image("albedo").map(|handle| format!("{:?}", handle.id())),
        "world_preview": set.image("world_preview").map(|handle| format!("{:?}", handle.id())),
        "catalog": set.image("catalog").map(|handle| format!("{:?}", handle.id())),
        "part_names": set.manifest.parts.iter().map(|part| part.name.as_str()).collect::<Vec<_>>()
        ,"parts": set.manifest.parts
    }));
    let mut query = world.query::<(Entity, &Building3dVisual, &EquipmentRoot, Option<&Children>)>();
    let mut roots = Vec::new();
    for (entity, visual, root, children) in query.iter(world) {
        if !world
            .get::<Building>(visual.owner)
            .is_some_and(|building| building.kind == BuildingType::RestArea)
        {
            continue;
        }
        let parts: Vec<_> = children.into_iter().flat_map(|children| children.iter()).take(17).map(|part| {
            let material = world.get::<MeshMaterial3d<TopDownStructuralMaterial>>(part);
            let material_value = material.and_then(|handle| world.resource::<Assets<TopDownStructuralMaterial>>()
                .get(&handle.0));
            let texture = material_value.and_then(|material| material.base.base_color_texture.as_ref());
            json!({"entity": format!("{part:?}"), "name": world.get::<Name>(part).map(Name::as_str),
                "parent": world.get::<ChildOf>(part).map(|parent| format!("{:?}", parent.parent())),
                "pose": world.get::<Transform>(part).map(|pose| json!({
                    "translation": pose.translation.to_array(), "rotation": pose.rotation.to_array(), "scale": pose.scale.to_array()})),
                "mesh": world.get::<Mesh3d>(part).map(|mesh| format!("{:?}", mesh.id())),
                "material": material.map(|material| format!("{:?}", material.0.id())),
                "albedo": texture.map(|texture| format!("{:?}", texture.id())),
                "material_values": material_value.map(|material| json!({
                    "white": material.base.base_color == Color::WHITE,
                    "opaque": material.base.alpha_mode == AlphaMode::Opaque, "lit": !material.base.unlit,
                    "roughness": material.base.perceptual_roughness, "reflectance": material.base.reflectance,
                    "emissive_black": material.base.emissive == LinearRgba::BLACK,
                    "emissive_texture_absent": material.base.emissive_texture.is_none()})),
                "visible": world.get::<ViewVisibility>(part).map(|visibility| visibility.get())})
        }).collect();
        if parts.len() > 16 || roots.len() == 128 {
            return Err("ordinary art consumer cap exceeded".into());
        }
        let expected_pose = world.get::<Transform>(visual.owner).map(|owner| {
            let mut pose =
                crate::systems::visual::building3d_cleanup::building_presentation_transform(
                    BuildingType::RestArea,
                    owner,
                );
            pose.translation.y = 0.0;
            pose
        });
        roots.push(
            json!({"entity": format!("{entity:?}"), "owner": format!("{:?}", visual.owner),
            "owner_pose_matches": expected_pose.as_ref().is_some_and(|pose| world.get::<Transform>(entity) == Some(pose)),
            "identity": root.observed_identity(), "parts": parts}),
        );
    }
    Ok(
        json!({"requested": session.identity, "active": active, "pending": pending,
        "descriptor": expected, "roots": roots}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_seeded_static_release_or_formal_session_is_excluded() {
        validate_conflicts(|_| false).unwrap();
        for conflict in CONFLICTS {
            assert!(
                validate_conflicts(|key| key == *conflict).is_err(),
                "{conflict}"
            );
        }
    }

    #[test]
    fn exact_policy_survives_default_initialization_and_retained_outputs_are_rejected() {
        let path = std::env::temp_dir().join(format!(
            "ordinary-art-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&path).unwrap();
        let root = std::fs::canonicalize(&path).unwrap();
        let mut input = value();
        input["output"] = json!(root.join("production-cancellation.json"));
        let session: Session = serde_json::from_value(input).unwrap();
        let mut app = App::new();
        install_session(
            &mut app,
            session.clone(),
            &root,
            &"b".repeat(32),
            "rest-area-lifecycle",
        )
        .unwrap();
        app.init_resource::<BuildingAssetLoadPolicy>()
            .init_resource::<BuildingAssetPool>();
        assert_eq!(
            app.world()
                .resource::<BuildingAssetLoadPolicy>()
                .allowed_candidate
                .as_ref(),
            Some(&session.identity)
        );
        assert!(
            app.world()
                .resource::<BuildingAssetPool>()
                .presentation_allowed(&session.identity)
        );
        assert!(
            !app.world()
                .contains_resource::<super::super::acceptance::BuildingArtSession>()
        );
        for output in [
            &session.output,
            &super::super::snapshot_io::temporary_path(&session.output),
        ] {
            std::fs::write(output, b"retained").unwrap();
            assert!(
                install_session(
                    &mut App::new(),
                    session.clone(),
                    &root,
                    &"b".repeat(32),
                    "rest-area-lifecycle"
                )
                .is_err()
            );
            std::fs::remove_file(output).unwrap();
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(root.join("absent"), &session.output).unwrap();
            assert!(
                install_session(
                    &mut App::new(),
                    session.clone(),
                    &root,
                    &"b".repeat(32),
                    "rest-area-lifecycle"
                )
                .is_err()
            );
            std::fs::remove_file(&session.output).unwrap();
        }
        std::fs::remove_dir(root).unwrap();
    }

    fn value() -> Value {
        json!({"mode": "feedback", "identity": {
        "kind": "RestArea", "authority": "art_preview", "generation": 1, "manifest_sha256": "a".repeat(64)},
        "locator": "manifests/building-rest-area-v1.buildingset", "nonce": "b".repeat(32),
        "output": "/private/session/production-cancellation.json"})
    }

    #[test]
    fn exact_preview_context_is_required_without_granting_candidate_authority() {
        let root = Path::new("/private/session");
        let good: Session = serde_json::from_value(value()).unwrap();
        validate(&good, root, &"b".repeat(32), "rest-area-lifecycle").unwrap();
        for (path, replacement) in [
            ("/mode", json!("candidate")),
            ("/identity/kind", json!("SoulSpa")),
            ("/identity/authority", json!("isolated_candidate")),
            ("/identity/authority", json!("release_approved")),
            ("/identity/generation", json!(0)),
            ("/identity/generation", json!(true)),
            ("/identity/manifest_sha256", json!("A".repeat(64))),
            ("/locator", json!("manifests/../foreign.buildingset")),
            ("/nonce", json!("c".repeat(32))),
            ("/output", json!("/foreign/production-cancellation.json")),
        ] {
            let mut changed = value();
            *changed.pointer_mut(path).unwrap() = replacement;
            assert!(match serde_json::from_value::<Session>(changed) {
                Err(_) => true,
                Ok(session) =>
                    validate(&session, root, &"b".repeat(32), "rest-area-lifecycle").is_err(),
            });
        }
        assert!(validate(&good, root, &"b".repeat(32), "navigation").is_err());
        assert!(
            validate(
                &good,
                Path::new("/private/../private/session"),
                &"b".repeat(32),
                "rest-area-lifecycle"
            )
            .is_err()
        );
        let mut unknown = value();
        unknown["promotion_authority"] = json!(true);
        assert!(serde_json::from_value::<Session>(unknown).is_err());
    }
}
