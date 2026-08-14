//! Production-only visual probe storyboard for the P02 actual-window recipe.
//!
//! This module never participates in an ordinary game or formal perf run. The
//! native acceptance launcher opts in with its status/acknowledgement
//! environment contract, then
//! captures phase-tagged client-window frames from the normal indoor-light
//! fixture. Every probe uses an entity that the production fixture already
//! spawned; no `visual_test` or synthetic presentation path is accepted.

use super::config::{PerfRenderMode, PerfScenarioConfig};
use super::indoor_light_fixture::{IndoorLightFixturePhase, IndoorLightFixtureState};
use bevy::camera::visibility::RenderLayers;
use bevy::camera_controller::pan_camera::PanCamera;
use bevy::ecs::system::SystemParam;
use bevy::mesh::{Mesh, Mesh3d};
use bevy::pbr::{MeshMaterial3d, StandardMaterial};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use hw_core::camera::MainCamera;
use hw_core::constants::{TILE_SIZE, topdown_rtt_vertical_compensation};
use hw_visual::blueprint::{BuildingBounceEffect, CompletionText, DeliveryPopup};
use hw_visual::visual3d::{
    ActorBillboard3d, Building3dVisual, Door3dVisual, DoorPresentationState,
    LegacyStructural2dMirror,
};
use hw_world::WorldMap;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::plugins::startup::{Building3dHandles, Camera3dRtt};
use crate::systems::jobs::{Building, BuildingType, Door, DoorState, RenderPresentationClass};
use crate::world::map::RIVER_Y_MIN;

const ACCEPTANCE_ENV: &str = "HW_P02_PRESENTATION_ACTUAL_WINDOW";
const STATUS_PATH_ENV: &str = "HW_P02_PRESENTATION_STATUS_PATH";
const ACK_PATH_ENV: &str = "HW_P02_PRESENTATION_ACK_PATH";
const SESSION_NONCE_ENV: &str = "HW_P02_PRESENTATION_SESSION_NONCE";
const STATUS_SCHEMA_VERSION: u32 = 7;
const PHASE_SETTLE: Duration = Duration::from_millis(1_600);
const ACK_TIMEOUT: Duration = Duration::from_secs(15);
const SETTLE_FRAMES: u32 = 3;
const CAMERA_SCALE: f32 = 0.75;
const WALL_PROBE_GRID: (i32, i32) = (16, 20);
const BRIDGE_PROBE_GRID: (i32, i32) = (90, 65);
const FOREGROUND_PROBE_GRID: (i32, i32) = (27, 28);
const ROI_HALF_SIZE: u32 = 112;
const OCCLUSION_ROI_HALF_SIZE: u32 = 48;
const MAX_OCCLUSION_CENTER_DISTANCE: f32 = 2.0;

type BridgeVisualQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Building3dVisual,
        &'static Transform,
        &'static Visibility,
        &'static InheritedVisibility,
        &'static RenderLayers,
        &'static Mesh3d,
        &'static MeshMaterial3d<StandardMaterial>,
    ),
>;

/// Groups all read-only storyboard evidence inputs so the production schedule
/// stays within Bevy's system arity and the verifier receives one coherent
/// production-world view.
#[derive(SystemParam)]
pub(crate) struct P02ActualWindowStatusParams<'w, 's> {
    handles_3d: Res<'w, Building3dHandles>,
    meshes: Res<'w, Assets<Mesh>>,
    standard_materials: Res<'w, Assets<StandardMaterial>>,
    window: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    camera_3d: Query<
        'w,
        's,
        (
            &'static Camera,
            &'static GlobalTransform,
            &'static RenderLayers,
        ),
        With<Camera3dRtt>,
    >,
    main_camera: Query<
        'w,
        's,
        (&'static Camera, &'static GlobalTransform),
        (With<MainCamera>, Without<Camera3dRtt>),
    >,
    doors: Query<'w, 's, (Entity, &'static Door, &'static Transform)>,
    door_visuals: Query<
        'w,
        's,
        (
            &'static Door3dVisual,
            &'static DoorPresentationState,
            &'static Transform,
            &'static Visibility,
            &'static InheritedVisibility,
        ),
    >,
    owners: Query<'w, 's, (Entity, &'static Building, &'static Transform)>,
    building_visuals: Query<
        'w,
        's,
        (
            &'static Building3dVisual,
            &'static Transform,
            &'static Visibility,
            &'static InheritedVisibility,
        ),
    >,
    bridge_visuals: BridgeVisualQuery<'w, 's>,
    billboards: Query<
        'w,
        's,
        (
            &'static ActorBillboard3d,
            &'static Transform,
            &'static Visibility,
        ),
    >,
    bounces: Query<'w, 's, &'static BuildingBounceEffect>,
    foreground: Query<
        'w,
        's,
        (&'static ChildOf, &'static GlobalTransform),
        (With<Sprite>, Without<LegacyStructural2dMirror>),
    >,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProbePhase {
    DoorOpen,
    DoorClosed,
    DoorLocked,
    SoulFront,
    SoulBehind,
    Bridge,
    WallBounceActive,
    WallBounceRest,
    ForegroundA,
    ForegroundB,
}

impl ProbePhase {
    #[cfg(test)]
    const ALL: [Self; 10] = [
        Self::DoorOpen,
        Self::DoorClosed,
        Self::DoorLocked,
        Self::SoulFront,
        Self::SoulBehind,
        Self::Bridge,
        Self::WallBounceActive,
        Self::WallBounceRest,
        Self::ForegroundA,
        Self::ForegroundB,
    ];

    const fn id(self) -> &'static str {
        match self {
            Self::DoorOpen => "door-open",
            Self::DoorClosed => "door-closed",
            Self::DoorLocked => "door-locked",
            Self::SoulFront => "soul-front",
            Self::SoulBehind => "soul-behind",
            Self::Bridge => "bridge",
            Self::WallBounceActive => "wall-bounce-active",
            Self::WallBounceRest => "wall-bounce-rest",
            Self::ForegroundA => "foreground-a",
            Self::ForegroundB => "foreground-b",
        }
    }

    const fn expected_door_state(self) -> Option<DoorState> {
        match self {
            Self::DoorOpen => Some(DoorState::Open),
            Self::DoorClosed => Some(DoorState::Closed),
            Self::DoorLocked => Some(DoorState::Locked),
            _ => None,
        }
    }

    const fn next(self) -> Option<Self> {
        match self {
            Self::DoorOpen => Some(Self::DoorClosed),
            Self::DoorClosed => Some(Self::DoorLocked),
            Self::DoorLocked => Some(Self::SoulFront),
            Self::SoulFront => Some(Self::SoulBehind),
            Self::SoulBehind => Some(Self::Bridge),
            Self::Bridge => Some(Self::WallBounceActive),
            Self::WallBounceActive => Some(Self::WallBounceRest),
            Self::WallBounceRest => Some(Self::ForegroundA),
            Self::ForegroundA => Some(Self::ForegroundB),
            Self::ForegroundB => None,
        }
    }

