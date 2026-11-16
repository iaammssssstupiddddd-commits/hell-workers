//! Separate profiling-only Tank generation observation, with no release authority.
use super::snapshot_io::{require_fresh_output, write_observation};
use super::{
    BuildingAssetAuthority, BuildingAssetKind, BuildingAssetPool, BuildingAssetRequest,
    BuildingAssetSetIdentity,
};
use crate::plugins::startup::Building3dHandles;
use crate::systems::visual::building_presentation::{EquipmentRoot, spawn_equipment_root};
use bevy::camera_controller::pan_camera::PanCamera;
use bevy::prelude::*;
use hw_core::relationships::StoredIn;
use hw_core::visual_mirror::StockpileVisualState;
use hw_jobs::{Building, BuildingType};
use hw_ui::camera::MainCamera;
use hw_visual::{Building3dVisual, StructuralPresentationState, TopDownStructuralMaterial};
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::PathBuf;

const G3: &str = "3eb6285f60f837280e3d97604ce055e0c651cfa80d2dd95c0937c86f27fd4ba2";
const G4: &str = "f452eefadd39c9a5b250fb928838356ffd071eec69fdfbdfe0d65e5538e6af61";
const LOCATORS: [&str; 2] = [
    "manifests/acceptance-tank-g3.buildingset",
    "manifests/building-tank-v1.buildingset",
];

#[derive(Resource, Deserialize)]
#[serde(deny_unknown_fields)]
struct Probe {
    nonce: String,
    status_path: PathBuf,
    ack_path: PathBuf,
    identities: [BuildingAssetSetIdentity; 2],
    #[serde(skip)]
    owner: Option<Entity>,
    #[serde(skip)]
    phase: u8,
    #[serde(skip)]
    stable: u32,
    #[serde(skip)]
    stable_key: Option<String>,
    #[serde(skip)]
    current_parts: Vec<Entity>,
    #[serde(skip)]
    retired_parts: Vec<Entity>,
    #[serde(skip)]
    failure: Option<String>,
    #[serde(skip)]
    last_write: f64,
}

pub(super) fn configure(app: &mut App) -> Result<(), String> {
    if [
        "HW_BUILDING_ART_SESSION",
        "HW_BUILDING_ASSET_RELEASES",
        "HW_BUILDING_M6_MODE",
        "HW_BUILDING_ORDINARY_ART_SESSION",
        "HW_BUILDING_PRODUCTION_OBSERVATIONS",
        "HW_NATIVE_UI_ROOT",
        "HW_M2_ACCEPTANCE_PROBE",
        "HW_BRIDGE_ACCEPTANCE_PROBE",
        "HW_BUILDING_RELEASE_GALLERY_SESSION",
        "HW_BUILDING_GROUP_ART_SESSION",
        "HW_BUILDING_GROUP_COMPARISON",
        "HW_BUILDING_M6_CANDIDATES",
        "HW_BUILDING_TANK_ILLUSTRATION_PILOT",
        "HW_NATIVE_SAVE_LOAD_ACCEPTANCE_ARTIFACT",
        "HW_NATIVE_DECONSTRUCTION_ACCEPTANCE_ARTIFACT",
        "HW_NATIVE_NOTIFICATION_ACCEPTANCE_ARTIFACT",
    ]
    .iter()
    .any(|name| std::env::var_os(name).is_some())
    {
        return Err("Tank generation probe cannot be combined with other sessions".into());
    }
    if crate::plugins::startup::PerfScenarioConfig::try_from_process()
        .map_err(|e| e.to_string())?
        .enabled()
    {
        return Err("Tank generation probe cannot run with a performance scenario".into());
    }
    let raw = std::env::var("HW_TANK_GENERATION_PROBE").map_err(|e| e.to_string())?;
    let probe: Probe = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    if probe.nonce.len() != 32
        || !probe
            .nonce
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        || !probe.status_path.is_absolute()
        || !probe.ack_path.is_absolute()
        || probe.status_path == probe.ack_path
    {
        return Err("invalid Tank generation probe paths or nonce".into());
    }
    for (id, generation, hash) in [(&probe.identities[0], 3, G3), (&probe.identities[1], 4, G4)] {
        if id.kind != BuildingAssetKind::Tank
            || id.authority != BuildingAssetAuthority::ReleaseApproved
            || id.generation != generation
            || id.manifest_sha256 != hash
        {
            return Err("Tank generation probe identity differs".into());
        }
    }
    require_fresh_output(&probe.status_path).map_err(|e| e.to_string())?;
    require_fresh_output(&probe.ack_path).map_err(|e| e.to_string())?;
    for path in [&probe.status_path, &probe.ack_path] {
        let parent = path.parent().ok_or("probe path requires a parent")?;
        if parent.canonicalize().map_err(|e| e.to_string())? != parent {
            return Err("probe path parent must be canonical and symlink-free".into());
        }
    }
    app.insert_resource(probe)
        .add_systems(PreUpdate, (setup, advance).chain())
        .add_systems(Last, observe);
    Ok(())
}

