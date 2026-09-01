//! Current-source actual-window probe for the production Wall fallback.
//!
//! This opt-in profiling path is deliberately separate from the frozen P02
//! historical acceptance contract. It observes a Wall spawned by the normal
//! `wall-density-v1` fixture and publishes one ACK-held projection ROI for a
//! native client-window capture.

use std::path::{Path, PathBuf};
use std::time::Duration;

use bevy::mesh::{Mesh, Mesh3d};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use hw_core::constants::topdown_rtt_vertical_compensation;
use hw_visual::visual3d::Building3dVisual;
use serde_json::{Value, json};

use super::config::{PerfRenderMode, PerfScenarioConfig};
use super::wall_density_fixture::{
    CAMERA_SCALE, CONTRACT_ID, CONTRACT_SHA256, WallDensityFixtureState,
};
use super::{PerfScenarioSize, PerfWallPhase, PerfWorkload};
use crate::plugins::startup::{Building3dHandles, Camera3dRtt};

const ACCEPTANCE_ENV: &str = "HW_WALL_ART_ACTUAL_WINDOW";
const STATUS_PATH_ENV: &str = "HW_WALL_ART_STATUS_PATH";
const ACK_PATH_ENV: &str = "HW_WALL_ART_ACK_PATH";
const SESSION_NONCE_ENV: &str = "HW_WALL_ART_SESSION_NONCE";
const STATUS_SCHEMA_VERSION: u32 = 1;
const PHASE_ID: &str = "current-wall";
const GENERATION: u32 = 1;
const SETTLE_DURATION: Duration = Duration::from_millis(1_600);
const ACK_TIMEOUT: Duration = Duration::from_secs(15);
const SETTLE_FRAMES: u32 = 3;
const ROI_HALF_SIZE: u32 = 48;

#[derive(Resource)]
pub(crate) struct WallActualWindowAcceptance {
    requested: bool,
    status_path: Option<PathBuf>,
    ack_path: Option<PathBuf>,
    session_nonce: Option<String>,
    started_at: Option<Duration>,
    published_at: Option<Duration>,
    settled_frames: u32,
    completed: bool,
    failed: bool,
}

impl Default for WallActualWindowAcceptance {
    fn default() -> Self {
        Self {
            requested: Self::requested_from_environment(),
            status_path: absolute_path_from_environment(STATUS_PATH_ENV),
            ack_path: absolute_path_from_environment(ACK_PATH_ENV),
            session_nonce: std::env::var(SESSION_NONCE_ENV)
                .ok()
                .filter(|value| is_session_nonce(value)),
            started_at: None,
            published_at: None,
            settled_frames: 0,
            completed: false,
            failed: false,
        }
    }
}

impl WallActualWindowAcceptance {
    pub(crate) fn requested_from_environment() -> bool {
        std::env::var(ACCEPTANCE_ENV).is_ok_and(|value| value == "1")
    }

    fn enabled(&self, config: &PerfScenarioConfig, fixture: &WallDensityFixtureState) -> bool {
        self.requested
            && self.status_path.is_some()
            && self.ack_path.is_some()
            && self.session_nonce.is_some()
            && !self.completed
            && !self.failed
            && config.enabled()
            && config.workload == PerfWorkload::WallDensity
            && config.size == PerfScenarioSize::Small
            && config.wall_phase() == Some(PerfWallPhase::Completed)
            && config.render_mode == PerfRenderMode::Gpu
            && fixture.actual_window_subject().is_some()
    }

    fn acknowledgement_matches(&self) -> bool {
        let (Some(path), Some(nonce)) = (&self.ack_path, &self.session_nonce) else {
            return false;
        };
        let Ok(bytes) = std::fs::read(path) else {
            return false;
        };
        let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
            return false;
        };
        acknowledgement_matches(&value, nonce)
    }
}

fn absolute_path_from_environment(key: &str) -> Option<PathBuf> {
    std::env::var_os(key)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute() && !path.as_os_str().is_empty())
}

