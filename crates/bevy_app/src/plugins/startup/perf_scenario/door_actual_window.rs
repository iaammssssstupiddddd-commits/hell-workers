//! Profiling-only actual-window gallery for production Door asset acceptance.

use std::path::{Path, PathBuf};
use std::time::Duration;

use bevy::mesh::Mesh3d;
use bevy::pbr::MeshMaterial3d;
use bevy::prelude::*;
use bevy::transform::TransformSystems;
use bevy::window::PrimaryWindow;
use hw_core::constants::topdown_rtt_vertical_compensation;
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
    DoorAssetAuthority, DoorAssetReadiness, DoorAssetReadinessState, ProductionDoorAssetPool,
};
use crate::plugins::startup::{Building3dHandles, Camera3dRtt};
use crate::systems::jobs::spawn_building_3d_visual;
use crate::systems::visual::building3d_cleanup::DoorPresentationSyncSet;

use super::PerfScenarioConfig;
use super::fixture::PerfFixtureMarker;

const REQUEST_ENV: &str = "HW_DOOR_ART_ACTUAL_WINDOW";
const STATUS_ENV: &str = "HW_DOOR_ART_STATUS_PATH";
const ACK_ENV: &str = "HW_DOOR_ART_ACK_PATH";
const NONCE_ENV: &str = "HW_DOOR_ART_SESSION_NONCE";
const STATUS_SCHEMA_VERSION: u32 = 2;
const GALLERY_CAMERA_SCALE: f32 = 0.5;
const GALLERY_FARTHEST_CAMERA_SCALE: f32 = 5.0;
const SETTLE: Duration = Duration::from_millis(1_600);
const ACK_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone, Copy)]
struct DoorGalleryCheckpoint {
    phase: &'static str,
    generation: u32,
    zoom: &'static str,
    camera_scale: f32,
}

const CHECKPOINTS: [DoorGalleryCheckpoint; 2] = [
    DoorGalleryCheckpoint {
        phase: "door-gallery-standard",
        generation: 1,
        zoom: "standard",
        camera_scale: GALLERY_CAMERA_SCALE,
    },
    DoorGalleryCheckpoint {
        phase: "door-gallery-farthest",
        generation: 2,
        zoom: "farthest",
        camera_scale: GALLERY_FARTHEST_CAMERA_SCALE,
    },
];

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
    checkpoint_index: usize,
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
            checkpoint_index: 0,
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
        let checkpoint = self.checkpoint();
        std::fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .is_some_and(|value| acknowledgement_matches(&value, nonce, checkpoint))
    }

    fn checkpoint(&self) -> DoorGalleryCheckpoint {
        CHECKPOINTS[self.checkpoint_index]
    }
}