fn setup(
    mut commands: Commands,
    mut probe: ResMut<Probe>,
    handles: Option<Res<Building3dHandles>>,
    mut camera: Query<(&mut Transform, &mut PanCamera), With<MainCamera>>,
    mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
    mut time: ResMut<Time<Virtual>>,
) {
    if probe.owner.is_some() {
        return;
    }
    let (Some(handles), Ok((mut transform, mut controller)), Ok(mut window)) =
        (handles, camera.single_mut(), windows.single_mut())
    else {
        return;
    };
    let pos = hw_world::WorldMap::grid_to_world(25, 25);
    transform.translation.x = pos.x;
    transform.translation.y = pos.y;
    transform.scale = Vec3::ONE;
    controller.zoom_factor = 1.0;
    let away = Vec2::new(window.width() * 0.8, window.height() * 0.7);
    window.set_cursor_position(Some(away));
    time.pause();
    let owner = commands
        .spawn((
            Building {
                kind: BuildingType::Tank,
                is_provisional: false,
            },
            Transform::from_translation(pos.extend(0.0)),
            StockpileVisualState { capacity: 2 },
        ))
        .id();
    commands.spawn(StoredIn(owner));
    spawn_equipment_root(&mut commands, owner, BuildingType::Tank, pos, &handles);
    probe.owner = Some(owner);
}

fn advance(
    mut probe: ResMut<Probe>,
    server: Res<AssetServer>,
    mut pool: ResMut<BuildingAssetPool>,
) {
    if probe.stable < 30 || probe.phase >= 5 {
        return;
    }
    let Ok(bytes) = std::fs::read(&probe.ack_path) else {
        return;
    };
    let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
        return;
    };
    if value != json!({"nonce": probe.nonce, "phase": probe.phase, "pid": std::process::id()}) {
        return;
    }
    let accepted = match probe.phase {
        0 | 1 => {
            pool.request(
                probe.identities[probe.phase as usize].clone(),
                LOCATORS[probe.phase as usize],
                &server,
            ) == BuildingAssetRequest::Started
        }
        2 | 4 => true,
        3 => pool.invalidate_active(&probe.identities[1]),
        _ => false,
    };
    if accepted {
        if probe.phase == 1 {
            pool.hold_publication(true);
        } else if probe.phase == 2 {
            pool.hold_publication(false);
        }
        if probe.phase != 1 {
            probe.retired_parts = probe.current_parts.clone();
        }
        probe.phase += 1;
        probe.stable = 0;
        probe.stable_key = None;
    } else {
        probe.failure = Some("generation request or invalidation was rejected".into());
    }
}