fn is_session_nonce(value: &str) -> bool {
    value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn acknowledgement_matches(value: &Value, nonce: &str) -> bool {
    value.get("schema_version").and_then(Value::as_u64) == Some(STATUS_SCHEMA_VERSION.into())
        && value.get("status").and_then(Value::as_str) == Some("captured")
        && value.get("session_nonce").and_then(Value::as_str) == Some(nonce)
        && value.get("phase").and_then(Value::as_str) == Some(PHASE_ID)
        && value.get("generation").and_then(Value::as_u64) == Some(GENERATION.into())
}

type WallVisualQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Building3dVisual,
        &'static Mesh3d,
        &'static GlobalTransform,
        &'static Visibility,
        &'static InheritedVisibility,
    ),
>;

#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct WallActualWindowParams<'w, 's> {
    window: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    camera: Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<Camera3dRtt>>,
    visuals: WallVisualQuery<'w, 's>,
    meshes: Res<'w, Assets<Mesh>>,
    handles: Res<'w, Building3dHandles>,
}

pub(crate) fn publish_wall_actual_window_probe_status_system(
    config: Res<PerfScenarioConfig>,
    fixture: Res<WallDensityFixtureState>,
    params: WallActualWindowParams,
    time: Res<Time<Real>>,
    mut acceptance: ResMut<WallActualWindowAcceptance>,
) {
    if !acceptance.enabled(&config, &fixture) {
        return;
    }
    let now = time.elapsed();
    let started_at = *acceptance.started_at.get_or_insert(now);
    acceptance.settled_frames = acceptance.settled_frames.saturating_add(1);

    if let Some(published_at) = acceptance.published_at {
        if acceptance.acknowledgement_matches() {
            acceptance.completed = true;
            return;
        }
        if now.saturating_sub(published_at) < ACK_TIMEOUT {
            return;
        }
        let failure = failure_status(
            &acceptance,
            "timed out waiting for current-wall capture acknowledgement",
        );
        if let Some(path) = acceptance.status_path.as_deref()
            && let Err(error) = write_status(path, &failure)
        {
            eprintln!("PERF_WALL_ART: cannot write timeout status: {error}");
            return;
        }
        acceptance.failed = true;
        return;
    }

    if acceptance.settled_frames < SETTLE_FRAMES || now.saturating_sub(started_at) < SETTLE_DURATION
    {
        return;
    }
    let Some(path) = acceptance.status_path.clone() else {
        return;
    };
    let status = match build_status(&config, &fixture, &params, &acceptance) {
        Ok(status) => status,
        Err(reason) => failure_status(&acceptance, &reason),
    };
    if let Err(error) = write_status(&path, &status) {
        eprintln!("PERF_WALL_ART: cannot write probe status: {error}");
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
    fixture: &WallDensityFixtureState,
    params: &WallActualWindowParams,
    acceptance: &WallActualWindowAcceptance,
) -> Result<Value, String> {
    let subject = fixture
        .actual_window_subject()
        .ok_or_else(|| "wall-density fixture has no ready probe subject".to_string())?;
    let nonce = acceptance
        .session_nonce
        .as_deref()
        .ok_or_else(|| "wall actual-window session nonce is absent".to_string())?;
    let window = params
        .window
        .single()
        .map_err(|_| "wall actual-window probe requires one primary Window".to_string())?;
    let physical_width = window.physical_width();
    let physical_height = window.physical_height();
    if physical_width == 0 || physical_height == 0 {
        return Err("wall actual-window client has zero physical extent".to_string());
    }
    let (camera, camera_transform) = params
        .camera
        .single()
        .map_err(|_| "wall actual-window probe requires one Camera3dRtt".to_string())?;
    let (mesh, visual_transform, visibility, inherited_visibility) = params
        .visuals
        .iter()
        .find_map(|(visual, mesh, transform, visibility, inherited)| {
            (visual.owner == subject.entity).then_some((mesh, transform, visibility, inherited))
        })
        .ok_or_else(|| "wall actual-window subject has no production 3D visual".to_string())?;
    if *visibility == Visibility::Hidden || !inherited_visibility.get() {
        return Err("wall actual-window subject is hidden".to_string());
    }
    if mesh.0.id() != params.handles.wall_mesh.id() {
        return Err(
            "wall actual-window subject does not use the production fallback mesh".to_string(),
        );
    }
    if !params.meshes.contains(params.handles.wall_mesh.id()) {
        return Err("production fallback Wall mesh is not resident".to_string());
    }
    let center = project_client_point(
        camera,
        camera_transform,
        visual_transform.translation(),
        physical_width,
        physical_height,
    )
    .ok_or_else(|| "wall actual-window subject cannot be projected into the client".to_string())?;
    let roi = roi_around_point(center, ROI_HALF_SIZE, physical_width, physical_height)
        .ok_or_else(|| "wall actual-window ROI lies outside the client".to_string())?;
    let quality = config
        .requested_rtt_quality()
        .ok_or_else(|| "wall actual-window RtT quality is absent".to_string())?;
    let scale_factor = config
        .requested_window_scale_factor()
        .ok_or_else(|| "wall actual-window scale factor is absent".to_string())?;
    Ok(json!({
        "schema_version": STATUS_SCHEMA_VERSION,
        "status": "ready",
        "session_nonce": nonce,
        "phase": PHASE_ID,
        "generation": GENERATION,
        "fixture": {
            "contract_id": CONTRACT_ID,
            "contract_sha256": CONTRACT_SHA256,
            "layout_checksum": subject.layout_checksum,
            "target_size": "N",
            "wall_phase": subject.phase.as_str(),
            "subject_ordinal": 0,
            "subject_grid": [subject.grid.0, subject.grid.1],
            "subject_mask": format!("{:04b}", subject.mask),
        },
        "window": {
            "physical_width": physical_width,
            "physical_height": physical_height,
            "scale_factor": scale_factor,
        },
        "render": {
            "backend": "vulkan",
            "render3d": "visible",
            "rtt_quality": quality.as_str(),
            "camera_scale": CAMERA_SCALE,
            "fallback_mesh_resident": true,
        },
        "probe": {
            "viewport_center": {"x": center.x, "y": center.y},
            "roi": {"x": roi.0, "y": roi.1, "width": roi.2, "height": roi.3},
            "world_position": {
                "x": visual_transform.translation().x,
                "y": visual_transform.translation().y,
                "z": visual_transform.translation().z,
            },
        },
    }))
}

fn failure_status(acceptance: &WallActualWindowAcceptance, reason: &str) -> Value {
    json!({
        "schema_version": STATUS_SCHEMA_VERSION,
        "status": "failed",
        "session_nonce": acceptance.session_nonce.as_deref().unwrap_or(""),
        "phase": PHASE_ID,
        "generation": GENERATION,
        "reason": reason,
    })
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
    half_size: u32,
    physical_width: u32,
    physical_height: u32,
) -> Option<(u32, u32, u32, u32)> {
    let half = half_size as f32;
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
        half_size * 2,
        half_size * 2,
    ))
}