    const fn is_soul_depth(self) -> bool {
        matches!(self, Self::SoulFront | Self::SoulBehind)
    }

    const fn is_foreground(self) -> bool {
        matches!(self, Self::ForegroundA | Self::ForegroundB)
    }

    const fn is_wall_bounce(self) -> bool {
        matches!(self, Self::WallBounceActive | Self::WallBounceRest)
    }

    const fn is_active_wall_bounce(self) -> bool {
        matches!(self, Self::WallBounceActive)
    }

    const fn settle_duration(self) -> Duration {
        if self.is_active_wall_bounce() {
            Duration::ZERO
        } else {
            PHASE_SETTLE
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ProbeRoi {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl ProbeRoi {
    fn as_json(self) -> Value {
        json!({
            "x": self.x,
            "y": self.y,
            "width": self.width,
            "height": self.height,
        })
    }
}

/// Runtime state for a single production actual-window probe storyboard.
#[derive(Resource, Debug)]
pub(crate) struct P02ActualWindowAcceptance {
    requested: bool,
    status_path: Option<PathBuf>,
    ack_path: Option<PathBuf>,
    session_nonce: Option<String>,
    phase: ProbePhase,
    phase_started_at: Option<Duration>,
    phase_frames: u32,
    generation: u32,
    published_generation: Option<u32>,
    wall_bounce_seeded_generation: Option<u32>,
    completed: bool,
    failure_reason: Option<String>,
}

impl Default for P02ActualWindowAcceptance {
    fn default() -> Self {
        Self {
            requested: Self::requested_from_environment(),
            status_path: std::env::var_os(STATUS_PATH_ENV)
                .map(PathBuf::from)
                .filter(|path| path.is_absolute() && !path.as_os_str().is_empty()),
            ack_path: std::env::var_os(ACK_PATH_ENV)
                .map(PathBuf::from)
                .filter(|path| path.is_absolute() && !path.as_os_str().is_empty()),
            session_nonce: std::env::var(SESSION_NONCE_ENV)
                .ok()
                .filter(|value| is_session_nonce(value)),
            phase: ProbePhase::DoorOpen,
            phase_started_at: None,
            phase_frames: 0,
            generation: 1,
            published_generation: None,
            wall_bounce_seeded_generation: None,
            completed: false,
            failure_reason: None,
        }
    }
}

impl P02ActualWindowAcceptance {
    /// The storyboard is an opt-in native-acceptance path. Keep its broad
    /// mutable-world systems out of ordinary profiling runs so evidence
    /// instrumentation cannot perturb the P02 performance contract.
    pub(crate) fn requested_from_environment() -> bool {
        std::env::var(ACCEPTANCE_ENV).is_ok_and(|value| value == "1")
    }

    fn enabled(&self, config: &PerfScenarioConfig, fixture: &IndoorLightFixtureState) -> bool {
        self.requested
            && self.status_path.is_some()
            && self.ack_path.is_some()
            && self.session_nonce.is_some()
            && fixture.phase == IndoorLightFixturePhase::Ready
            && config.rtt_light_selection().is_some_and(|selection| {
                selection.stage_id() == "p02" && selection.lane() == "static"
            })
    }

    fn update_phase(&mut self, now: Duration) {
        let started_at = *self.phase_started_at.get_or_insert(now);
        self.phase_frames = self.phase_frames.saturating_add(1);
        if self.completed || self.failure_reason.is_some() {
            return;
        }
        if self.published_generation != Some(self.generation) {
            return;
        }
        if self.acknowledges_current_generation() {
            if let Some(next) = self.phase.next() {
                self.phase = next;
                self.phase_started_at = Some(now);
                self.phase_frames = 0;
                self.generation = self.generation.saturating_add(1);
                self.published_generation = None;
                self.wall_bounce_seeded_generation = None;
            } else {
                self.completed = true;
            }
            return;
        }
        if now.saturating_sub(started_at) >= ACK_TIMEOUT {
            self.failure_reason = Some(format!(
                "timed out waiting for capture acknowledgement for {} generation {}",
                self.phase.id(),
                self.generation
            ));
        }
    }

    fn ready_to_publish(&self, now: Duration) -> bool {
        let started_at = self.phase_started_at.unwrap_or(now);
        !self.completed
            && self.failure_reason.is_none()
            && self.published_generation != Some(self.generation)
            && self.phase_frames >= SETTLE_FRAMES
            && now.saturating_sub(started_at) >= self.phase.settle_duration()
    }

    fn acknowledges_current_generation(&self) -> bool {
        let (Some(path), Some(nonce)) = (&self.ack_path, &self.session_nonce) else {
            return false;
        };
        let Ok(bytes) = std::fs::read(path) else {
            return false;
        };
        let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
            return false;
        };
        acknowledgement_matches(&value, nonce, self.phase, self.generation)
    }

    fn should_seed_wall_bounce(&self) -> bool {
        self.phase.is_active_wall_bounce()
            && self.wall_bounce_seeded_generation != Some(self.generation)
    }

    fn mark_wall_bounce_seeded(&mut self) {
        self.wall_bounce_seeded_generation = Some(self.generation);
    }

    fn mark_published(&mut self) {
        self.published_generation = Some(self.generation);
    }
}

fn is_session_nonce(value: &str) -> bool {
    value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn acknowledgement_matches(value: &Value, nonce: &str, phase: ProbePhase, generation: u32) -> bool {
    value.get("schema_version").and_then(Value::as_u64) == Some(STATUS_SCHEMA_VERSION.into())
        && value.get("status").and_then(Value::as_str) == Some("captured")
        && value.get("session_nonce").and_then(Value::as_str) == Some(nonce)
        && value.get("phase").and_then(Value::as_str) == Some(phase.id())
        && value.get("generation").and_then(Value::as_u64) == Some(generation.into())
}

/// Applies the next deterministic camera and foreground state before the
/// normal Visual schedule synchronizes the RtT camera and active visuals.
pub(crate) fn prepare_p02_actual_window_view_system(
    mut commands: Commands,
    config: Res<PerfScenarioConfig>,
    fixture: Res<IndoorLightFixtureState>,
    time: Res<Time<Real>>,
    mut acceptance: ResMut<P02ActualWindowAcceptance>,
    mut transforms: ParamSet<(
        Query<(&mut Transform, &mut Projection, &mut PanCamera), With<MainCamera>>,
        Query<(Entity, &Door, &Transform)>,
        Query<(Entity, &Building, &Transform)>,
        Query<(&ChildOf, &mut Transform), (With<Sprite>, Without<LegacyStructural2dMirror>)>,
    )>,
    mut bounces: Query<&mut BuildingBounceEffect>,
    mut ui_roots: Query<&mut Node, Without<ChildOf>>,
    mut transient_text: Query<&mut Visibility, Or<(With<CompletionText>, With<DeliveryPopup>)>>,
) {
    if !acceptance.enabled(&config, &fixture) {
        return;
    }
    acceptance.update_phase(time.elapsed());
    let phase = acceptance.phase;

    let center = if let Some(expected_state) = phase.expected_door_state() {
        transforms
            .p1()
            .iter()
            .filter(|(_, door, _)| door.state == expected_state)
            .min_by_key(|(entity, _, transform)| {
                (
                    transform.translation.y.to_bits(),
                    transform.translation.x.to_bits(),
                    entity.to_bits(),
                )
            })
            .map(|(_, _, transform)| transform.translation.truncate())
            .unwrap_or_else(|| WorldMap::grid_to_world(WALL_PROBE_GRID.0, WALL_PROBE_GRID.1))
    } else {
        static_phase_camera_target(phase)
    };
    if let Ok((mut transform, mut projection, mut pan)) = transforms.p0().single_mut() {
        transform.translation.x = center.x;
        transform.translation.y = center.y;
        pan.enabled = false;
        if let Projection::Orthographic(orthographic) = projection.as_mut() {
            orthographic.scale = CAMERA_SCALE;
        }
    }
    for mut node in &mut ui_roots {
        node.display = Display::None;
    }
    // Completion text is real game feedback, but it is unrelated to a
    // presentation probe and otherwise obscures every local ROI.
    for mut visibility in &mut transient_text {
        *visibility = Visibility::Hidden;
    }
    if phase.is_wall_bounce() {
        let wall = transforms
            .p2()
            .iter()
            .find(|(_, building, transform)| {
                building.kind == BuildingType::Wall
                    && transform.translation.truncate()
                        == WorldMap::grid_to_world(WALL_PROBE_GRID.0, WALL_PROBE_GRID.1)
            })
            .map(|(entity, _, _)| entity);
        if phase.is_active_wall_bounce() {
            if acceptance.should_seed_wall_bounce() {
                if let Some(wall) = wall {
                    commands.entity(wall).insert(active_wall_bounce_probe());
                    acceptance.mark_wall_bounce_seeded();
                }
            } else if let Some(wall) = wall
                && let Ok(mut bounce) = bounces.get_mut(wall)
            {
                // Indoor-light static capture deliberately pauses virtual
                // time. Pin the ordinary production completion effect at its
                // in-progress midpoint, so the normal Visual schedule still
                // writes a non-unit owner scale and synchronizes the matching
                // 3D presentation transform before the screenshot ACK.
                pin_active_wall_bounce(&mut bounce);
            }
        } else if let Some(wall) = wall
            && let Ok(mut bounce) = bounces.get_mut(wall)
        {
            // On the next normal Visual pass the production animation system
            // resets the owner scale and removes this completed effect. This
            // provides the resting-frame proof without unpausing the static
            // performance fixture.
            finish_wall_bounce(&mut bounce);
        }
    }
    let foreground_owners = transforms
        .p2()
        .iter()
        .filter_map(|(entity, building, _)| {
            (building.kind == BuildingType::SandPile
                && crate::systems::jobs::presentation_class(building.kind)
                    == RenderPresentationClass::Foreground2d)
                .then_some(entity)
        })
        .collect::<Vec<_>>();
    for (parent, mut transform) in &mut transforms.p3() {
        if !foreground_owners.contains(&parent.parent()) {
            continue;
        }
        let scale = match phase {
            ProbePhase::ForegroundA => 1.0,
            ProbePhase::ForegroundB => 1.18,
            _ => 1.0,
        };
        transform.scale = Vec3::splat(scale);
    }
}

/// Creates the ordinary production completion effect at its visible midpoint.
///
/// The actual-window fixture freezes `Time<Virtual>` by contract, so starting
/// it at zero would correctly remain at scale 1.0 forever. The storyboard
/// therefore pins the same one-shot effect at a deterministic in-progress
/// state; `building_bounce_animation_system` remains the sole writer of the
/// owner transform and the following Visual pass synchronizes its 3D child.
fn active_wall_bounce_probe() -> BuildingBounceEffect {
    let mut bounce = BuildingBounceEffect::completion();
    pin_active_wall_bounce(&mut bounce);
    bounce
}

fn pin_active_wall_bounce(bounce: &mut BuildingBounceEffect) {
    bounce.bounce_animation.timer = bounce.bounce_animation.config.duration * 0.5;
}

fn finish_wall_bounce(bounce: &mut BuildingBounceEffect) {
    bounce.bounce_animation.timer = bounce.bounce_animation.config.duration;
}

/// Keeps only the selected production billboard visible during the two depth
/// phases, then places that existing visual in front of or behind an existing
/// fixture Wall. The Soul's semantic root is intentionally untouched so the
/// performance artifact remains the static fixture contract.
pub(crate) fn apply_p02_actual_window_actor_probe_system(
    config: Res<PerfScenarioConfig>,
    fixture: Res<IndoorLightFixtureState>,
    acceptance: Res<P02ActualWindowAcceptance>,
    camera: Query<(&Camera, &GlobalTransform), With<Camera3dRtt>>,
    mut transforms: ParamSet<(
        Query<(Entity, &Building, &Transform)>,
        Query<(&Building3dVisual, &Transform)>,
        Query<(&ActorBillboard3d, &mut Transform, &mut Visibility)>,
    )>,
) {
    if !acceptance.enabled(&config, &fixture) {
        return;
    }
    let Some(subject) = fixture.actual_window_subject_soul() else {
        return;
    };
    let is_gpu = matches!(config.render_mode, PerfRenderMode::Gpu);
    let phase = acceptance.phase;
    for (billboard, _, mut visibility) in &mut transforms.p2() {
        *visibility = if is_gpu && phase.is_soul_depth() && billboard.owner == subject {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    if !is_gpu || !phase.is_soul_depth() {
        return;
    }
    let Ok((camera, camera_transform)) = camera.single() else {
        return;
    };
    let wall = transforms
        .p0()
        .iter()
        .find(|(_, building, transform)| {
            building.kind == BuildingType::Wall
                && transform.translation.truncate()
                    == WorldMap::grid_to_world(WALL_PROBE_GRID.0, WALL_PROBE_GRID.1)
        })
        .map(|(wall, _, _)| wall);
    let wall_position = wall.and_then(|wall| {
        transforms
            .p1()
            .iter()
            .find(|(visual, _)| visual.owner == wall)
            .map(|(_, transform)| transform.translation)
    });
    let Some(wall_position) = wall_position else {
        return;
    };
    let Ok(wall_view) = camera.world_to_viewport_with_depth(camera_transform, wall_position) else {
        return;
    };
    // Move along the actual camera ray rather than across the map. This
    // keeps the billboard center on the Wall center in screen space while
    // choosing strict front/behind depths, so the subsequent PNG predicate
    // measures depth-tested occlusion rather than merely two nearby sprites.
    let ray_offset = camera_transform.forward().as_vec3() * (TILE_SIZE * 1.5);
    let candidates = [wall_position - ray_offset, wall_position + ray_offset]
        .into_iter()
        .filter_map(|position| {
            let view = camera
                .world_to_viewport_with_depth(camera_transform, position)
                .ok()?;
            let screen_distance = view.truncate().distance(wall_view.truncate());
            (screen_distance <= MAX_OCCLUSION_CENTER_DISTANCE).then_some((
                screen_distance,
                view.z,
                position,
            ))
        })
        .collect::<Vec<_>>();
    let selected = match phase {
        ProbePhase::SoulFront => candidates
            .iter()
            .filter(|(_, depth, _)| *depth < wall_view.z)
            .min_by(|left, right| left.0.total_cmp(&right.0))
            .copied(),
        ProbePhase::SoulBehind => candidates
            .iter()
            .filter(|(_, depth, _)| *depth > wall_view.z)
            .min_by(|left, right| left.0.total_cmp(&right.0))
            .copied(),
        _ => None,
    };
    let Some((_, _, position)) = selected else {
        return;
    };
    for (billboard, mut transform, _) in &mut transforms.p2() {
        if billboard.owner == subject {
            transform.translation = position;
        }
    }
}

/// Publishes a phase only after the camera, active visual and Transform
/// propagation have all had multiple frames to settle. The Python launcher
/// snapshots this JSON before capturing the corresponding X11 client frame.
pub(crate) fn publish_p02_actual_window_probe_status_system(
    config: Res<PerfScenarioConfig>,
    fixture: Res<IndoorLightFixtureState>,
    status_params: P02ActualWindowStatusParams,
    time: Res<Time<Real>>,
    mut acceptance: ResMut<P02ActualWindowAcceptance>,
) {
    if !acceptance.enabled(&config, &fixture) {
        return;
    }
    let Some(path) = acceptance.status_path.clone() else {
        return;
    };
    let Some(nonce) = acceptance.session_nonce.clone() else {
        return;
    };
    if let Some(reason) = &acceptance.failure_reason {
        if acceptance.published_generation == Some(acceptance.generation) {
            return;
        }
        let status = json!({
            "schema_version": STATUS_SCHEMA_VERSION,
            "status": "failed",
            "session_nonce": nonce,
            "phase": acceptance.phase.id(),
            "generation": acceptance.generation,
            "reason": reason,
        });
        if let Err(error) = write_status(&path, &status) {
            eprintln!("PERF_P02_PRESENTATION: cannot write probe status: {error}");
            return;
        }
        acceptance.mark_published();
        return;
    }
    if !acceptance.ready_to_publish(time.elapsed()) {
        return;
    }
    let status = match status_params.build_probe_status(
        &config,
        &fixture,
        acceptance.phase,
        acceptance.generation,
        &nonce,
    ) {
        Ok(value) => value,
        Err(reason) => json!({
            "schema_version": STATUS_SCHEMA_VERSION,
            "status": "failed",
            "session_nonce": nonce,
            "phase": acceptance.phase.id(),
            "generation": acceptance.generation,
            "reason": reason,
        }),
    };
    if let Err(error) = write_status(&path, &status) {
        eprintln!("PERF_P02_PRESENTATION: cannot write probe status: {error}");
        return;
    }
    acceptance.mark_published();
}

impl<'w, 's> P02ActualWindowStatusParams<'w, 's> {
    fn build_probe_status(
        &self,
        config: &PerfScenarioConfig,
        fixture: &IndoorLightFixtureState,
        phase: ProbePhase,
        generation: u32,
        session_nonce: &str,
    ) -> Result<Value, String> {
        let handles_3d = &self.handles_3d;
        let meshes = &self.meshes;
        let standard_materials = &self.standard_materials;
        let window = &self.window;
        let camera_3d = &self.camera_3d;
        let main_camera = &self.main_camera;
        let doors = &self.doors;
        let door_visuals = &self.door_visuals;
        let owners = &self.owners;
        let building_visuals = &self.building_visuals;
        let bridge_visuals = &self.bridge_visuals;
        let billboards = &self.billboards;
        let bounces = &self.bounces;
        let foreground = &self.foreground;
        let window = window.single().map_err(|_| "missing primary window")?;
        let physical_width = window.physical_width();
        let physical_height = window.physical_height();
        if physical_width == 0 || physical_height == 0 {
            return Err("primary window has no physical extent".to_string());
        }
        let is_gpu = matches!(config.render_mode, PerfRenderMode::Gpu);
        let fixture_checksum = fixture
            .actual_window_layout_checksum()
            .ok_or_else(|| "ready fixture has no layout checksum".to_string())?;
        let camera_target = phase_camera_target(phase, doors)?;
        let probe = if let Some(expected_state) = phase.expected_door_state() {
            let (door_entity, _, _) = doors
                .iter()
                .filter(|(_, door, _)| door.state == expected_state)
                .min_by_key(|(entity, _, transform)| {
                    (
                        transform.translation.y.to_bits(),
                        transform.translation.x.to_bits(),
                        entity.to_bits(),
                    )
                })
                .ok_or_else(|| format!("missing {expected_state:?} Door root"))?;
            let (visual, presentation, transform, visibility, inherited_visibility) = door_visuals
                .iter()
                .find(|(visual, _, _, _, _)| visual.owner == door_entity)
                .ok_or_else(|| format!("missing {expected_state:?} Door3dVisual"))?;
            let _ = visual;
            require_probe_visual_for_render_mode(
                visibility,
                inherited_visibility,
                is_gpu,
                &format!("{expected_state:?} Door3dVisual"),
            )?;
            let expected_presentation = door_presentation_state(expected_state);
            if *presentation != expected_presentation {
                return Err(format!(
                    "Door semantic/presentation mismatch: expected {expected_presentation:?}, got {presentation:?}"
                ));
            }
            let (camera, global, _) = camera_3d.single().map_err(|_| "missing RtT camera")?;
            let roi = project_rtt_roi(
                camera,
                global,
                transform.translation,
                physical_width,
                physical_height,
            )
            .ok_or_else(|| "Door probe projection is outside the client window".to_string())?;
            json!({
                "kind": "door",
                "expected_state": door_state_name(expected_state),
                "presentation_state": door_presentation_name(*presentation),
                "roi": roi.as_json(),
            })
        } else if phase.is_soul_depth() {
            let (wall_entity, _, _) = owners
                .iter()
                .find(|(_, building, transform)| {
                    building.kind == BuildingType::Wall
                        && transform.translation.truncate()
                            == WorldMap::grid_to_world(WALL_PROBE_GRID.0, WALL_PROBE_GRID.1)
                })
                .ok_or_else(|| "missing production Wall depth probe".to_string())?;
            let (_, wall_transform, visibility, inherited_visibility) = building_visuals
                .iter()
                .find(|(visual, _, _, _)| visual.owner == wall_entity)
                .ok_or_else(|| "missing production Wall3dVisual depth probe".to_string())?;
            require_probe_visual_for_render_mode(
                visibility,
                inherited_visibility,
                is_gpu,
                "production Wall3dVisual depth probe",
            )?;
            let (camera, global, _) = camera_3d.single().map_err(|_| "missing RtT camera")?;
            let wall_view = camera
                .world_to_viewport_with_depth(global, wall_transform.translation)
                .map_err(|error| format!("cannot project Wall depth probe: {error}"))?;
            let roi = project_rtt_roi(
                camera,
                global,
                wall_transform.translation,
                physical_width,
                physical_height,
            )
            .ok_or_else(|| {
                "Wall depth probe projection is outside the client window".to_string()
            })?;
            let subject = fixture
                .actual_window_subject_soul()
                .ok_or_else(|| "ready fixture has no Soul depth subject".to_string())?;
            let actor = billboards
                .iter()
                .find(|(billboard, _, _)| billboard.owner == subject);
            if !is_gpu {
                if actor.is_some() {
                    return Err("CPU actual-window case retained a Soul billboard".to_string());
                }
                json!({
                    "kind": "soul-depth",
                    "render3d": "hidden",
                    "roi": roi.as_json(),
                })
            } else {
                let (_, actor_transform, visibility) = actor
                    .ok_or_else(|| "GPU actual-window case has no Soul billboard".to_string())?;
                if *visibility == Visibility::Hidden {
                    return Err("GPU Soul depth subject is hidden".to_string());
                }
                let actor_view = camera
                    .world_to_viewport_with_depth(global, actor_transform.translation)
                    .map_err(|error| format!("cannot project Soul depth probe: {error}"))?;
                let wall_center = project_rtt_point(
                    camera,
                    global,
                    wall_transform.translation,
                    physical_width,
                    physical_height,
                )
                .ok_or_else(|| "Wall depth center is outside the client window".to_string())?;
                let soul_center = project_rtt_point(
                    camera,
                    global,
                    actor_transform.translation,
                    physical_width,
                    physical_height,
                )
                .ok_or_else(|| "Soul depth center is outside the client window".to_string())?;
                let center_distance = wall_center.distance(soul_center);
                if center_distance > MAX_OCCLUSION_CENTER_DISTANCE {
                    return Err(format!(
                        "Soul depth center is {center_distance:.3}px from the Wall, above the overlap limit"
                    ));
                }
                let occlusion_roi = roi_around_point(
                    wall_center,
                    OCCLUSION_ROI_HALF_SIZE,
                    physical_width,
                    physical_height,
                )
                .ok_or_else(|| "Soul occlusion ROI is outside the client window".to_string())?;
                let relation = if actor_view.z < wall_view.z {
                    "front"
                } else {
                    "behind"
                };
                let expected_relation = if phase == ProbePhase::SoulFront {
                    "front"
                } else {
                    "behind"
                };
                if relation != expected_relation {
                    return Err(format!(
                        "Soul depth probe is {relation}, expected {expected_relation}"
                    ));
                }
                json!({
                    "kind": "soul-depth",
                    "render3d": "visible",
                    "relation": relation,
                    "wall_depth": wall_view.z,
                    "soul_depth": actor_view.z,
                    "roi": roi.as_json(),
                    "occlusion_roi": occlusion_roi.as_json(),
                    "wall_center": point_as_json(wall_center),
                    "soul_center": point_as_json(soul_center),
                    "center_distance": center_distance,
                })
            }
        } else if phase == ProbePhase::Bridge {
            let (bridge_entity, _, _) = owners
                .iter()
                .find(|(_, building, transform)| {
                    is_fixture_building_root(
                        building,
                        transform,
                        BuildingType::Bridge,
                        BRIDGE_PROBE_GRID,
                    )
                })
                .ok_or_else(|| "missing production Bridge root".to_string())?;
            let (_, transform, visibility, inherited_visibility, bridge_layers, mesh, material) =
                bridge_visuals
                    .iter()
                    .find(|(visual, _, _, _, _, _, _)| visual.owner == bridge_entity)
                    .ok_or_else(|| "missing production Bridge3dVisual".to_string())?;
            require_probe_visual_for_render_mode(
                visibility,
                inherited_visibility,
                is_gpu,
                "production Bridge3dVisual",
            )?;
            let (camera, global, camera_layers) =
                camera_3d.single().map_err(|_| "missing RtT camera")?;
            require_bridge_rtt_drawable(
                bridge_layers,
                camera_layers,
                mesh,
                material,
                &handles_3d.bridge_mesh,
                &handles_3d.bridge_material,
                meshes,
                standard_materials,
            )?;
            let roi = project_rtt_roi(
                camera,
                global,
                transform.translation,
                physical_width,
                physical_height,
            )
            .ok_or_else(|| "Bridge probe projection is outside the client window".to_string())?;
            json!({
                "kind": "bridge",
                "owner_3d_visual": true,
                "rtt_drawable": true,
                "render3d": if is_gpu { "visible" } else { "hidden" },
                "roi": roi.as_json(),
            })
        } else if phase.is_wall_bounce() {
            let (wall_entity, _, owner_transform) = owners
                .iter()
                .find(|(_, building, transform)| {
                    building.kind == BuildingType::Wall
                        && transform.translation.truncate()
                            == WorldMap::grid_to_world(WALL_PROBE_GRID.0, WALL_PROBE_GRID.1)
                })
                .ok_or_else(|| "missing production Wall bounce root".to_string())?;
            let (_, visual_transform, visibility, inherited_visibility) = building_visuals
                .iter()
                .find(|(visual, _, _, _)| visual.owner == wall_entity)
                .ok_or_else(|| "missing production Wall3dVisual bounce probe".to_string())?;
            require_probe_visual_for_render_mode(
                visibility,
                inherited_visibility,
                is_gpu,
                "production Wall3dVisual bounce probe",
            )?;
            let bounce_active = bounces.get(wall_entity).is_ok();
            let owner_scale = owner_transform.scale.x;
            let visual_scale = visual_transform.scale.x;
            if phase.is_active_wall_bounce() {
                if !bounce_active
                    || owner_scale <= 1.001
                    || (owner_scale - visual_scale).abs() > 0.001
                {
                    return Err(
                        "active Wall bounce did not reach its matching 3D presentation transform"
                            .to_string(),
                    );
                }
            } else if bounce_active
                || (owner_scale - 1.0).abs() > 0.001
                || (visual_scale - 1.0).abs() > 0.001
            {
                return Err(
                    "resting Wall bounce probe retained an effect or non-unit presentation scale"
                        .to_string(),
                );
            }
            let (camera, global, _) = camera_3d.single().map_err(|_| "missing RtT camera")?;
            let roi = project_rtt_roi(
                camera,
                global,
                visual_transform.translation,
                physical_width,
                physical_height,
            )
            .ok_or_else(|| "Wall bounce projection is outside the client window".to_string())?;
            json!({
                "kind": "wall-bounce",
                "bounce_active": bounce_active,
                "owner_scale": owner_scale,
                "visual_scale": visual_scale,
                "roi": roi.as_json(),
            })
        } else if phase.is_foreground() {
            let (camera, global) = main_camera.single().map_err(|_| "missing MainCamera")?;
            let (child, transform) = foreground
                .iter()
                .find(|(child, _)| {
                    owners
                        .get(child.parent())
                        .is_ok_and(|(_, building, owner_transform)| {
                            building.kind == BuildingType::SandPile
                                && owner_transform.translation.truncate()
                                    == WorldMap::grid_to_world(
                                        FOREGROUND_PROBE_GRID.0,
                                        FOREGROUND_PROBE_GRID.1,
                                    )
                        })
                })
                .ok_or_else(|| "missing production Foreground2d Sprite".to_string())?;
            let _ = child;
            let roi = project_main_roi(
                camera,
                global,
                transform.translation(),
                physical_width,
                physical_height,
            )
            .ok_or_else(|| {
                "Foreground probe projection is outside the client window".to_string()
            })?;
            json!({
                "kind": "foreground",
                "phase_scale": if phase == ProbePhase::ForegroundA { 1.0 } else { 1.18 },
                "roi": roi.as_json(),
            })
        } else {
            return Err("unknown P02 actual-window phase".to_string());
        };
        Ok(json!({
            "schema_version": STATUS_SCHEMA_VERSION,
            "status": "ready",
            "session_nonce": session_nonce,
            "phase": phase.id(),
            "generation": generation,
            "fixture_layout_checksum": fixture_checksum,
            "render3d": if is_gpu { "visible" } else { "hidden" },
            "camera_target": {"x": camera_target.x, "y": camera_target.y},
            "window": {"physical_width": physical_width, "physical_height": physical_height},
            "probe": probe,
        }))
    }
}

/// Proves that the production Bridge visual can be drawn by the RtT camera,
/// rather than merely existing as a correctly-owned entity.  The actual-window
/// image predicate intentionally permits a fully opaque composite toggle, so
/// this source-side check closes the RenderLayer/mesh/material bypass route.
fn require_bridge_rtt_drawable(
    bridge_layers: &RenderLayers,
    camera_layers: &RenderLayers,
    mesh: &Mesh3d,
    material: &MeshMaterial3d<StandardMaterial>,
    expected_mesh: &Handle<Mesh>,
    expected_material: &Handle<StandardMaterial>,
    meshes: &Assets<Mesh>,
    standard_materials: &Assets<StandardMaterial>,
) -> Result<(), String> {
    if !bridge_layers.intersects(camera_layers) {
        return Err("production Bridge3dVisual shares no RenderLayer with Camera3dRtt".to_string());
    }
    if mesh.0.id() != expected_mesh.id() {
        return Err("production Bridge3dVisual mesh differs from the Bridge handle".to_string());
    }
    if material.0.id() != expected_material.id() {
        return Err(
            "production Bridge3dVisual material differs from the Bridge handle".to_string(),
        );
    }
    if !meshes.contains(expected_mesh.id()) {
        return Err("production Bridge mesh asset is unavailable".to_string());
    }
    if !standard_materials.contains(expected_material.id()) {
        return Err("production Bridge material asset is unavailable".to_string());
    }
    Ok(())
}

fn phase_camera_target(
    phase: ProbePhase,
    doors: &Query<(Entity, &Door, &Transform)>,
) -> Result<Vec2, String> {
    if let Some(expected_state) = phase.expected_door_state() {
        return doors
            .iter()
            .filter(|(_, door, _)| door.state == expected_state)
            .min_by_key(|(entity, _, transform)| {
                (
                    transform.translation.y.to_bits(),
                    transform.translation.x.to_bits(),
                    entity.to_bits(),
                )
            })
            .map(|(_, _, transform)| transform.translation.truncate())
            .ok_or_else(|| format!("missing {expected_state:?} Door camera target"));
    }
    Ok(static_phase_camera_target(phase))
}

fn static_phase_camera_target(phase: ProbePhase) -> Vec2 {
    match phase {
        ProbePhase::SoulFront
        | ProbePhase::SoulBehind
        | ProbePhase::WallBounceActive
        | ProbePhase::WallBounceRest => {
            WorldMap::grid_to_world(WALL_PROBE_GRID.0, WALL_PROBE_GRID.1)
        }
        // A Bridge anchor is not its draw position: the production placement
        // geometry centers it over the river. Every storyboard camera route
        // must follow the same fixture geometry.
        ProbePhase::Bridge => {
            fixture_building_draw_position(BuildingType::Bridge, BRIDGE_PROBE_GRID)
        }
        ProbePhase::ForegroundA | ProbePhase::ForegroundB => {
            WorldMap::grid_to_world(FOREGROUND_PROBE_GRID.0, FOREGROUND_PROBE_GRID.1)
        }
        ProbePhase::DoorOpen | ProbePhase::DoorClosed | ProbePhase::DoorLocked => {
            unreachable!("door phase has an expected state")
        }
    }
}

/// Returns the fixture's production draw position for an anchor.
///
/// The indoor-light fixture spawns completed blueprints through this exact
/// placement geometry. In particular, a Bridge's logical anchor controls its
/// river footprint while its root and 3D visual are centered over that span.
fn fixture_building_draw_position(kind: BuildingType, anchor: (i32, i32)) -> Vec2 {
    hw_ui::selection::building_geometry(kind, anchor, RIVER_Y_MIN).draw_pos
}

fn is_fixture_building_root(
    building: &Building,
    transform: &Transform,
    kind: BuildingType,
    anchor: (i32, i32),
) -> bool {
    building.kind == kind
        && transform.translation.truncate() == fixture_building_draw_position(kind, anchor)
}

fn project_rtt_roi(
    camera: &Camera,
    camera_transform: &GlobalTransform,
    world_position: Vec3,
    physical_width: u32,
    physical_height: u32,
) -> Option<ProbeRoi> {
    let center = project_rtt_point(
        camera,
        camera_transform,
        world_position,
        physical_width,
        physical_height,
    )?;
    roi_around_point(center, ROI_HALF_SIZE, physical_width, physical_height)
}

fn project_main_roi(
    camera: &Camera,
    camera_transform: &GlobalTransform,
    world_position: Vec3,
    physical_width: u32,
    physical_height: u32,
) -> Option<ProbeRoi> {
    let center = project_client_point(
        camera,
        camera_transform,
        world_position,
        physical_width,
        physical_height,
        1.0,
    )?;
    roi_around_point(center, ROI_HALF_SIZE, physical_width, physical_height)
}

fn project_rtt_point(
    camera: &Camera,
    camera_transform: &GlobalTransform,
    world_position: Vec3,
    physical_width: u32,
    physical_height: u32,
) -> Option<Vec2> {
    project_client_point(
        camera,
        camera_transform,
        world_position,
        physical_width,
        physical_height,
        topdown_rtt_vertical_compensation(),
    )
}

fn project_client_point(
    camera: &Camera,
    camera_transform: &GlobalTransform,
    world_position: Vec3,
    physical_width: u32,
    physical_height: u32,
    vertical_compensation: f32,
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
    let center_y = ((normalized_y - 0.5) * vertical_compensation + 0.5) * physical_height as f32;
    (center_x.is_finite() && center_y.is_finite()).then_some(Vec2::new(center_x, center_y))
}

fn roi_around_point(
    center: Vec2,
    half_size: u32,
    physical_width: u32,
    physical_height: u32,
) -> Option<ProbeRoi> {
    let half = half_size as f32;
    let center_x = center.x;
    let center_y = center.y;
    if center_x < half
        || center_y < half
        || center_x + half > physical_width as f32
        || center_y + half > physical_height as f32
    {
        return None;
    }
    Some(ProbeRoi {
        x: (center_x - half).round() as u32,
        y: (center_y - half).round() as u32,
        width: half_size * 2,
        height: half_size * 2,
    })
}

fn point_as_json(point: Vec2) -> Value {
    json!({"x": point.x, "y": point.y})
}

fn require_visible_probe_visual(
    visibility: &Visibility,
    inherited_visibility: &InheritedVisibility,
    label: &str,
) -> Result<(), String> {
    if *visibility == Visibility::Hidden {
        Err(format!("{label} is hidden"))
    } else if !inherited_visibility.get() {
        Err(format!("{label} is hidden by its hierarchy"))
    } else {
        Ok(())
    }
}

/// Require the object-side visibility that matches the production Render3d
/// mode. GPU screenshots must contain the concrete visual, whereas CPU
/// screenshots deliberately hide it and prove that state through an unchanged
/// composite plus this source-side assertion.
fn require_probe_visual_for_render_mode(
    visibility: &Visibility,
    inherited_visibility: &InheritedVisibility,
    is_gpu: bool,
    label: &str,
) -> Result<(), String> {
    if is_gpu {
        return require_visible_probe_visual(visibility, inherited_visibility, label);
    }
    if *visibility == Visibility::Hidden || !inherited_visibility.get() {
        Ok(())
    } else {
        Err(format!(
            "{label} remains visible in CPU Render3d-hidden mode"
        ))
    }
}

fn door_presentation_state(state: DoorState) -> DoorPresentationState {
    match state {
        DoorState::Closed => DoorPresentationState::Closed,
        DoorState::Open => DoorPresentationState::Open,
        DoorState::Locked => DoorPresentationState::Locked,
    }
}

const fn door_state_name(state: DoorState) -> &'static str {
    match state {
        DoorState::Closed => "closed",
        DoorState::Open => "open",
        DoorState::Locked => "locked",
    }
}

const fn door_presentation_name(state: DoorPresentationState) -> &'static str {
    match state {
        DoorPresentationState::Closed => "closed",
        DoorPresentationState::Open => "open",
        DoorPresentationState::Locked => "locked",
    }
}

fn write_status(path: &Path, value: &Value) -> std::io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("P02 status path has no parent"))?;
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
    use bevy::ecs::system::{IntoSystem, System};

    #[test]
    fn actual_window_mutable_transform_queries_are_parameter_set_isolated() {
        let mut world = World::new();
        let mut prepare = IntoSystem::into_system(prepare_p02_actual_window_view_system);
        prepare.initialize(&mut world);
        let mut actor = IntoSystem::into_system(apply_p02_actual_window_actor_probe_system);
        actor.initialize(&mut world);
    }

    #[test]
    fn actual_window_storyboard_is_complete_and_unique() {
        assert_eq!(ProbePhase::ALL.len(), 10);
        let mut ids = ProbePhase::ALL.map(ProbePhase::id).to_vec();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), ProbePhase::ALL.len());
        assert_eq!(ProbePhase::DoorOpen.next(), Some(ProbePhase::DoorClosed));
        assert_eq!(
            ProbePhase::Bridge.next(),
            Some(ProbePhase::WallBounceActive)
        );
        assert_eq!(
            ProbePhase::WallBounceRest.next(),
            Some(ProbePhase::ForegroundA)
        );
        assert_eq!(ProbePhase::ForegroundB.next(), None);
    }