fn observe(world: &mut World) {
    let (owner, phase, nonce, path) = {
        let probe = world.resource::<Probe>();
        (
            probe.owner,
            probe.phase,
            probe.nonce.clone(),
            probe.status_path.clone(),
        )
    };
    let Some(owner) = owner else {
        return;
    };
    let pool = world.resource::<BuildingAssetPool>();
    let active = pool.active(BuildingAssetKind::Tank).cloned();
    let pending = pool.pending(BuildingAssetKind::Tank).cloned();
    let failure = pool
        .failure(BuildingAssetKind::Tank)
        .map(|failure| format!("{failure:?}"));
    let handles = pool.descriptor(BuildingAssetKind::Tank).map(|d| {
        json!({
        "body": d.mesh("body").map(|h| format!("{:?}", h.id())),
        "water": d.mesh("water").map(|h| format!("{:?}", h.id())),
        "albedo": d.image("albedo").map(|h| format!("{:?}", h.id())),
        "world_preview": d.image("world_preview").map(|h| format!("{:?}", h.id()))})
    });
    let mut query =
        world.query_filtered::<(Entity, &Building3dVisual, &EquipmentRoot), With<EquipmentRoot>>();
    let roots: Vec<_> = query
        .iter(world)
        .filter(|(_, visual, _)| visual.owner == owner)
        .map(|(entity, _, root)| (entity, root.observed_identity().cloned()))
        .collect();
    let mut parts = Vec::new();
    let mut part_entities = Vec::new();
    if let [(root, _)] = roots.as_slice()
        && let Some(children) = world.get::<Children>(*root)
    {
        for part in children.iter() {
            part_entities.push(part);
            let mesh = world.get::<Mesh3d>(part);
            let material = world
                .get::<MeshMaterial3d<TopDownStructuralMaterial>>(part)
                .and_then(|h| {
                    world
                        .resource::<Assets<TopDownStructuralMaterial>>()
                        .get(&h.0)
                });
            parts.push(json!({"entity": format!("{part:?}"),
                    "name": world.get::<Name>(part).map(Name::as_str),
                    "mesh": mesh.map(|m| format!("{:?}", m.0.id())),
                    "mesh_resident": mesh.is_some_and(|m| world.resource::<Assets<Mesh>>().get(&m.0).is_some()),
                    "visible": world.get::<ViewVisibility>(part).is_some_and(|v| v.get()),
                    "layer_3d": world.get::<bevy::camera::visibility::RenderLayers>(part).is_some_and(|l| l.iter().any(|v| v == hw_core::constants::LAYER_3D)),
                    "translation": world.get::<Transform>(part).map(|t| t.translation.to_array()),
                    "rotation": world.get::<Transform>(part).map(|t| t.rotation.to_array()),
                    "scale": world.get::<Transform>(part).map(|t| t.scale.to_array()),
                    "albedo": material.and_then(|m| m.base.base_color_texture.as_ref()).map(|h| format!("{:?}", h.id())),
                    "body_paint": material.map(crate::systems::visual::building_presentation::body_paint_snapshot),
                    "opaque": material.is_some_and(|m| m.base.alpha_mode == AlphaMode::Opaque),
                    "lit": material.is_some_and(|m| !m.base.unlit),
                    "albedo_resident": material.and_then(|m| m.base.base_color_texture.as_ref()).is_some_and(|h| world.resource::<Assets<Image>>().get(h).is_some())}));
        }
    }
    let probe = world.resource::<Probe>();
    let expected = match phase {
        0 | 4 => active.is_none(),
        1 | 2 => active.as_ref() == Some(&probe.identities[0]),
        3 => active.as_ref() == Some(&probe.identities[1]),
        _ => false,
    };
    let root_identity = roots.first().and_then(|(_, id)| id.clone());
    let state = roots
        .first()
        .map(|(entity, _)| format!("{:?}", world.get::<StructuralPresentationState>(*entity)));
    let retired_gone = probe
        .retired_parts
        .iter()
        .all(|entity| world.get_entity(*entity).is_err());
    let pending_matches = if phase == 2 {
        pending.as_ref() == Some(&probe.identities[1])
    } else {
        pending.is_none()
    };
    let owner_pos = world
        .get::<Transform>(owner)
        .map(|t| t.translation.to_array());
    let root_pos = roots
        .first()
        .and_then(|(entity, _)| world.get::<Transform>(*entity))
        .map(|t| t.translation.to_array());
    let mut camera = world.query_filtered::<(&Transform, &PanCamera), With<MainCamera>>();
    let camera = camera.iter(world).next().map(|(transform, controller)| {
        json!({"position": transform.translation.to_array(), "scale": transform.scale.to_array(),
            "zoom": controller.zoom_factor})
    });
    let stable = expected
        && pending_matches
        && failure.is_none()
        && roots.len() == 1
        && root_identity == active
        && state.as_deref() == Some("Some(TankPartial)")
        && retired_gone
        && camera.is_some()
        && owner_pos.is_some()
        && root_pos.is_some()
        && parts.len() == if active.is_some() { 2 } else { 1 }
        && parts
            .iter()
            .all(|p| p["mesh_resident"] == true && p["visible"] == true && p["layer_3d"] == true);
    let key = json!({"phase": phase, "owner": format!("{owner:?}"), "root": roots.first().map(|(e, _)| format!("{e:?}")),
        "identity": root_identity, "parts": parts, "handles": handles, "state": state,
        "owner_pos": owner_pos, "root_pos": root_pos, "camera": camera}).to_string();
    let now = world.resource::<Time<Real>>().elapsed_secs_f64();
    let mut probe = world.resource_mut::<Probe>();
    probe.stable = if stable && probe.stable_key.as_deref() == Some(&key) {
        probe.stable.saturating_add(1)
    } else if stable {
        1
    } else {
        0
    };
    probe.stable_key = stable.then_some(key);
    probe.current_parts = part_entities;
    if now - probe.last_write < 0.1 {
        return;
    }
    probe.last_write = now;
    let snapshot = json!({"schema_version": 1, "nonce": nonce, "pid": std::process::id(), "phase": phase,
        "stable": probe.stable, "owner": format!("{owner:?}"), "active": active, "pending": pending,
        "failure": failure.or(probe.failure.clone()), "root": roots.first().map(|(e, _)| format!("{e:?}")), "root_identity": root_identity,
        "parts": parts, "handles": handles, "retired_gone": retired_gone, "state": state,
        "owner_pos": owner_pos, "root_pos": root_pos, "camera": camera});
    if let Err(error) = write_observation(
        &path,
        &serde_json::to_vec(&snapshot).expect("snapshot serializes"),
    ) {
        error!("Tank generation observation failed: {error}");
    }
}
