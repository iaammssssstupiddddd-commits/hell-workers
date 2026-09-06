//! Profiling-only actual-window gallery for the unapproved production Door.

use std::path::{Path, PathBuf};
use std::time::Duration;

use bevy::mesh::Mesh3d;
use bevy::pbr::MeshMaterial3d;
use bevy::prelude::*;
use bevy::transform::TransformSystems;
use bevy::window::PrimaryWindow;
use hw_core::visual_mirror::building::{BuildingTypeVisual, BuildingVisualState};
use hw_core::visual_mirror::construction::BlueprintVisualState;
use hw_core::world::DoorState;
use hw_jobs::{Building, BuildingType, Door};
use hw_ui::camera::MainCamera;
use hw_visual::TopDownStructuralMaterial;
use hw_visual::visual3d::{
    Building3dVisual, Door3dPresentationMode, DoorPresentationAxis, DoorPresentationState,
};
use serde_json::{Value, json};

use crate::assets::door_asset_set::{
    DoorAssetReadiness, DoorAssetReadinessState, ProductionDoorAssetPool,
};
use crate::plugins::startup::{Building3dHandles, Camera3dRtt};
use crate::systems::jobs::spawn_building_3d_visual;
use crate::systems::visual::building3d_cleanup::DoorPresentationSyncSet;

use super::fixture::PerfFixtureMarker;

const REQUEST_ENV: &str = "HW_DOOR_ART_ACTUAL_WINDOW";
const STATUS_ENV: &str = "HW_DOOR_ART_STATUS_PATH";
const ACK_ENV: &str = "HW_DOOR_ART_ACK_PATH";
const NONCE_ENV: &str = "HW_DOOR_ART_SESSION_NONCE";
const PHASE: &str = "door-gallery";
const GENERATION: u32 = 1;
const GALLERY_CAMERA_SCALE: f32 = 0.5;
const SETTLE: Duration = Duration::from_millis(1_600);
const ACK_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone, Copy)]
struct DoorGalleryTarget {
    owner: Entity,
    state: DoorPresentationState,
    axis: DoorPresentationAxis,
    grid: (i32, i32),
}

#[derive(Resource)]
pub(crate) struct DoorActualWindowAcceptance {
    requested: bool,
    status_path: Option<PathBuf>,
    ack_path: Option<PathBuf>,
    nonce: Option<String>,
    targets: Vec<DoorGalleryTarget>,
    started_at: Option<Duration>,
    published_at: Option<Duration>,
    completed: bool,
    failed: bool,
}

impl Default for DoorActualWindowAcceptance {
    fn default() -> Self {
        Self {
            requested: Self::requested_from_environment(),
            status_path: absolute_path(STATUS_ENV),
            ack_path: absolute_path(ACK_ENV),
            nonce: std::env::var(NONCE_ENV).ok().filter(|value| {
                value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
            }),
            targets: Vec::new(),
            started_at: None,
            published_at: None,
            completed: false,
            failed: false,
        }
    }
}

impl DoorActualWindowAcceptance {
    pub(crate) fn requested_from_environment() -> bool {
        std::env::var(REQUEST_ENV).as_deref() == Ok("1")
    }

    fn enabled(&self) -> bool {
        self.requested
            && self.status_path.is_some()
            && self.ack_path.is_some()
            && self.nonce.is_some()
            && !self.completed
            && !self.failed
    }

    fn acknowledged(&self) -> bool {
        let (Some(path), Some(nonce)) = (&self.ack_path, &self.nonce) else {
            return false;
        };
        std::fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .is_some_and(|value| {
                value.get("status").and_then(Value::as_str) == Some("captured")
                    && value.get("session_nonce").and_then(Value::as_str) == Some(nonce)
                    && value.get("phase").and_then(Value::as_str) == Some(PHASE)
                    && value.get("generation").and_then(Value::as_u64) == Some(GENERATION.into())
            })
    }
}

fn absolute_path(key: &str) -> Option<PathBuf> {
    std::env::var_os(key)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute() && !path.as_os_str().is_empty())
}

fn connector_state(grid: (i32, i32)) -> BlueprintVisualState {
    BlueprintVisualState {
        is_wall_or_door: true,
        occupied_grids: vec![grid],
        ..default()
    }
}