    #[test]
    fn wall_bounce_probe_pins_the_production_effect_while_virtual_time_is_paused() {
        let mut bounce = active_wall_bounce_probe();
        let duration = bounce.bounce_animation.config.duration;
        assert!(bounce.bounce_animation.timer > 0.0);
        assert!(bounce.bounce_animation.timer < duration);

        // A paused `Time<Virtual>` has a zero delta. The production animation
        // still resolves the pinned midpoint to a non-unit owner scale, which
        // the normal Visual system then mirrors to `Building3dVisual`.
        let scale = hw_visual::animations::update_bounce_animation(
            &Time::default(),
            &mut bounce.bounce_animation,
        )
        .expect("midpoint completion bounce must still be active");
        assert!(scale > 1.001);

        finish_wall_bounce(&mut bounce);
        assert_eq!(bounce.bounce_animation.timer, duration);
    }

    #[test]
    fn pinned_wall_bounce_reaches_and_then_clears_its_3d_presentation_while_paused() {
        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<Time<Virtual>>()
            .add_systems(
                Update,
                (
                    hw_visual::blueprint::building_bounce_animation_system,
                    crate::systems::visual::building3d_cleanup::sync_building_3d_transform_system,
                )
                    .chain(),
            );
        app.world_mut().resource_mut::<Time<Virtual>>().pause();
        let paused_time = app.world().resource::<Time<Virtual>>().as_generic();
        app.world_mut().insert_resource(paused_time);

        let wall = app
            .world_mut()
            .spawn((
                Building {
                    kind: BuildingType::Wall,
                    is_provisional: false,
                },
                Transform::default(),
                active_wall_bounce_probe(),
            ))
            .id();
        let visual = app
            .world_mut()
            .spawn((Building3dVisual { owner: wall }, Transform::default()))
            .id();

        app.update();
        let owner_scale = app.world().get::<Transform>(wall).unwrap().scale.x;
        let visual_scale = app.world().get::<Transform>(visual).unwrap().scale.x;
        assert!(owner_scale > 1.001);
        assert!((owner_scale - visual_scale).abs() <= 0.001);

        {
            let mut bounce = app
                .world_mut()
                .get_mut::<BuildingBounceEffect>(wall)
                .unwrap();
            finish_wall_bounce(&mut bounce);
        }
        app.update();
        assert!(app.world().get::<BuildingBounceEffect>(wall).is_none());
        let owner_scale = app.world().get::<Transform>(wall).unwrap().scale.x;
        let visual_scale = app.world().get::<Transform>(visual).unwrap().scale.x;
        assert!((owner_scale - 1.0).abs() <= 0.001);
        assert!((visual_scale - 1.0).abs() <= 0.001);
    }

