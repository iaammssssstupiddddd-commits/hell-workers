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
const CANDIDATE_SCOPE: &str = "ordinary-rest-candidate-lifecycle-v1";
pub(super) const SPA_CANDIDATE_SCOPE: &str = "ordinary-spa-candidate-occupied-v1";
const SPA_RESUMED_SCOPE: &str = "ordinary-spa-candidate-resumed-v1";
const SPA_LOADED_SCOPE: &str = "ordinary-spa-candidate-load-v1";
const M5_SCOPE: &str = "ordinary-m5-art-planning-v1";
const M2_PLANNING_SCOPE: &str = "ordinary-m2-candidate-planning-v1";
const M2_COMPLETION_SCOPE: &str = "ordinary-m2-candidate-completion-v1";

pub(crate) fn m5_kind_for_case(case: &str) -> Option<BuildingAssetKind> {
    match case {
        "m5-sand-planning" => Some(BuildingAssetKind::SandPile),
        "m5-bone-planning" => Some(BuildingAssetKind::BonePile),
        "m5-parking-planning" => Some(BuildingAssetKind::WheelbarrowParking),
        "m5-lamp-planning" => Some(BuildingAssetKind::OutdoorLamp),
        _ => None,
    }
}
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
    let profile = match session.mode.as_str() {
        "m2-tank-candidate-completion"
        | "m2-tank-candidate-inflow"
        | "m2-tank-candidate-full"
        | "m2-tank-candidate-supply" => {
            session.identity.authority == BuildingAssetAuthority::IsolatedCandidate
                && session.identity.kind == BuildingAssetKind::Tank
                && case == session.mode
        }
        "m2-candidate-completion" => {
            session.identity.authority == BuildingAssetAuthority::IsolatedCandidate
                && session.identity.kind == BuildingAssetKind::MudMixer
                && case == "m2-mixer-candidate-completion"
        }
        "m2-candidate-planning" => {
            session.identity.authority == BuildingAssetAuthority::IsolatedCandidate
                && session.identity.kind == BuildingAssetKind::MudMixer
                && case == "m2-mixer-candidate-planning"
        }
        "m5-feedback" => {
            session.identity.authority == BuildingAssetAuthority::ArtPreview
                && m5_kind_for_case(case) == Some(session.identity.kind)
        }
        "feedback" => {
            session.identity.kind == BuildingAssetKind::RestArea
                && session.identity.authority == BuildingAssetAuthority::ArtPreview
                && matches!(case, "rest-area-lifecycle" | "rest-area-maxzoom")
        }
        "candidate" => {
            session.identity.authority == BuildingAssetAuthority::IsolatedCandidate
                && matches!(
                    (session.identity.kind, case),
                    (BuildingAssetKind::RestArea, "rest-area-candidate-lifecycle")
                        | (
                            BuildingAssetKind::SoulSpa,
                            "soul-spa-candidate-occupied-save"
                        )
                )
        }
        "candidate-resumed" => {
            session.identity.authority == BuildingAssetAuthority::IsolatedCandidate
                && session.identity.kind == BuildingAssetKind::SoulSpa
                && case == "soul-spa-candidate-resumed-use"
        }
        "candidate-loaded" => {
            session.identity.authority == BuildingAssetAuthority::IsolatedCandidate
                && session.identity.kind == BuildingAssetKind::SoulSpa
                && case == "soul-spa-candidate-loaded-use"
        }
        _ => false,
    };
    if !profile
        || session.identity.generation == 0
        || !hex(&session.identity.manifest_sha256, 64)
        || session.locator
            != format!(
                "manifests/building-{}-v1.buildingset",
                session.identity.kind.slug()
            )
        || !hex(&session.nonce, 32)
        || session.nonce != nonce
        || !root.is_absolute()
        || root
            .components()
            .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
        || session.output != root.join("production-cancellation.json")
    {
        return Err(
            "ordinary art requires exact kind, authority, mode and dedicated UI lifecycle context"
                .into(),
        );
    }
    Ok(())
}

