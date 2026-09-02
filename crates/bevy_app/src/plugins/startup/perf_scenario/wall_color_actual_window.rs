//! Actual-window color board for the production Wall calibration contract.
//!
//! This profiling-only path renders five isolated `Sprite` patches through a
//! dedicated final Camera2d. Four patches use the same unlit sRGB base-color
//! path; the fifth multiplies the purple patch in linear space to prove the
//! emissive luminance direction without involving gameplay lighting.

use std::path::{Path, PathBuf};
use std::time::Duration;

use bevy::camera::visibility::RenderLayers;
use bevy::color::LinearRgba;
use bevy::prelude::*;
use bevy::ui::IsDefaultUiCamera;
use bevy::window::PrimaryWindow;
use hw_ui::camera::MainCamera;
use serde_json::{Value, json};
#[cfg(test)]
use sha2::{Digest, Sha256};

use super::config::{PerfRenderMode, PerfScenarioConfig};
use super::{PerfScenarioSize, PerfWallPhase, PerfWorkload};

const ACCEPTANCE_ENV: &str = "HW_WALL_COLOR_ACTUAL_WINDOW";
const STATUS_PATH_ENV: &str = "HW_WALL_COLOR_STATUS_PATH";
const ACK_PATH_ENV: &str = "HW_WALL_COLOR_ACK_PATH";
const SESSION_NONCE_ENV: &str = "HW_WALL_COLOR_SESSION_NONCE";
const STATUS_SCHEMA_VERSION: u32 = 1;
const PHASE_ID: &str = "wall-color-board";
const GENERATION: u32 = 1;
const SETTLE_DURATION: Duration = Duration::from_millis(1_600);
const ACK_TIMEOUT: Duration = Duration::from_secs(15);
const SETTLE_FRAMES: u32 = 3;
const CAMERA_ORDER: isize = 100;
const RENDER_LAYER: usize = 31;
const BOARD_WIDTH: u32 = 320;
const BOARD_HEIGHT: u32 = 96;
const PATCH_SIZE: f32 = 32.0;
const EMISSIVE_STRENGTH: f32 = 2.0;

pub(super) const CONTRACT_ID: &str = "wall-color-calibration-v1";
#[cfg(test)]
const CONTRACT_BYTES: &[u8] = include_bytes!(
    "../../../../../../tools/blender_ai_workflow/fixtures/wall-color-calibration-v1.json"
);
pub(super) const CONTRACT_SHA256: &str =
    "73a25e1ccc9a039f053f4293cc677b3d980f743a8700e28564740bbf40b22586";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PatchSpec {
    id: &'static str,
    center_px: (u32, u32),
    input_rgb: (u8, u8, u8),
    emissive: bool,
}

const PATCHES: [PatchSpec; 5] = [
    PatchSpec {
        id: "stone",
        center_px: (32, 32),
        input_rgb: (0x21, 0x1b, 0x1b),
        emissive: false,
    },
    PatchSpec {
        id: "rust",
        center_px: (96, 32),
        input_rgb: (0x8c, 0x4a, 0x2f),
        emissive: false,
    },
    PatchSpec {
        id: "dark_brown_line",
        center_px: (160, 32),
        input_rgb: (0x1a, 0x0a, 0x00),
        emissive: false,
    },
    PatchSpec {
        id: "purple",
        center_px: (224, 32),
        input_rgb: (0x8b, 0x00, 0x8b),
        emissive: false,
    },
    PatchSpec {
        id: "purple_emissive",
        center_px: (288, 32),
        input_rgb: (0x8b, 0x00, 0x8b),
        emissive: true,
    },
];

#[derive(Component)]
pub(crate) struct WallColorCamera;

#[derive(Component)]
pub(crate) struct WallColorPatch {
    ordinal: usize,
}

#[derive(Resource)]
pub(crate) struct WallColorActualWindowAcceptance {
    requested: bool,
    status_path: Option<PathBuf>,
    ack_path: Option<PathBuf>,
    session_nonce: Option<String>,
    started_at: Option<Duration>,
    published_at: Option<Duration>,
    settled_frames: u32,
    board_spawned: bool,
    completed: bool,
    failed: bool,
}

impl Default for WallColorActualWindowAcceptance {
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
            board_spawned: false,
            completed: false,
            failed: false,
        }
    }
}

impl WallColorActualWindowAcceptance {
    pub(crate) fn requested_from_environment() -> bool {
        std::env::var(ACCEPTANCE_ENV).is_ok_and(|value| value == "1")
    }

