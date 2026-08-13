//! Production-only visual probe storyboard for the P02 actual-window recipe.
//!
//! This module never participates in an ordinary game or formal perf run. The
//! native acceptance launcher opts in with two environment variables, then
//! captures phase-tagged client-window frames from the normal indoor-light
//! fixture. Every probe uses an entity that the production fixture already
//! spawned; no `visual_test` or synthetic presentation path is accepted.

use super::config::{PerfRenderMode, PerfScenarioConfig};
use super::indoor_light_fixture::{IndoorLightFixturePhase, IndoorLightFixtureState};
use bevy::camera_controller::pan_camera::PanCamera;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use hw_core::camera::MainCamera;
use hw_core::constants::TILE_SIZE;
use hw_visual::blueprint::{CompletionText, DeliveryPopup};
use hw_visual::visual3d::{
    ActorBillboard3d, Building3dVisual, Door3dVisual, DoorPresentationState,
    LegacyStructural2dMirror,
};
use hw_world::WorldMap;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::plugins::startup::Camera3dRtt;
use crate::systems::jobs::{Building, BuildingType, Door, DoorState, RenderPresentationClass};

const ACCEPTANCE_ENV: &str = "HW_P02_PRESENTATION_ACTUAL_WINDOW";
const STATUS_PATH_ENV: &str = "HW_P02_PRESENTATION_STATUS_PATH";
const STATUS_SCHEMA_VERSION: u32 = 2;
const PHASE_HOLD: Duration = Duration::from_millis(1_600);
const SETTLE_FRAMES: u32 = 3;
const CAMERA_SCALE: f32 = 0.75;
const WALL_PROBE_GRID: (i32, i32) = (16, 20);
const BRIDGE_PROBE_GRID: (i32, i32) = (90, 65);
const FOREGROUND_PROBE_GRID: (i32, i32) = (27, 28);
const ROI_HALF_SIZE: u32 = 112;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProbePhase {
    DoorOpen,
    DoorClosed,
    DoorLocked,
    SoulFront,
    SoulBehind,
    Bridge,
    ForegroundA,
    ForegroundB,
}

impl ProbePhase {
    #[cfg(test)]
    const ALL: [Self; 8] = [
        Self::DoorOpen,
        Self::DoorClosed,
        Self::DoorLocked,
        Self::SoulFront,
        Self::SoulBehind,
        Self::Bridge,
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
            Self::Bridge => Some(Self::ForegroundA),
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
    phase: ProbePhase,
    phase_started_at: Option<Duration>,
    phase_frames: u32,
    generation: u32,
    published_generation: Option<u32>,
}

impl Default for P02ActualWindowAcceptance {
    fn default() -> Self {
        Self {
            requested: std::env::var(ACCEPTANCE_ENV).is_ok_and(|value| value == "1"),
            status_path: std::env::var_os(STATUS_PATH_ENV)
                .map(PathBuf::from)
                .filter(|path| path.is_absolute() && !path.as_os_str().is_empty()),
            phase: ProbePhase::DoorOpen,
            phase_started_at: None,
            phase_frames: 0,
            generation: 1,
            published_generation: None,
        }
    }
}

impl P02ActualWindowAcceptance {
    fn enabled(&self, config: &PerfScenarioConfig, fixture: &IndoorLightFixtureState) -> bool {
        self.requested
            && self.status_path.is_some()
            && fixture.phase == IndoorLightFixturePhase::Ready
            && config.rtt_light_selection().is_some_and(|selection| {
                selection.stage_id() == "p02" && selection.lane() == "static"
            })
    }

    fn advance_if_due(&mut self, now: Duration) {
        let started_at = *self.phase_started_at.get_or_insert(now);
        if now.saturating_sub(started_at) < PHASE_HOLD {
            self.phase_frames = self.phase_frames.saturating_add(1);
            return;
        }
        let Some(next) = self.phase.next() else {
            self.phase_frames = self.phase_frames.saturating_add(1);
            return;
        };
        self.phase = next;
        self.phase_started_at = Some(now);
        self.phase_frames = 1;
        self.generation = self.generation.saturating_add(1);
        self.published_generation = None;
    }