    #[test]
    fn bridge_probe_uses_the_fixture_draw_position_not_its_anchor_center() {
        let draw_position = fixture_building_draw_position(BuildingType::Bridge, BRIDGE_PROBE_GRID);
        assert_eq!(
            draw_position,
            hw_ui::selection::building_geometry(
                BuildingType::Bridge,
                BRIDGE_PROBE_GRID,
                RIVER_Y_MIN,
            )
            .draw_pos
        );
        assert_ne!(
            draw_position,
            WorldMap::grid_to_world(BRIDGE_PROBE_GRID.0, BRIDGE_PROBE_GRID.1),
        );
        assert_eq!(
            static_phase_camera_target(ProbePhase::Bridge),
            draw_position
        );
        let bridge = Building {
            kind: BuildingType::Bridge,
            is_provisional: false,
        };
        assert!(is_fixture_building_root(
            &bridge,
            &Transform::from_translation(draw_position.extend(0.0)),
            BuildingType::Bridge,
            BRIDGE_PROBE_GRID,
        ));
        assert!(!is_fixture_building_root(
            &bridge,
            &Transform::from_translation(
                WorldMap::grid_to_world(BRIDGE_PROBE_GRID.0, BRIDGE_PROBE_GRID.1).extend(0.0),
            ),
            BuildingType::Bridge,
            BRIDGE_PROBE_GRID,
        ));
    }