    fn enabled(&self, config: &PerfScenarioConfig) -> bool {
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

pub(crate) fn setup_wall_color_board_system(
    mut commands: Commands,
    config: Res<PerfScenarioConfig>,
    main_camera: Query<Entity, (With<Camera2d>, With<MainCamera>)>,
    mut acceptance: ResMut<WallColorActualWindowAcceptance>,
) {
    if !acceptance.enabled(&config) || acceptance.board_spawned {
        return;
    }
    let Ok(main_camera) = main_camera.single() else {
        let failure = failure_status(
            &acceptance,
            "wall-color setup requires one main Camera2d for UI isolation",
        );
        if let Some(path) = acceptance.status_path.as_deref()
            && let Err(error) = write_status(path, &failure)
        {
            eprintln!("PERF_WALL_COLOR: cannot write setup failure: {error}");
        }
        acceptance.failed = true;
        return;
    };
    // Bevy 0.19 otherwise assigns untargeted UI to the highest-order camera.
    // Keep gameplay UI on its established main camera while the final camera
    // clears and renders only the calibration board.
    commands.entity(main_camera).insert(IsDefaultUiCamera);
    commands.spawn((
        Camera2d,
        Camera {
            order: CAMERA_ORDER,
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            ..default()
        },
        RenderLayers::layer(RENDER_LAYER),
        WallColorCamera,
    ));
    for (ordinal, patch) in PATCHES.iter().enumerate() {
        let color = patch_color(*patch);
        let x = patch.center_px.0 as f32 - BOARD_WIDTH as f32 / 2.0;
        let y = BOARD_HEIGHT as f32 / 2.0 - patch.center_px.1 as f32;
        commands.spawn((
            Sprite::from_color(color, Vec2::splat(PATCH_SIZE)),
            Transform::from_xyz(x, y, 0.0),
            RenderLayers::layer(RENDER_LAYER),
            WallColorPatch { ordinal },
        ));
    }
    acceptance.board_spawned = true;
}

type PatchQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static WallColorPatch,
        &'static Sprite,
        &'static Transform,
        &'static Visibility,
        &'static InheritedVisibility,
    ),
>;
type DefaultUiCameraQuery<'w, 's> =
    Query<'w, 's, Entity, (With<Camera2d>, With<MainCamera>, With<IsDefaultUiCamera>)>;

#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct WallColorActualWindowParams<'w, 's> {
    window: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    camera: Query<'w, 's, (&'static Camera, &'static RenderLayers), With<WallColorCamera>>,
    default_ui_camera: DefaultUiCameraQuery<'w, 's>,
    patches: PatchQuery<'w, 's>,
}