    fn mark_published(&mut self) {
        self.published_generation = Some(self.generation);
    }
}

/// Applies the next deterministic camera and foreground state before the
/// normal Visual schedule synchronizes the RtT camera and active visuals.
pub(crate) fn prepare_p02_actual_window_view_system(
    config: Res<PerfScenarioConfig>,
    fixture: Res<IndoorLightFixtureState>,
    time: Res<Time<Real>>,
    mut acceptance: ResMut<P02ActualWindowAcceptance>,
    mut camera: Query<(&mut Transform, &mut Projection, &mut PanCamera), With<MainCamera>>,
    doors: Query<(Entity, &Door, &Transform)>,
    owners: Query<&Building>,
    mut foreground: Query<
        (&ChildOf, &mut Transform),
        (With<Sprite>, Without<LegacyStructural2dMirror>),
    >,
    mut ui_roots: Query<&mut Node, Without<ChildOf>>,
    mut transient_text: Query<&mut Visibility, Or<(With<CompletionText>, With<DeliveryPopup>)>>,
) {
    if !acceptance.enabled(&config, &fixture) {
        return;
    }
    acceptance.advance_if_due(time.elapsed());
    let phase = acceptance.phase;

    let center = if let Some(expected_state) = phase.expected_door_state() {
        doors
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
        let grid = match phase {
            ProbePhase::SoulFront | ProbePhase::SoulBehind => WALL_PROBE_GRID,
            ProbePhase::Bridge => BRIDGE_PROBE_GRID,
            ProbePhase::ForegroundA | ProbePhase::ForegroundB => FOREGROUND_PROBE_GRID,
            ProbePhase::DoorOpen | ProbePhase::DoorClosed | ProbePhase::DoorLocked => {
                unreachable!("door phase has an expected state")
            }
        };
        WorldMap::grid_to_world(grid.0, grid.1)
    };
    if let Ok((mut transform, mut projection, mut pan)) = camera.single_mut() {
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
    for (parent, mut transform) in &mut foreground {
        let Ok(building) = owners.get(parent.parent()) else {
            continue;
        };
        if building.kind != BuildingType::SandPile
            || crate::systems::jobs::presentation_class(building.kind)
                != RenderPresentationClass::Foreground2d
        {
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

/// Keeps only the selected production billboard visible during the two depth
/// phases, then places that existing visual in front of or behind an existing
/// fixture Wall. The Soul's semantic root is intentionally untouched so the
/// performance artifact remains the static fixture contract.
pub(crate) fn apply_p02_actual_window_actor_probe_system(
    config: Res<PerfScenarioConfig>,
    fixture: Res<IndoorLightFixtureState>,
    acceptance: Res<P02ActualWindowAcceptance>,
    camera: Query<(&Camera, &GlobalTransform), With<Camera3dRtt>>,
    owners: Query<(Entity, &Building, &Transform)>,
    visuals: Query<(&Building3dVisual, &Transform)>,
    mut billboards: Query<(&ActorBillboard3d, &mut Transform, &mut Visibility)>,
) {
    if !acceptance.enabled(&config, &fixture) {
        return;
    }
    let Some(subject) = fixture.actual_window_subject_soul() else {
        return;
    };
    let is_gpu = matches!(config.render_mode, PerfRenderMode::Gpu);
    let phase = acceptance.phase;
    for (billboard, _, mut visibility) in &mut billboards {
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
    let wall_position = owners
        .iter()
        .find(|(_, building, transform)| {
            building.kind == BuildingType::Wall
                && transform.translation.truncate()
                    == WorldMap::grid_to_world(WALL_PROBE_GRID.0, WALL_PROBE_GRID.1)
        })
        .and_then(|(wall, _, _)| {
            visuals
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
    let mut candidates = Vec::new();
    for offset_y in -3..=3 {
        for offset_x in -3..=3 {
            if offset_x == 0 && offset_y == 0 {
                continue;
            }
            let position = Vec3::new(
                wall_position.x + offset_x as f32 * TILE_SIZE,
                TILE_SIZE * 0.55,
                wall_position.z - offset_y as f32 * TILE_SIZE,
            );
            let Ok(view) = camera.world_to_viewport_with_depth(camera_transform, position) else {
                continue;
            };
            let screen_distance = view.truncate().distance(wall_view.truncate());
            if screen_distance <= ROI_HALF_SIZE as f32 * 0.65 {
                candidates.push((screen_distance, view.z, position));
            }
        }
    }
    let selected = match phase {
        ProbePhase::SoulFront => candidates.into_iter().min_by(|left, right| {
            left.1
                .total_cmp(&right.1)
                .then_with(|| left.0.total_cmp(&right.0))
        }),
        ProbePhase::SoulBehind => candidates.into_iter().max_by(|left, right| {
            left.1
                .total_cmp(&right.1)
                .then_with(|| right.0.total_cmp(&left.0))
        }),
        _ => None,
    };
    let Some((_, _, position)) = selected else {
        return;
    };
    for (billboard, mut transform, _) in &mut billboards {
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
    mut acceptance: ResMut<P02ActualWindowAcceptance>,
    window: Query<&Window, With<PrimaryWindow>>,
    camera_3d: Query<(&Camera, &GlobalTransform), With<Camera3dRtt>>,
    main_camera: Query<(&Camera, &GlobalTransform), (With<MainCamera>, Without<Camera3dRtt>)>,
    doors: Query<(Entity, &Door, &Transform)>,
    door_visuals: Query<(&Door3dVisual, &DoorPresentationState, &Transform)>,
    owners: Query<(Entity, &Building, &Transform)>,
    building_visuals: Query<(&Building3dVisual, &Transform)>,
    billboards: Query<(&ActorBillboard3d, &Transform, &Visibility)>,
    foreground: Query<
        (&ChildOf, &GlobalTransform),
        (With<Sprite>, Without<LegacyStructural2dMirror>),
    >,
) {
    if !acceptance.enabled(&config, &fixture)
        || acceptance.phase_frames < SETTLE_FRAMES
        || acceptance.published_generation == Some(acceptance.generation)
    {
        return;
    }
    let Some(path) = acceptance.status_path.clone() else {
        return;
    };
    let status = match build_probe_status(
        &config,
        &fixture,
        acceptance.phase,
        acceptance.generation,
        &window,
        &camera_3d,
        &main_camera,
        &doors,
        &door_visuals,
        &owners,
        &building_visuals,
        &billboards,
        &foreground,
    ) {
        Ok(value) => value,
        Err(reason) => json!({
            "schema_version": STATUS_SCHEMA_VERSION,
            "status": "failed",
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

#[allow(clippy::too_many_arguments)]
fn build_probe_status(
    config: &PerfScenarioConfig,
    fixture: &IndoorLightFixtureState,
    phase: ProbePhase,
    generation: u32,
    window: &Query<&Window, With<PrimaryWindow>>,
    camera_3d: &Query<(&Camera, &GlobalTransform), With<Camera3dRtt>>,
    main_camera: &Query<(&Camera, &GlobalTransform), (With<MainCamera>, Without<Camera3dRtt>)>,
    doors: &Query<(Entity, &Door, &Transform)>,
    door_visuals: &Query<(&Door3dVisual, &DoorPresentationState, &Transform)>,
    owners: &Query<(Entity, &Building, &Transform)>,
    building_visuals: &Query<(&Building3dVisual, &Transform)>,
    billboards: &Query<(&ActorBillboard3d, &Transform, &Visibility)>,
    foreground: &Query<
        (&ChildOf, &GlobalTransform),
        (With<Sprite>, Without<LegacyStructural2dMirror>),
    >,
) -> Result<Value, String> {
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
        let (visual, presentation, transform) = door_visuals
            .iter()
            .find(|(visual, _, _)| visual.owner == door_entity)
            .ok_or_else(|| format!("missing {expected_state:?} Door3dVisual"))?;
        let _ = visual;
        let expected_presentation = door_presentation_state(expected_state);
        if *presentation != expected_presentation {
            return Err(format!(
                "Door semantic/presentation mismatch: expected {expected_presentation:?}, got {presentation:?}"
            ));
        }
        let (camera, global) = camera_3d.single().map_err(|_| "missing RtT camera")?;
        let roi = project_roi(
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
        let (_, wall_transform) = building_visuals
            .iter()
            .find(|(visual, _)| visual.owner == wall_entity)
            .ok_or_else(|| "missing production Wall3dVisual depth probe".to_string())?;
        let (camera, global) = camera_3d.single().map_err(|_| "missing RtT camera")?;
        let wall_view = camera
            .world_to_viewport_with_depth(global, wall_transform.translation)
            .map_err(|error| format!("cannot project Wall depth probe: {error}"))?;
        let roi = project_roi(
            camera,
            global,
            wall_transform.translation,
            physical_width,
            physical_height,
        )
        .ok_or_else(|| "Wall depth probe projection is outside the client window".to_string())?;
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
            let (_, actor_transform, visibility) =
                actor.ok_or_else(|| "GPU actual-window case has no Soul billboard".to_string())?;
            if *visibility == Visibility::Hidden {
                return Err("GPU Soul depth subject is hidden".to_string());
            }
            let actor_view = camera
                .world_to_viewport_with_depth(global, actor_transform.translation)
                .map_err(|error| format!("cannot project Soul depth probe: {error}"))?;
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
            })
        }
    } else if phase == ProbePhase::Bridge {
        let bridge_position = WorldMap::grid_to_world(BRIDGE_PROBE_GRID.0, BRIDGE_PROBE_GRID.1);
        let (bridge_entity, _, _) = owners
            .iter()
            .find(|(_, building, transform)| {
                building.kind == BuildingType::Bridge
                    && transform.translation.truncate() == bridge_position
            })
            .ok_or_else(|| "missing production Bridge root".to_string())?;
        let (_, transform) = building_visuals
            .iter()
            .find(|(visual, _)| visual.owner == bridge_entity)
            .ok_or_else(|| "missing production Bridge3dVisual".to_string())?;
        let (camera, global) = camera_3d.single().map_err(|_| "missing RtT camera")?;
        let roi = project_roi(
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
            "render3d": if is_gpu { "visible" } else { "hidden" },
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
        let roi = project_roi(
            camera,
            global,
            transform.translation(),
            physical_width,
            physical_height,
        )
        .ok_or_else(|| "Foreground probe projection is outside the client window".to_string())?;
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
        "phase": phase.id(),
        "generation": generation,
        "fixture_layout_checksum": fixture_checksum,
        "render3d": if is_gpu { "visible" } else { "hidden" },
        "camera_target": {"x": camera_target.x, "y": camera_target.y},
        "window": {"physical_width": physical_width, "physical_height": physical_height},
        "probe": probe,
    }))
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
    let grid = match phase {
        ProbePhase::SoulFront | ProbePhase::SoulBehind => WALL_PROBE_GRID,
        ProbePhase::Bridge => BRIDGE_PROBE_GRID,
        ProbePhase::ForegroundA | ProbePhase::ForegroundB => FOREGROUND_PROBE_GRID,
        ProbePhase::DoorOpen | ProbePhase::DoorClosed | ProbePhase::DoorLocked => {
            unreachable!("door phase has an expected state")
        }
    };
    Ok(WorldMap::grid_to_world(grid.0, grid.1))
}

fn project_roi(
    camera: &Camera,
    camera_transform: &GlobalTransform,
    world_position: Vec3,
    physical_width: u32,
    physical_height: u32,
) -> Option<ProbeRoi> {
    let viewport = camera.logical_viewport_size()?;
    if viewport.x <= 0.0 || viewport.y <= 0.0 {
        return None;
    }
    let point = camera
        .world_to_viewport(camera_transform, world_position)
        .ok()?;
    let center_x = point.x / viewport.x * physical_width as f32;
    let center_y = point.y / viewport.y * physical_height as f32;
    let half = ROI_HALF_SIZE as f32;
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
        width: ROI_HALF_SIZE * 2,
        height: ROI_HALF_SIZE * 2,
    })
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

    #[test]
    fn actual_window_storyboard_is_complete_and_unique() {
        assert_eq!(ProbePhase::ALL.len(), 8);
        let mut ids = ProbePhase::ALL.map(ProbePhase::id).to_vec();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), ProbePhase::ALL.len());
        assert_eq!(ProbePhase::DoorOpen.next(), Some(ProbePhase::DoorClosed));
        assert_eq!(ProbePhase::ForegroundB.next(), None);
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
}