pub(crate) fn setup_door_actual_window_gallery_system(
    mut commands: Commands,
    handles: Res<Building3dHandles>,
    mut acceptance: ResMut<DoorActualWindowAcceptance>,
) {
    if !acceptance.enabled() || !acceptance.targets.is_empty() {
        return;
    }
    let rows = [
        (18, DoorPresentationAxis::EastWest),
        (14, DoorPresentationAxis::NorthSouth),
    ];
    let states = [
        (14, DoorState::Closed, DoorPresentationState::Closed),
        (18, DoorState::Open, DoorPresentationState::Open),
        (22, DoorState::Locked, DoorPresentationState::Locked),
    ];
    for (y, axis) in rows {
        for (x, semantic, presentation) in states {
            let grid = (x, y);
            let position = hw_world::WorldMap::grid_to_world(x, y);
            let owner = commands
                .spawn((
                    Building {
                        kind: BuildingType::Door,
                        is_provisional: false,
                    },
                    BuildingVisualState {
                        kind: BuildingTypeVisual::Door,
                        is_provisional: false,
                    },
                    Door { state: semantic },
                    Transform::from_xyz(position.x, position.y, 0.0),
                    Visibility::Inherited,
                    Name::new(format!("Door Art Gallery {axis:?} {presentation:?}")),
                ))
                .id();
            spawn_building_3d_visual(
                &mut commands,
                owner,
                BuildingType::Door,
                position,
                false,
                &handles,
            );
            let connectors = match axis {
                DoorPresentationAxis::EastWest => [(x - 1, y), (x + 1, y)],
                DoorPresentationAxis::NorthSouth => [(x, y - 1), (x, y + 1)],
            };
            for connector in connectors {
                commands.spawn((
                    connector_state(connector),
                    Name::new("Door Gallery Connector"),
                ));
            }
            acceptance.targets.push(DoorGalleryTarget {
                owner,
                state: presentation,
                axis,
                grid,
            });
        }
    }
}

#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct DoorGalleryViewParams<'w, 's> {
    main_camera: Query<'w, 's, &'static mut Transform, (With<MainCamera>, Without<Camera3dRtt>)>,
    ui_roots: Query<'w, 's, &'static mut Node, Without<ChildOf>>,
    fixture_roots: Query<
        'w,
        's,
        &'static mut Visibility,
        (With<PerfFixtureMarker>, Without<Building3dVisual>),
    >,
    building_visuals: Query<
        'w,
        's,
        (&'static Building3dVisual, &'static mut Visibility),
        Without<PerfFixtureMarker>,
    >,
}

pub(crate) fn prepare_door_actual_window_gallery_view_system(
    acceptance: Res<DoorActualWindowAcceptance>,
    mut params: DoorGalleryViewParams,
) {
    if !acceptance.enabled() || acceptance.targets.is_empty() {
        return;
    }
    if let Ok(mut camera) = params.main_camera.single_mut() {
        let center = hw_world::WorldMap::grid_to_world(18, 16);
        camera.translation.x = center.x;
        camera.translation.y = center.y;
        camera.scale = Vec3::new(GALLERY_CAMERA_SCALE, GALLERY_CAMERA_SCALE, 1.0);
    }
    for mut node in &mut params.ui_roots {
        node.display = Display::None;
    }
    for mut visibility in &mut params.fixture_roots {
        *visibility = Visibility::Hidden;
    }
    for (visual, mut visibility) in &mut params.building_visuals {
        if !acceptance
            .targets
            .iter()
            .any(|target| target.owner == visual.owner)
        {
            *visibility = Visibility::Hidden;
        }
    }
}

type GalleryVisualQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Building3dVisual,
        &'static DoorPresentationState,
        &'static DoorPresentationAxis,
        &'static Door3dPresentationMode,
        &'static Mesh3d,
        &'static MeshMaterial3d<TopDownStructuralMaterial>,
        &'static GlobalTransform,
    ),
>;

#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct DoorGalleryProbeParams<'w, 's> {
    window: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    visuals: GalleryVisualQuery<'w, 's>,
    readiness: Res<'w, DoorAssetReadiness>,
    production: Res<'w, ProductionDoorAssetPool>,
}