fn write_status(path: &Path, value: &Value) -> std::io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("wall actual-window status path has no parent"))?;
    std::fs::create_dir_all(parent)?;
    let temporary = path.with_extension("tmp");
    let mut bytes = serde_json::to_vec_pretty(value).map_err(std::io::Error::other)?;
    bytes.push(b'\n');
    std::fs::write(&temporary, bytes)?;
    std::fs::rename(temporary, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acknowledgement_is_bound_to_nonce_phase_and_generation() {
        let nonce = "0123456789abcdef0123456789abcdef";
        let valid = json!({
            "schema_version": STATUS_SCHEMA_VERSION,
            "status": "captured",
            "session_nonce": nonce,
            "phase": PHASE_ID,
            "generation": GENERATION,
        });
        assert!(acknowledgement_matches(&valid, nonce));
        let mut wrong_generation = valid.clone();
        wrong_generation["generation"] = json!(2);
        assert!(!acknowledgement_matches(&wrong_generation, nonce));
    }

    #[test]
    fn projected_roi_must_remain_inside_the_client() {
        assert_eq!(
            roi_around_point(Vec2::new(64.0, 64.0), 48, 1280, 720),
            Some((16, 16, 96, 96))
        );
        assert_eq!(roi_around_point(Vec2::new(20.0, 64.0), 48, 1280, 720), None);
    }
}