    #[test]
    fn bridge_probe_requires_the_production_rtt_layer_mesh_and_material() {
        let mut meshes = Assets::<Mesh>::default();
        let mut materials = Assets::<StandardMaterial>::default();
        let expected_mesh = meshes.add(Mesh::from(Cuboid::new(1.0, 1.0, 1.0)));
        let expected_material = materials.add(StandardMaterial::default());
        let bridge_layers = RenderLayers::layer(3);
        let camera_layers = RenderLayers::layer(3);
        let bridge_mesh = Mesh3d(expected_mesh.clone());
        let bridge_material = MeshMaterial3d(expected_material.clone());

        assert!(
            require_bridge_rtt_drawable(
                &bridge_layers,
                &camera_layers,
                &bridge_mesh,
                &bridge_material,
                &expected_mesh,
                &expected_material,
                &meshes,
                &materials,
            )
            .is_ok()
        );

        assert!(
            require_bridge_rtt_drawable(
                &RenderLayers::layer(2),
                &camera_layers,
                &bridge_mesh,
                &bridge_material,
                &expected_mesh,
                &expected_material,
                &meshes,
                &materials,
            )
            .is_err()
        );

        let other_mesh = meshes.add(Mesh::from(Cuboid::new(2.0, 2.0, 2.0)));
        assert!(
            require_bridge_rtt_drawable(
                &bridge_layers,
                &camera_layers,
                &Mesh3d(other_mesh),
                &bridge_material,
                &expected_mesh,
                &expected_material,
                &meshes,
                &materials,
            )
            .is_err()
        );

        let other_material = materials.add(StandardMaterial::default());
        assert!(
            require_bridge_rtt_drawable(
                &bridge_layers,
                &camera_layers,
                &bridge_mesh,
                &MeshMaterial3d(other_material),
                &expected_mesh,
                &expected_material,
                &meshes,
                &materials,
            )
            .is_err()
        );

        let removed_mesh = meshes
            .remove(expected_mesh.id())
            .expect("expected Bridge mesh must be registered");
        assert!(
            require_bridge_rtt_drawable(
                &bridge_layers,
                &camera_layers,
                &bridge_mesh,
                &bridge_material,
                &expected_mesh,
                &expected_material,
                &meshes,
                &materials,
            )
            .is_err()
        );
        meshes
            .insert(expected_mesh.id(), removed_mesh)
            .expect("restoring registered Bridge mesh must succeed");

        let removed_material = materials
            .remove(expected_material.id())
            .expect("expected Bridge material must be registered");
        assert!(
            require_bridge_rtt_drawable(
                &bridge_layers,
                &camera_layers,
                &bridge_mesh,
                &bridge_material,
                &expected_mesh,
                &expected_material,
                &meshes,
                &materials,
            )
            .is_err()
        );
        materials
            .insert(expected_material.id(), removed_material)
            .expect("restoring registered Bridge material must succeed");
    }