pub(crate) fn publish_door_actual_window_status_system(
    time: Res<Time<Real>>,
    params: DoorGalleryProbeParams,
    mut acceptance: ResMut<DoorActualWindowAcceptance>,
) {
    if !acceptance.enabled() || acceptance.targets.len() != 6 {
        return;
    }
    let now = time.elapsed();
    let started = *acceptance.started_at.get_or_insert(now);
    if let Some(published) = acceptance.published_at {
        if acceptance.acknowledged() {
            acceptance.completed = true;
        } else if now.saturating_sub(published) >= ACK_TIMEOUT {
            publish_failure(
                &mut acceptance,
                "Door gallery capture acknowledgement timed out",
            );
        }
        return;
    }
    if now.saturating_sub(started) < SETTLE {
        return;
    }
    let status = build_status(&params, &acceptance).unwrap_or_else(|reason| {
        json!({"schema_version": 1, "status": "failed", "phase": PHASE, "generation": GENERATION, "reason": reason})
    });
    let Some(path) = acceptance.status_path.as_deref() else {
        return;
    };
    if write_json(path, &status).is_err() {
        return;
    }
    if status.get("status").and_then(Value::as_str) == Some("ready") {
        acceptance.published_at = Some(now);
    } else {
        acceptance.failed = true;
    }
}

fn build_status(
    params: &DoorGalleryProbeParams,
    acceptance: &DoorActualWindowAcceptance,
) -> Result<Value, String> {
    let identity = match &params.readiness.state {
        DoorAssetReadinessState::Eligible(identity) => identity,
        other => return Err(format!("Door asset set is not eligible: {other:?}")),
    };
    let resolved = params
        .production
        .resolved
        .as_ref()
        .filter(|resolved| &resolved.identity == identity)
        .ok_or_else(|| "Door production pool identity differs".to_string())?;
    let mut observations = Vec::new();
    for target in &acceptance.targets {
        let matches: Vec<_> = params
            .visuals
            .iter()
            .filter(|(visual, ..)| visual.owner == target.owner)
            .collect();
        if matches.len() != 1 {
            return Err(format!(
                "Door {:?} has {} visual roots",
                target.grid,
                matches.len()
            ));
        }
        let (_, state, axis, mode, mesh, _, transform) = matches[0];
        if *state != target.state
            || *axis != target.axis
            || *mode != Door3dPresentationMode::Production
        {
            return Err(format!("Door {:?} presentation differs", target.grid));
        }
        let mesh_index = match target.state {
            DoorPresentationState::Closed => 0,
            DoorPresentationState::Open => 1,
            DoorPresentationState::Locked => 2,
        };
        if mesh.0 != resolved.meshes[mesh_index] {
            return Err(format!("Door {:?} mesh role differs", target.grid));
        }
        observations.push(json!({
            "axis": format!("{:?}", target.axis),
            "grid": [target.grid.0, target.grid.1],
            "state": format!("{:?}", target.state),
            "world": [transform.translation().x, transform.translation().y, transform.translation().z],
        }));
    }
    let window = params
        .window
        .single()
        .map_err(|_| "Door gallery requires one primary window".to_string())?;
    Ok(json!({
        "schema_version": 1,
        "status": "ready",
        "phase": PHASE,
        "generation": GENERATION,
        "session_nonce": acceptance.nonce,
        "evidence_kind": "art_preview",
        "window": {"width": window.physical_width(), "height": window.physical_height()},
        "candidate_identity": {
            "asset_set_generation": identity.asset_set_generation,
            "authority": format!("{:?}", identity.authority),
            "manifest_sha256": identity.manifest_sha256,
        },
        "gallery": {"production_count": 6, "fallback_count": 0, "targets": observations},
    }))
}

fn publish_failure(acceptance: &mut DoorActualWindowAcceptance, reason: &str) {
    if let Some(path) = acceptance.status_path.as_deref() {
        let _ = write_json(
            path,
            &json!({"schema_version": 1, "status": "failed", "phase": PHASE, "generation": GENERATION, "reason": reason}),
        );
    }
    acceptance.failed = true;
}

fn write_json(path: &Path, value: &Value) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("tmp");
    let bytes = serde_json::to_vec_pretty(value).map_err(std::io::Error::other)?;
    std::fs::write(&temporary, bytes)?;
    std::fs::rename(temporary, path)
}

pub(crate) fn configure_door_actual_window_probe(app: &mut App) {
    app.add_systems(
        Update,
        (
            setup_door_actual_window_gallery_system,
            ApplyDeferred,
            prepare_door_actual_window_gallery_view_system,
        )
            .chain(),
    )
    .add_systems(
        PostUpdate,
        publish_door_actual_window_status_system
            .after(DoorPresentationSyncSet)
            .after(TransformSystems::Propagate),
    );
}