fn acknowledgement_matches(value: &Value, nonce: &str, checkpoint: DoorGalleryCheckpoint) -> bool {
    value.get("schema_version").and_then(Value::as_u64) == Some(STATUS_SCHEMA_VERSION.into())
        && value.get("status").and_then(Value::as_str) == Some("captured")
        && value.get("session_nonce").and_then(Value::as_str) == Some(nonce)
        && value.get("phase").and_then(Value::as_str) == Some(checkpoint.phase)
        && value.get("generation").and_then(Value::as_u64) == Some(checkpoint.generation.into())
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
    mut virtual_time: ResMut<Time<Virtual>>,
    mut params: DoorGalleryViewParams,
) {
    if !acceptance.enabled() || acceptance.targets.is_empty() {
        return;
    }
    virtual_time.pause();
    if let Ok(mut camera) = params.main_camera.single_mut() {
        let center = hw_world::WorldMap::grid_to_world(18, 16);
        camera.translation.x = center.x;
        camera.translation.y = center.y;
        let scale = acceptance.checkpoint().camera_scale;
        camera.scale = Vec3::new(scale, scale, 1.0);
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
    camera: Query<
        'w,
        's,
        (
            &'static Camera,
            &'static Projection,
            &'static GlobalTransform,
        ),
        With<Camera3dRtt>,
    >,
    main_camera: Query<'w, 's, &'static Transform, (With<MainCamera>, Without<Camera3dRtt>)>,
    visuals: GalleryVisualQuery<'w, 's>,
    readiness: Res<'w, DoorAssetReadiness>,
    production: Res<'w, ProductionDoorAssetPool>,
}

pub(crate) fn publish_door_actual_window_status_system(
    time: Res<Time<Real>>,
    config: Res<PerfScenarioConfig>,
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
            if acceptance.checkpoint_index + 1 == CHECKPOINTS.len() {
                acceptance.completed = true;
            } else {
                acceptance.checkpoint_index += 1;
                acceptance.started_at = Some(now);
                acceptance.published_at = None;
            }
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
    let status = build_status(&config, &params, &acceptance).unwrap_or_else(|reason| {
        let checkpoint = acceptance.checkpoint();
        json!({"schema_version": STATUS_SCHEMA_VERSION, "status": "failed", "phase": checkpoint.phase, "generation": checkpoint.generation, "reason": reason})
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
    config: &PerfScenarioConfig,
    params: &DoorGalleryProbeParams,
    acceptance: &DoorActualWindowAcceptance,
) -> Result<Value, String> {
    let checkpoint = acceptance.checkpoint();
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
    let window = params
        .window
        .single()
        .map_err(|_| "Door gallery requires one primary window".to_string())?;
    let physical_width = window.physical_width();
    let physical_height = window.physical_height();
    if physical_width == 0 || physical_height == 0 {
        return Err("Door gallery client has zero physical extent".to_string());
    }
    let (camera, projection, camera_transform) = params
        .camera
        .single()
        .map_err(|_| "Door gallery requires one Camera3dRtt".to_string())?;
    let Projection::Orthographic(orthographic) = projection else {
        return Err("Door gallery Camera3dRtt is not orthographic".to_string());
    };
    if (orthographic.scale - checkpoint.camera_scale).abs() > f32::EPSILON {
        return Err(format!(
            "Door gallery camera scale differs: expected {}, got {}",
            checkpoint.camera_scale, orthographic.scale
        ));
    }
    let main_camera = params
        .main_camera
        .single()
        .map_err(|_| "Door gallery requires one MainCamera".to_string())?;
    let expected_center = hw_world::WorldMap::grid_to_world(18, 16);
    if main_camera.translation.x != expected_center.x
        || main_camera.translation.y != expected_center.y
        || main_camera.scale != Vec3::new(checkpoint.camera_scale, checkpoint.camera_scale, 1.0)
    {
        return Err("Door gallery MainCamera focus differs".to_string());
    }
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
        let viewport_center = project_client_point(
            camera,
            camera_transform,
            transform.translation(),
            physical_width,
            physical_height,
        )
        .ok_or_else(|| format!("Door {:?} cannot be projected", target.grid))?;
        let cell_edge = project_client_point(
            camera,
            camera_transform,
            transform.translation() + Vec3::new(hw_core::constants::TILE_SIZE * 0.45, 0.0, 0.0),
            physical_width,
            physical_height,
        )
        .ok_or_else(|| format!("Door {:?} cell edge cannot be projected", target.grid))?;
        let half_extent = (cell_edge.x - viewport_center.x).abs().floor().max(3.0) as u32;
        let roi = roi_around_point(
            viewport_center,
            half_extent,
            physical_width,
            physical_height,
        )
        .ok_or_else(|| format!("Door {:?} ROI lies outside the client", target.grid))?;
        observations.push(json!({
            "axis": format!("{:?}", target.axis),
            "grid": [target.grid.0, target.grid.1],
            "state": format!("{:?}", target.state),
            "world": [transform.translation().x, transform.translation().y, transform.translation().z],
            "viewport_center": {"x": viewport_center.x, "y": viewport_center.y},
            "roi": {"x": roi.0, "y": roi.1, "width": roi.2, "height": roi.3},
        }));
    }
    let quality = config
        .requested_rtt_quality()
        .ok_or_else(|| "Door gallery RtT quality is absent".to_string())?;
    let scale_factor = config
        .requested_window_scale_factor()
        .ok_or_else(|| "Door gallery scale factor is absent".to_string())?;
    let evidence_kind = match identity.authority {
        DoorAssetAuthority::ArtPreview => "art_preview",
        DoorAssetAuthority::IsolatedCandidate => "isolated_candidate",
        DoorAssetAuthority::ReleaseApproved => "release_approved",
    };
    Ok(json!({
        "schema_version": STATUS_SCHEMA_VERSION,
        "status": "ready",
        "phase": checkpoint.phase,
        "generation": checkpoint.generation,
        "session_nonce": acceptance.nonce,
        "evidence_kind": evidence_kind,
        "checkpoint": {"zoom": checkpoint.zoom, "camera_scale": checkpoint.camera_scale},
        "window": {"width": physical_width, "height": physical_height, "scale_factor": scale_factor},
        "render": {"backend": "vulkan", "render3d": "visible", "rtt_quality": quality.as_str()},
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
        let checkpoint = acceptance.checkpoint();
        let _ = write_json(
            path,
            &json!({"schema_version": STATUS_SCHEMA_VERSION, "status": "failed", "phase": checkpoint.phase, "generation": checkpoint.generation, "reason": reason}),
        );
    }
    acceptance.failed = true;
}

fn project_client_point(
    camera: &Camera,
    camera_transform: &GlobalTransform,
    world_position: Vec3,
    physical_width: u32,
    physical_height: u32,
) -> Option<Vec2> {
    let viewport = camera.logical_viewport_size()?;
    if viewport.x <= 0.0 || viewport.y <= 0.0 {
        return None;
    }
    let point = camera
        .world_to_viewport(camera_transform, world_position)
        .ok()?;
    let center_x = point.x / viewport.x * physical_width as f32;
    let normalized_y = point.y / viewport.y;
    let center_y =
        ((normalized_y - 0.5) * topdown_rtt_vertical_compensation() + 0.5) * physical_height as f32;
    (center_x.is_finite() && center_y.is_finite()).then_some(Vec2::new(center_x, center_y))
}

fn roi_around_point(
    center: Vec2,
    half_extent: u32,
    physical_width: u32,
    physical_height: u32,
) -> Option<(u32, u32, u32, u32)> {
    let half = half_extent as f32;
    if center.x < half
        || center.y < half
        || center.x + half > physical_width as f32
        || center.y + half > physical_height as f32
    {
        return None;
    }
    Some((
        (center.x - half).round() as u32,
        (center.y - half).round() as u32,
        half_extent * 2,
        half_extent * 2,
    ))
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

#[cfg(test)]
mod tests {
    use super::*;

    const NONCE: &str = "0123456789abcdef0123456789abcdef";

    fn acknowledgement(checkpoint: DoorGalleryCheckpoint) -> Value {
        json!({
            "schema_version": STATUS_SCHEMA_VERSION,
            "status": "captured",
            "session_nonce": NONCE,
            "phase": checkpoint.phase,
            "generation": checkpoint.generation,
        })
    }

    #[test]
    fn acknowledgement_is_bound_to_the_current_zoom_checkpoint() {
        let standard = acknowledgement(CHECKPOINTS[0]);
        assert!(acknowledgement_matches(&standard, NONCE, CHECKPOINTS[0]));
        assert!(!acknowledgement_matches(&standard, NONCE, CHECKPOINTS[1]));
    }

    #[test]
    fn acknowledgement_rejects_wrong_schema_nonce_and_generation() {
        for changed in [
            json!({
                "schema_version": 1,
                "status": "captured",
                "session_nonce": NONCE,
                "phase": CHECKPOINTS[0].phase,
                "generation": CHECKPOINTS[0].generation,
            }),
            json!({
                "schema_version": STATUS_SCHEMA_VERSION,
                "status": "captured",
                "session_nonce": "ffffffffffffffffffffffffffffffff",
                "phase": CHECKPOINTS[0].phase,
                "generation": CHECKPOINTS[0].generation,
            }),
            json!({
                "schema_version": STATUS_SCHEMA_VERSION,
                "status": "captured",
                "session_nonce": NONCE,
                "phase": CHECKPOINTS[0].phase,
                "generation": CHECKPOINTS[1].generation,
            }),
        ] {
            assert!(!acknowledgement_matches(&changed, NONCE, CHECKPOINTS[0]));
        }
    }

    #[test]
    fn roi_requires_the_complete_square_inside_the_client() {
        assert_eq!(
            roi_around_point(Vec2::new(50.0, 40.0), 10, 100, 80),
            Some((40, 30, 20, 20))
        );
        assert_eq!(roi_around_point(Vec2::new(5.0, 40.0), 10, 100, 80), None);
    }
}