pub(super) fn scope(world: &World) -> &'static str {
    if world
        .get_resource::<Session>()
        .is_some_and(|session| session.mode == "m2-tank-candidate-supply")
    {
        return "ordinary-m2-tank-candidate-supply-v4";
    }
    if world
        .get_resource::<Session>()
        .is_some_and(|session| session.mode == "m2-tank-candidate-full")
    {
        return "ordinary-m2-tank-candidate-full-v1";
    }
    if world
        .get_resource::<Session>()
        .is_some_and(|session| session.mode == "m2-tank-candidate-inflow")
    {
        return "ordinary-m2-tank-candidate-inflow-v1";
    }
    if world
        .get_resource::<Session>()
        .is_some_and(|session| session.mode == "m2-tank-candidate-completion")
    {
        return "ordinary-m2-tank-candidate-completion-v1";
    }
    if world
        .get_resource::<Session>()
        .is_some_and(|session| session.mode == "m2-candidate-completion")
    {
        return M2_COMPLETION_SCOPE;
    }
    if world
        .get_resource::<Session>()
        .is_some_and(|session| session.mode == "m2-candidate-planning")
    {
        return M2_PLANNING_SCOPE;
    }
    if world
        .get_resource::<Session>()
        .is_some_and(|session| session.mode == "m5-feedback")
    {
        return M5_SCOPE;
    }
    if world.get_resource::<Session>().is_some_and(|session| {
        session.mode == "candidate-loaded" && session.identity.kind == BuildingAssetKind::SoulSpa
    }) {
        return SPA_LOADED_SCOPE;
    }
    if world.get_resource::<Session>().is_some_and(|session| {
        session.mode == "candidate-resumed" && session.identity.kind == BuildingAssetKind::SoulSpa
    }) {
        return SPA_RESUMED_SCOPE;
    }
    if world.get_resource::<Session>().is_some_and(|session| {
        session.mode == "candidate" && session.identity.kind == BuildingAssetKind::SoulSpa
    }) {
        return SPA_CANDIDATE_SCOPE;
    }
    if world
        .get_resource::<Session>()
        .is_some_and(|session| session.mode == "candidate")
    {
        CANDIDATE_SCOPE
    } else {
        SCOPE
    }
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
    if matches!(
        session.mode.as_str(),
        "m5-feedback" | "m2-candidate-planning"
    ) {
        return super::ordinary_m5::snapshot(world, &session.identity);
    }
    let mut expected = descriptor.as_ref().map(|set| json!({
        "identity": set.manifest.identity,
        "body_mesh": set.mesh("body").map(|handle| format!("{:?}", handle.id())),
        "albedo": set.image("albedo").map(|handle| format!("{:?}", handle.id())),
        "world_preview": set.image("world_preview").map(|handle| format!("{:?}", handle.id())),
        "catalog": set.image("catalog").map(|handle| format!("{:?}", handle.id())),
        "part_names": set.manifest.parts.iter().map(|part| part.name.as_str()).collect::<Vec<_>>()
        ,"parts": set.manifest.parts
    }));
    if matches!(
        session.identity.kind,
        BuildingAssetKind::MudMixer | BuildingAssetKind::Tank
    ) && let (Some(set), Some(Value::Object(expected))) =
        (descriptor.as_ref(), expected.as_mut())
    {
        expected.insert(
            "meshes".into(),
            json!(
                set.manifest
                    .identity
                    .kind
                    .mesh_roles()
                    .iter()
                    .map(|role| (*role, set.mesh(role).map(|mesh| format!("{:?}", mesh.id()))))
                    .collect::<std::collections::BTreeMap<_, _>>()
            ),
        );
    }
    if session.identity.kind == BuildingAssetKind::SoulSpa
        && let (Some(set), Some(Value::Object(expected))) = (descriptor.as_ref(), expected.as_mut())
    {
        expected.insert(
            "slot_mesh".into(),
            json!(set.mesh("slot").map(|handle| format!("{:?}", handle.id()))),
        );
        expected.insert(
            "slot_emissive".into(),
            json!(
                set.image("slot_emissive")
                    .map(|handle| format!("{:?}", handle.id()))
            ),
        );
    }
    let kind = match session.identity.kind {
        BuildingAssetKind::RestArea => BuildingType::RestArea,
        BuildingAssetKind::SoulSpa => BuildingType::SoulSpa,
        BuildingAssetKind::MudMixer if session.mode == "m2-candidate-completion" => {
            BuildingType::MudMixer
        }
        BuildingAssetKind::Tank
            if matches!(
                session.mode.as_str(),
                "m2-tank-candidate-completion"
                    | "m2-tank-candidate-inflow"
                    | "m2-tank-candidate-full"
                    | "m2-tank-candidate-supply"
            ) =>
        {
            BuildingType::Tank
        }
        _ => return Err("ordinary art session kind differs".into()),
    };
    let mut query = world.query::<(Entity, &Building3dVisual, &EquipmentRoot, Option<&Children>)>();
    let mut roots = Vec::new();
    for (entity, visual, root, children) in query.iter(world) {
        if !world
            .get::<Building>(visual.owner)
            .is_some_and(|building| building.kind == kind)
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
                    "base_color_srgba": material.base.base_color.to_srgba().to_f32_array(),
                    "white": material.base.base_color == Color::WHITE,
                    "opaque": material.base.alpha_mode == AlphaMode::Opaque, "lit": !material.base.unlit,
                    "roughness": material.base.perceptual_roughness, "reflectance": material.base.reflectance,
                    "emissive_black": material.base.emissive == LinearRgba::BLACK,
                    "emissive_white": material.base.emissive == LinearRgba::WHITE,
                    "emissive_texture": material.base.emissive_texture.as_ref().map(|texture| format!("{:?}", texture.id())),
                    "emissive_texture_absent": material.base.emissive_texture.is_none()})),
                "visible": world.get::<ViewVisibility>(part).map(|visibility| visibility.get())})
        }).collect();
        if parts.len() > 16 || roots.len() == 128 {
            return Err("ordinary art consumer cap exceeded".into());
        }
        let expected_pose = world.get::<Transform>(visual.owner).map(|owner| {
            let mut pose =
                crate::systems::visual::building3d_cleanup::building_presentation_transform(
                    kind, owner,
                );
            pose.translation.y = 0.0;
            pose
        });
        roots.push(
            json!({"entity": format!("{entity:?}"), "owner": format!("{:?}", visual.owner),
            "tank_water_count": (kind == BuildingType::Tank).then(|| world.get::<hw_core::relationships::StoredItems>(visual.owner).map_or(0, |items| items.len())),
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
    fn tank_completion_is_exact_candidate_only_and_never_mixer_or_seeded() {
        let root = Path::new("/unused/tank-completion");
        let session = Session {
            mode: "m2-tank-candidate-completion".into(),
            identity: BuildingAssetSetIdentity {
                kind: BuildingAssetKind::Tank,
                generation: 1,
                authority: BuildingAssetAuthority::IsolatedCandidate,
                manifest_sha256: "a".repeat(64),
            },
            locator: "manifests/building-tank-v1.buildingset".into(),
            nonce: "b".repeat(32),
            output: root.join("production-cancellation.json"),
        };
        validate(
            &session,
            root,
            &session.nonce,
            "m2-tank-candidate-completion",
        )
        .unwrap();
        for case in [
            "m2-mixer-candidate-completion",
            "m2-tank-candidate-planning",
            "m2-ui-tank-empty",
            "navigation",
        ] {
            assert!(validate(&session, root, &session.nonce, case).is_err());
        }
        for authority in [
            BuildingAssetAuthority::ArtPreview,
            BuildingAssetAuthority::ReleaseApproved,
        ] {
            let mut wrong = session.clone();
            wrong.identity.authority = authority;
            assert!(validate(&wrong, root, &wrong.nonce, "m2-tank-candidate-completion").is_err());
        }
        let mut world = World::new();
        let mut inflow = session.clone();
        let mut full = session.clone();
        let mut supply = session.clone();
        supply.mode = "m2-tank-candidate-supply".into();
        validate(&supply, root, &supply.nonce, "m2-tank-candidate-supply").unwrap();
        assert!(validate(&session, root, &session.nonce, "m2-tank-candidate-supply").is_err());
        assert!(validate(&supply, root, &supply.nonce, "m2-tank-candidate-full").is_err());
        for authority in [
            BuildingAssetAuthority::ArtPreview,
            BuildingAssetAuthority::ReleaseApproved,
        ] {
            let mut wrong = supply.clone();
            wrong.identity.authority = authority;
            assert!(validate(&wrong, root, &wrong.nonce, "m2-tank-candidate-supply").is_err());
        }
        let mut wrong = supply.clone();
        wrong.identity.kind = BuildingAssetKind::MudMixer;
        assert!(validate(&wrong, root, &wrong.nonce, "m2-tank-candidate-supply").is_err());
        world.insert_resource(supply);
        assert_eq!(scope(&world), "ordinary-m2-tank-candidate-supply-v4");
        full.mode = "m2-tank-candidate-full".into();
        validate(&full, root, &full.nonce, "m2-tank-candidate-full").unwrap();
        assert!(validate(&session, root, &session.nonce, "m2-tank-candidate-full").is_err());
        assert!(validate(&full, root, &full.nonce, "m2-tank-candidate-inflow").is_err());
        world.insert_resource(full);
        assert_eq!(scope(&world), "ordinary-m2-tank-candidate-full-v1");
        inflow.mode = "m2-tank-candidate-inflow".into();
        validate(&inflow, root, &inflow.nonce, "m2-tank-candidate-inflow").unwrap();
        assert!(validate(&session, root, &session.nonce, "m2-tank-candidate-inflow").is_err());
        assert!(validate(&inflow, root, &inflow.nonce, "m2-tank-candidate-completion").is_err());
        for authority in [
            BuildingAssetAuthority::ArtPreview,
            BuildingAssetAuthority::ReleaseApproved,
        ] {
            let mut wrong = inflow.clone();
            wrong.identity.authority = authority;
            assert!(validate(&wrong, root, &wrong.nonce, "m2-tank-candidate-inflow").is_err());
        }
        world.insert_resource(inflow);
        assert_eq!(scope(&world), "ordinary-m2-tank-candidate-inflow-v1");
        world.insert_resource(session);
        assert_eq!(scope(&world), "ordinary-m2-tank-candidate-completion-v1");
    }

    #[test]
    fn mixer_completion_is_separate_candidate_only_scope_without_seeded_admission() {
        let root = Path::new("/unused/mixer-completion");
        let session = Session {
            mode: "m2-candidate-completion".into(),
            identity: BuildingAssetSetIdentity {
                kind: BuildingAssetKind::MudMixer,
                generation: 1,
                authority: BuildingAssetAuthority::IsolatedCandidate,
                manifest_sha256: "a".repeat(64),
            },
            locator: "manifests/building-mud-mixer-v1.buildingset".into(),
            nonce: "b".repeat(32),
            output: root.join("production-cancellation.json"),
        };
        validate(
            &session,
            root,
            &session.nonce,
            "m2-mixer-candidate-completion",
        )
        .unwrap();
        for case in [
            "m2-mixer-candidate-planning",
            "m2-ui-mixer-refining",
            "navigation",
        ] {
            assert!(validate(&session, root, &session.nonce, case).is_err());
        }
        for authority in [
            BuildingAssetAuthority::ArtPreview,
            BuildingAssetAuthority::ReleaseApproved,
        ] {
            let mut wrong = session.clone();
            wrong.identity.authority = authority;
            assert!(validate(&wrong, root, &wrong.nonce, "m2-mixer-candidate-completion").is_err());
        }
        let mut wrong = session.clone();
        wrong.identity.kind = BuildingAssetKind::Tank;
        assert!(validate(&wrong, root, &wrong.nonce, "m2-mixer-candidate-completion").is_err());
        let mut world = World::new();
        world.insert_resource(session);
        assert_eq!(scope(&world), M2_COMPLETION_SCOPE);
    }

    #[test]
    fn mixer_planning_admits_only_its_exact_candidate_case_without_seeded_authority() {
        let root = Path::new("/unused/mixer-ui");
        let session = Session {
            mode: "m2-candidate-planning".into(),
            identity: BuildingAssetSetIdentity {
                kind: BuildingAssetKind::MudMixer,
                generation: 1,
                authority: BuildingAssetAuthority::IsolatedCandidate,
                manifest_sha256: "a".repeat(64),
            },
            locator: "manifests/building-mud-mixer-v1.buildingset".into(),
            nonce: "b".repeat(32),
            output: root.join("production-cancellation.json"),
        };
        validate(
            &session,
            root,
            &session.nonce,
            "m2-mixer-candidate-planning",
        )
        .unwrap();
        for case in [
            "m2-ui-mixer-refining",
            "m5-lamp-planning",
            "m2-tank-candidate-planning",
            "navigation",
        ] {
            assert!(validate(&session, root, &session.nonce, case).is_err());
        }
        for authority in [
            BuildingAssetAuthority::ArtPreview,
            BuildingAssetAuthority::ReleaseApproved,
        ] {
            let mut wrong = session.clone();
            wrong.identity.authority = authority;
            assert!(validate(&wrong, root, &wrong.nonce, "m2-mixer-candidate-planning").is_err());
        }
        let mut wrong = session.clone();
        wrong.identity.kind = BuildingAssetKind::Tank;
        assert!(validate(&wrong, root, &wrong.nonce, "m2-mixer-candidate-planning").is_err());
        assert!(validate(&session, root, "foreign", "m2-mixer-candidate-planning").is_err());
        let mut world = World::new();
        world.insert_resource(session);
        assert_eq!(scope(&world), M2_PLANNING_SCOPE);
    }

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
    fn candidate_requires_its_explicit_mode_case_and_exact_authority() {
        let root = Path::new("/private/session");
        let mut candidate = value();
        candidate["mode"] = json!("candidate");
        candidate["identity"]["authority"] = json!("isolated_candidate");
        let good: Session = serde_json::from_value(candidate.clone()).unwrap();
        validate(
            &good,
            root,
            &"b".repeat(32),
            "rest-area-candidate-lifecycle",
        )
        .unwrap();
        for case in [
            "rest-area-lifecycle",
            "rest-area-maxzoom",
            "soul-spa-usage",
            "navigation",
        ] {
            assert!(validate(&good, root, &"b".repeat(32), case).is_err());
        }
        for (path, replacement) in [
            ("/mode", json!("feedback")),
            ("/mode", json!("release")),
            ("/identity/kind", json!("SoulSpa")),
            ("/identity/authority", json!("art_preview")),
            ("/identity/authority", json!("release_approved")),
            ("/identity/generation", json!(0)),
            ("/nonce", json!("c".repeat(32))),
        ] {
            let mut changed = candidate.clone();
            *changed.pointer_mut(path).unwrap() = replacement;
            let session: Session = serde_json::from_value(changed).unwrap();
            assert!(
                validate(
                    &session,
                    root,
                    &"b".repeat(32),
                    "rest-area-candidate-lifecycle"
                )
                .is_err()
            );
        }
        let mut app = App::new();
        assert_eq!(scope(app.world()), SCOPE);
        app.insert_resource(good);
        assert_eq!(scope(app.world()), CANDIDATE_SCOPE);
    }

    #[test]
    fn m5_feedback_requires_exact_case_kind_and_preview_authority() {
        let root = Path::new("/private/session");
        for case in [
            "m5-sand-planning",
            "m5-bone-planning",
            "m5-parking-planning",
            "m5-lamp-planning",
        ] {
            let kind = m5_kind_for_case(case).unwrap();
            let mut input = value();
            input["mode"] = json!("m5-feedback");
            input["identity"]["kind"] = json!(kind);
            input["locator"] = json!(format!("manifests/building-{}-v1.buildingset", kind.slug()));
            let session: Session = serde_json::from_value(input.clone()).unwrap();
            validate(&session, root, &"b".repeat(32), case).unwrap();
            let mut app = App::new();
            app.insert_resource(session.clone());
            assert_eq!(scope(app.world()), M5_SCOPE);
            for other in [
                "rest-area-lifecycle",
                "navigation",
                "m5-art-context",
                "bridge-planning",
            ] {
                assert!(validate(&session, root, &"b".repeat(32), other).is_err());
            }
            for (path, replacement) in [
                ("/mode", json!("feedback")),
                ("/identity/authority", json!("isolated_candidate")),
                ("/identity/authority", json!("release_approved")),
                ("/identity/kind", json!("Bridge")),
                ("/identity/generation", json!(0)),
                ("/nonce", json!("c".repeat(32))),
            ] {
                let mut changed = input.clone();
                *changed.pointer_mut(path).unwrap() = replacement;
                let changed: Session = serde_json::from_value(changed).unwrap();
                assert!(validate(&changed, root, &"b".repeat(32), case).is_err());
            }
            for other in [
                "m5-sand-planning",
                "m5-bone-planning",
                "m5-parking-planning",
                "m5-lamp-planning",
            ] {
                assert_eq!(
                    validate(&session, root, &"b".repeat(32), other).is_ok(),
                    case == other
                );
            }
        }
    }

    #[test]
    fn loaded_spa_mode_is_separate_from_the_occupied_and_resumed_cases() {
        let mut input = value();
        input["mode"] = json!("candidate-loaded");
        input["identity"]["kind"] = json!("SoulSpa");
        input["identity"]["authority"] = json!("isolated_candidate");
        input["locator"] = json!("manifests/building-soul-spa-v1.buildingset");
        let session: Session = serde_json::from_value(input.clone()).unwrap();
        let root = Path::new("/private/session");
        validate(
            &session,
            root,
            &"b".repeat(32),
            "soul-spa-candidate-loaded-use",
        )
        .unwrap();
        for case in [
            "soul-spa-candidate-occupied-save",
            "soul-spa-candidate-resumed-use",
            "soul-spa-occupied-save",
        ] {
            assert!(validate(&session, root, &"b".repeat(32), case).is_err());
        }
        for (path, replacement) in [
            ("/mode", json!("candidate")),
            ("/identity/authority", json!("art_preview")),
            ("/identity/kind", json!("RestArea")),
        ] {
            let mut changed = input.clone();
            *changed.pointer_mut(path).unwrap() = replacement;
            let changed: Session = serde_json::from_value(changed).unwrap();
            assert!(
                validate(
                    &changed,
                    root,
                    &"b".repeat(32),
                    "soul-spa-candidate-loaded-use"
                )
                .is_err()
            );
        }
        let mut app = App::new();
        app.insert_resource(session);
        assert_eq!(scope(app.world()), SPA_LOADED_SCOPE);
    }

    #[test]
    fn resumed_spa_mode_cannot_borrow_the_occupied_case_or_authority() {
        let mut input = value();
        input["mode"] = json!("candidate-resumed");
        input["identity"]["kind"] = json!("SoulSpa");
        input["identity"]["authority"] = json!("isolated_candidate");
        input["locator"] = json!("manifests/building-soul-spa-v1.buildingset");
        let root = Path::new("/private/session");
        let session: Session = serde_json::from_value(input.clone()).unwrap();
        validate(
            &session,
            root,
            &"b".repeat(32),
            "soul-spa-candidate-resumed-use",
        )
        .unwrap();
        for case in [
            "soul-spa-candidate-occupied-save",
            "soul-spa-occupied-save",
            "rest-area-candidate-lifecycle",
        ] {
            assert!(validate(&session, root, &"b".repeat(32), case).is_err());
        }
        for (path, replacement) in [
            ("/mode", json!("candidate")),
            ("/identity/authority", json!("art_preview")),
            ("/identity/authority", json!("release_approved")),
            ("/identity/kind", json!("RestArea")),
        ] {
            let mut changed = input.clone();
            *changed.pointer_mut(path).unwrap() = replacement;
            let changed: Session = serde_json::from_value(changed).unwrap();
            assert!(
                validate(
                    &changed,
                    root,
                    &"b".repeat(32),
                    "soul-spa-candidate-resumed-use"
                )
                .is_err()
            );
        }
        let mut app = App::new();
        app.insert_resource(session);
        assert_eq!(scope(app.world()), SPA_RESUMED_SCOPE);
    }

    #[test]
    fn spa_candidate_is_admitted_only_for_its_own_occupied_case_and_scope() {
        let mut input = value();
        input["mode"] = json!("candidate");
        input["identity"]["kind"] = json!("SoulSpa");
        input["identity"]["authority"] = json!("isolated_candidate");
        input["locator"] = json!("manifests/building-soul-spa-v1.buildingset");
        let session: Session = serde_json::from_value(input.clone()).unwrap();
        let root = Path::new("/private/session");
        validate(
            &session,
            root,
            &"b".repeat(32),
            "soul-spa-candidate-occupied-save",
        )
        .unwrap();
        for case in [
            "soul-spa-occupied-save",
            "soul-spa-usage",
            "rest-area-candidate-lifecycle",
            "navigation",
        ] {
            assert!(validate(&session, root, &"b".repeat(32), case).is_err());
        }
        for (path, replacement) in [
            ("/mode", json!("feedback")),
            ("/identity/authority", json!("art_preview")),
            ("/identity/authority", json!("release_approved")),
            ("/identity/kind", json!("RestArea")),
            (
                "/locator",
                json!("manifests/building-rest-area-v1.buildingset"),
            ),
        ] {
            let mut changed = input.clone();
            *changed.pointer_mut(path).unwrap() = replacement;
            let changed: Session = serde_json::from_value(changed).unwrap();
            assert!(
                validate(
                    &changed,
                    root,
                    &"b".repeat(32),
                    "soul-spa-candidate-occupied-save"
                )
                .is_err()
            );
        }
        let mut app = App::new();
        app.insert_resource(session);
        assert_eq!(scope(app.world()), SPA_CANDIDATE_SCOPE);
    }

    #[test]
    fn exact_preview_context_is_required_without_granting_candidate_authority() {
        let root = Path::new("/private/session");
        let good: Session = serde_json::from_value(value()).unwrap();
        validate(&good, root, &"b".repeat(32), "rest-area-lifecycle").unwrap();
        validate(&good, root, &"b".repeat(32), "rest-area-maxzoom").unwrap();
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