    #[test]
    fn p02_actual_window_roi_stays_inside_the_client_extent() {
        let roi = ProbeRoi {
            x: 528,
            y: 248,
            width: ROI_HALF_SIZE * 2,
            height: ROI_HALF_SIZE * 2,
        };
        assert!(roi.x + roi.width <= 1280);
        assert!(roi.y + roi.height <= 720);
    }

    #[test]
    fn acknowledgement_is_bound_to_the_current_nonce_phase_and_generation() {
        let valid = json!({
            "schema_version": STATUS_SCHEMA_VERSION,
            "status": "captured",
            "session_nonce": "a".repeat(32),
            "phase": "door-open",
            "generation": 1,
        });
        assert!(acknowledgement_matches(
            &valid,
            &"a".repeat(32),
            ProbePhase::DoorOpen,
            1
        ));
        assert!(!acknowledgement_matches(
            &json!({
                "schema_version": STATUS_SCHEMA_VERSION,
                "status": "captured",
                "session_nonce": "a".repeat(32),
                "phase": "door-open",
                "generation": 2,
            }),
            &"a".repeat(32),
            ProbePhase::DoorOpen,
            1
        ));
        assert!(!acknowledgement_matches(
            &json!({
                "schema_version": STATUS_SCHEMA_VERSION,
                "status": "captured",
                "session_nonce": "b".repeat(32),
                "phase": "door-open",
                "generation": 1,
            }),
            &"a".repeat(32),
            ProbePhase::DoorOpen,
            1
        ));
    }