pub(crate) fn publish_wall_color_actual_window_status_system(
    config: Res<PerfScenarioConfig>,
    params: WallColorActualWindowParams,
    time: Res<Time<Real>>,
    mut acceptance: ResMut<WallColorActualWindowAcceptance>,
) {
    if !acceptance.enabled(&config) || !acceptance.board_spawned {
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
            "timed out waiting for wall-color acknowledgement",
        );
        if let Some(path) = acceptance.status_path.as_deref()
            && let Err(error) = write_status(path, &failure)
        {
            eprintln!("PERF_WALL_COLOR: cannot write timeout status: {error}");
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
    let status = match build_status(&params, &acceptance) {
        Ok(status) => status,
        Err(reason) => failure_status(&acceptance, &reason),
    };
    if let Err(error) = write_status(&path, &status) {
        eprintln!("PERF_WALL_COLOR: cannot write probe status: {error}");
        return;
    }
    if status.get("status").and_then(Value::as_str) == Some("ready") {
        acceptance.published_at = Some(now);
    } else {
        acceptance.failed = true;
    }
}

fn build_status(
    params: &WallColorActualWindowParams,
    acceptance: &WallColorActualWindowAcceptance,
) -> Result<Value, String> {
    let nonce = acceptance
        .session_nonce
        .as_deref()
        .ok_or_else(|| "wall-color session nonce is absent".to_string())?;
    let window = params
        .window
        .single()
        .map_err(|_| "wall-color probe requires one primary Window".to_string())?;
    let physical_width = window.physical_width();
    let physical_height = window.physical_height();
    if physical_width != 1280 || physical_height != 720 || window.scale_factor() != 1.0 {
        return Err("wall-color client dimensions differ from 1280x720 at scale 1".to_string());
    }
    let (camera, layers) = params
        .camera
        .single()
        .map_err(|_| "wall-color probe requires one dedicated Camera2d".to_string())?;
    if camera.order != CAMERA_ORDER || *layers != RenderLayers::layer(RENDER_LAYER) {
        return Err("wall-color camera contract differs".to_string());
    }
    params
        .default_ui_camera
        .single()
        .map_err(|_| "wall-color UI is not isolated on one main Camera2d".to_string())?;
    let mut seen = [false; PATCHES.len()];
    for (marker, sprite, transform, visibility, inherited_visibility) in &params.patches {
        let Some(spec) = PATCHES.get(marker.ordinal) else {
            return Err("wall-color patch ordinal is outside the contract".to_string());
        };
        if seen[marker.ordinal] {
            return Err("wall-color patch ordinal is duplicated".to_string());
        }
        seen[marker.ordinal] = true;
        if *visibility == Visibility::Hidden || !inherited_visibility.get() {
            return Err(format!("wall-color patch {} is hidden", spec.id));
        }
        if sprite.custom_size != Some(Vec2::splat(PATCH_SIZE))
            || sprite.color != patch_color(*spec)
            || transform.translation
                != Vec3::new(
                    spec.center_px.0 as f32 - BOARD_WIDTH as f32 / 2.0,
                    BOARD_HEIGHT as f32 / 2.0 - spec.center_px.1 as f32,
                    0.0,
                )
        {
            return Err(format!("wall-color patch {} differs", spec.id));
        }
    }
    if !seen.into_iter().all(|present| present) {
        return Err("wall-color patch inventory is incomplete".to_string());
    }
    let board_x = (physical_width - BOARD_WIDTH) / 2;
    let board_y = (physical_height - BOARD_HEIGHT) / 2;
    Ok(json!({
        "schema_version": STATUS_SCHEMA_VERSION,
        "status": "ready",
        "session_nonce": nonce,
        "phase": PHASE_ID,
        "generation": GENERATION,
        "fixture": {
            "contract_id": CONTRACT_ID,
            "contract_sha256": CONTRACT_SHA256,
            "board_roi": {"x": board_x, "y": board_y, "width": BOARD_WIDTH, "height": BOARD_HEIGHT},
            "patch_size_px": PATCH_SIZE as u32,
            "patches": PATCHES.iter().map(patch_status).collect::<Vec<_>>(),
        },
        "window": {
            "physical_width": physical_width,
            "physical_height": physical_height,
            "scale_factor": window.scale_factor(),
        },
        "render": {
            "backend": "vulkan",
            "camera": "dedicated-final-camera2d",
            "camera_order": CAMERA_ORDER,
            "render_layer": RENDER_LAYER,
            "ui_target": "main-camera",
            "base_path": "sprite-unlit-srgb",
            "emissive_path": "sprite-linear-multiplier",
            "emissive_strength": EMISSIVE_STRENGTH,
        },
    }))
}

fn patch_status(spec: &PatchSpec) -> Value {
    json!({
        "id": spec.id,
        "center_px": [spec.center_px.0, spec.center_px.1],
        "input_hex_srgb": format!("#{:02x}{:02x}{:02x}", spec.input_rgb.0, spec.input_rgb.1, spec.input_rgb.2),
        "emissive": spec.emissive,
    })
}

fn patch_color(spec: PatchSpec) -> Color {
    let base = Color::srgb_u8(spec.input_rgb.0, spec.input_rgb.1, spec.input_rgb.2);
    if !spec.emissive {
        return base;
    }
    let linear = base.to_linear();
    Color::from(LinearRgba::new(
        linear.red * EMISSIVE_STRENGTH,
        linear.green * EMISSIVE_STRENGTH,
        linear.blue * EMISSIVE_STRENGTH,
        linear.alpha,
    ))
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

fn failure_status(acceptance: &WallColorActualWindowAcceptance, reason: &str) -> Value {
    json!({
        "schema_version": STATUS_SCHEMA_VERSION,
        "status": "failed",
        "session_nonce": acceptance.session_nonce.as_deref().unwrap_or(""),
        "phase": PHASE_ID,
        "generation": GENERATION,
        "reason": reason,
    })
}

fn write_status(path: &Path, value: &Value) -> std::io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("wall-color status path has no parent"))?;
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
    fn embedded_contract_hash_is_current() {
        let actual = format!("{:x}", Sha256::digest(CONTRACT_BYTES));
        assert_eq!(actual, CONTRACT_SHA256);
    }

    #[test]
    fn patches_match_the_sealed_board_layout() {
        assert_eq!(PATCHES.len(), 5);
        assert_eq!(PATCHES[0].center_px, (32, 32));
        assert_eq!(PATCHES[4].center_px, (288, 32));
        assert!(PATCHES.iter().take(4).all(|patch| !patch.emissive));
        assert!(PATCHES[4].emissive);
        assert_eq!(PATCHES[3].input_rgb, PATCHES[4].input_rgb);
    }

    #[test]
    fn emissive_patch_increases_linear_luminance() {
        let base = patch_color(PATCHES[3]).to_linear();
        let emissive = patch_color(PATCHES[4]).to_linear();
        assert!((emissive.red - base.red * EMISSIVE_STRENGTH).abs() < f32::EPSILON);
        assert!(emissive.red > base.red);
        assert_eq!(emissive.green, 0.0);
        assert!(emissive.blue > base.blue);
    }

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
        let mut wrong_phase = valid.clone();
        wrong_phase["phase"] = json!("current-wall");
        assert!(!acknowledgement_matches(&wrong_phase, nonce));
    }
}