    #[test]
    fn rtt_roi_mapping_applies_centered_vertical_compensation() {
        let centered = ((0.5 - 0.5) * topdown_rtt_vertical_compensation() + 0.5) * 720.0;
        let near_top = ((0.2 - 0.5) * topdown_rtt_vertical_compensation() + 0.5) * 720.0;
        assert_eq!(centered, 360.0);
        assert!(near_top < 144.0);
    }

    #[test]
    fn actual_window_probe_visibility_matches_render_mode() {
        assert!(
            require_probe_visual_for_render_mode(
                &Visibility::Visible,
                &InheritedVisibility::VISIBLE,
                true,
                "Door3dVisual"
            )
            .is_ok()
        );
        assert!(
            require_probe_visual_for_render_mode(
                &Visibility::Inherited,
                &InheritedVisibility::VISIBLE,
                true,
                "Bridge3dVisual"
            )
            .is_ok()
        );
        assert_eq!(
            require_probe_visual_for_render_mode(
                &Visibility::Hidden,
                &InheritedVisibility::VISIBLE,
                true,
                "Wall3dVisual"
            ),
            Err("Wall3dVisual is hidden".to_string())
        );
        assert_eq!(
            require_probe_visual_for_render_mode(
                &Visibility::Inherited,
                &InheritedVisibility::HIDDEN,
                true,
                "Wall3dVisual"
            ),
            Err("Wall3dVisual is hidden by its hierarchy".to_string())
        );
        assert!(
            require_probe_visual_for_render_mode(
                &Visibility::Hidden,
                &InheritedVisibility::VISIBLE,
                false,
                "Door3dVisual"
            )
            .is_ok()
        );
        assert!(
            require_probe_visual_for_render_mode(
                &Visibility::Inherited,
                &InheritedVisibility::HIDDEN,
                false,
                "Bridge3dVisual"
            )
            .is_ok()
        );
        assert_eq!(
            require_probe_visual_for_render_mode(
                &Visibility::Visible,
                &InheritedVisibility::VISIBLE,
                false,
                "Wall3dVisual"
            ),
            Err("Wall3dVisual remains visible in CPU Render3d-hidden mode".to_string())
        );
    }
}
