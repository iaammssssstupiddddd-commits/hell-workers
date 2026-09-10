//! Joint actual-window acceptance for production Wall and Door candidates.
//!
//! This profiling-only scene uses normal `Building` roots, the shared
//! `WallTopologyIndex`, and the production Wall/Door presentation systems. It
//! holds six ACK-bound checkpoints, including real support teardown/recreation
//! and a paused normal save/load through the Last dispatcher. The construction
//! preview consumes the same production topology and asset pool.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use bevy::camera::visibility::RenderLayers;
use bevy::mesh::Mesh3d;
use bevy::pbr::MeshMaterial3d;
use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::transform::TransformSystems;
use bevy::window::PrimaryWindow;
use hw_core::constants::topdown_rtt_vertical_compensation;
use hw_core::visual_mirror::building::{BuildingTypeVisual, BuildingVisualState};
use hw_core::world::DoorState;
use hw_core::{SaveSlotId, WorldEpoch};
use hw_jobs::{Blueprint, Building, BuildingType, Door, ProvisionalWall};
use hw_ui::camera::MainCamera;
use hw_visual::TopDownStructuralMaterial;
use hw_visual::visual3d::{
    Building3dVisual, Door3dPresentationMode, DoorPresentationAxis, DoorPresentationState,
    Wall3dPresentationMode, Wall3dPresentationState,
};
use hw_visual::wall_connection::{WallConnectionMask, WallMeshFamily, resolve_wall_topology};
use hw_world::{WorldMap, WorldMapWrite};
use serde_json::{Value, json};

use crate::assets::door_asset_set::{
    DoorAssetReadiness, DoorAssetReadinessState, ProductionDoorAssetPool,
    ProductionDoorMaterialPool,
};
use crate::assets::wall_asset_set::{
    ProductionWallAssetPool, ProductionWallMaterialPool, WallAssetReadiness,
    WallAssetReadinessState, WallProductionActivation, WallProductionActivationState,
};
use crate::plugins::startup::{Building3dHandles, Camera3dRtt};
use crate::plugins::visual::WallPresentationApplySet;
use crate::systems::jobs::spawn_building_3d_visual;
use crate::systems::jobs::wall_construction::spawn_wall_shell;
use crate::systems::save::{
    SaveLoadOperation, SaveLoadOutcome, SaveLoadResult, SaveLoadState, SavePath, SaveStorageRoot,
    manual_save_request, normal_load_request,
};
use crate::systems::visual::building3d_cleanup::DoorPresentationSyncSet;

use super::PerfScenarioConfig;
use super::fixture::PerfFixtureMarker;

const REQUEST_ENV: &str = "HW_WALL_DOOR_JOINT_ACTUAL_WINDOW";
const STATUS_ENV: &str = "HW_WALL_DOOR_JOINT_STATUS_PATH";
const ACK_ENV: &str = "HW_WALL_DOOR_JOINT_ACK_PATH";
const NONCE_ENV: &str = "HW_WALL_DOOR_JOINT_SESSION_NONCE";
const STATUS_SCHEMA_VERSION: u32 = 2;
const SETTLE: Duration = Duration::from_millis(1_600);
const ACK_TIMEOUT: Duration = Duration::from_secs(15);
const CAMERA_SCALE: f32 = 1.0;
const CAMERA_GRID_CENTER: (i32, i32) = (27, 47);
const PREVIEW_GRID: (i32, i32) = (14, 50);
const PERSISTENT_FORMWORK_GRID: (i32, i32) = (25, 51);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum JointCheckpoint {
    Framed,
    Completed,
    SupportChanged,
    SupportRemoved,
    SupportRestored,
    Loaded,
}

impl JointCheckpoint {
    const ALL: [Self; 6] = [
        Self::Framed,
        Self::Completed,
        Self::SupportChanged,
        Self::SupportRemoved,
        Self::SupportRestored,
        Self::Loaded,
    ];

    const fn phase(self) -> &'static str {
        match self {
            Self::Framed => "wall-door-joint-framed",
            Self::Completed => "wall-door-joint-completed",
            Self::SupportChanged => "wall-door-joint-support-changed",
            Self::SupportRemoved => "wall-door-joint-support-removed",
            Self::SupportRestored => "wall-door-joint-support-restored",
            Self::Loaded => "wall-door-joint-loaded",
        }
    }

    const fn generation(self) -> u32 {
        match self {
            Self::Framed => 1,
            Self::Completed => 2,
            Self::SupportChanged => 3,
            Self::SupportRemoved => 4,
            Self::SupportRestored => 5,
            Self::Loaded => 6,
        }
    }
}

#[derive(Clone, Copy)]
struct JointDoorTarget {
    owner: Entity,
    grid: (i32, i32),
    state: DoorPresentationState,
    initial_axis: DoorPresentationAxis,
}

#[derive(Clone, Copy)]
struct JointWallTarget {
    owner: Entity,
    grid: (i32, i32),
    initially_provisional: bool,
}

#[derive(Resource)]
pub(crate) struct WallDoorJointActualWindowAcceptance {
    requested: bool,
    status_path: Option<PathBuf>,
    ack_path: Option<PathBuf>,
    nonce: Option<String>,
    checkpoint_index: usize,
    doors: Vec<JointDoorTarget>,
    walls: Vec<JointWallTarget>,
    visual_entities: Option<HashMap<Entity, Entity>>,
    started_at: Option<Duration>,
    published_at: Option<Duration>,
    completed: bool,
    failed: bool,
    preview: Option<Entity>,
    load_operation: Option<SaveLoadOperation>,
    initial_epoch: Option<u64>,
    loaded_epoch: Option<u64>,
}

impl Default for WallDoorJointActualWindowAcceptance {
    fn default() -> Self {
        Self {
            requested: Self::requested_from_environment(),
            status_path: absolute_path(STATUS_ENV),
            ack_path: absolute_path(ACK_ENV),
            nonce: std::env::var(NONCE_ENV).ok().filter(|value| {
                value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
            }),
            checkpoint_index: 0,
            doors: Vec::new(),
            walls: Vec::new(),
            visual_entities: None,
            started_at: None,
            published_at: None,
            completed: false,
            failed: false,
            preview: None,
            load_operation: None,
            initial_epoch: None,
            loaded_epoch: None,
        }
    }
}

impl WallDoorJointActualWindowAcceptance {
    pub(crate) fn requested_from_environment() -> bool {
        std::env::var(REQUEST_ENV).as_deref() == Ok("1")
    }

    fn enabled(&self, config: &PerfScenarioConfig) -> bool {
        self.requested
            && config.wall_door_joint_actual_window()
            && self.status_path.is_some()
            && self.ack_path.is_some()
            && self.nonce.is_some()
            && !self.completed
            && !self.failed
    }

    fn checkpoint(&self) -> JointCheckpoint {
        JointCheckpoint::ALL[self.checkpoint_index]
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
}

fn absolute_path(key: &str) -> Option<PathBuf> {
    std::env::var_os(key)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute() && !path.as_os_str().is_empty())
}

fn acknowledgement_matches(value: &Value, nonce: &str, checkpoint: JointCheckpoint) -> bool {
    value.get("schema_version").and_then(Value::as_u64) == Some(STATUS_SCHEMA_VERSION.into())
        && value.get("status").and_then(Value::as_str) == Some("captured")
        && value.get("session_nonce").and_then(Value::as_str) == Some(nonce)
        && value.get("phase").and_then(Value::as_str) == Some(checkpoint.phase())
        && value.get("generation").and_then(Value::as_u64) == Some(checkpoint.generation().into())
}

fn spawn_wall(
    commands: &mut Commands,
    world_map: &mut WorldMapWrite,
    handles: &Building3dHandles,
    grid: (i32, i32),
    provisional: bool,
) -> JointWallTarget {
    let owner = spawn_wall_shell(commands, handles, grid, provisional);
    commands.entity(owner).insert(BuildingVisualState {
        kind: BuildingTypeVisual::Wall,
        is_provisional: provisional,
    });
    world_map.reserve_building_footprint(BuildingType::Wall, owner, std::iter::once(grid));
    JointWallTarget {
        owner,
        grid,
        initially_provisional: provisional,
    }
}

fn spawn_door(
    commands: &mut Commands,
    world_map: &mut WorldMapWrite,
    handles: &Building3dHandles,
    grid: (i32, i32),
    state: DoorState,
    axis: DoorPresentationAxis,
) -> JointDoorTarget {
    let position = WorldMap::grid_to_world(grid.0, grid.1);
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
            Door { state },
            Transform::from_xyz(position.x, position.y, 0.0),
            Visibility::Inherited,
            Name::new(format!("Wall/Door Joint {axis:?} {state:?}")),
        ))
        .id();
    spawn_building_3d_visual(
        commands,
        owner,
        BuildingType::Door,
        position,
        false,
        handles,
    );
    world_map.reserve_building_footprint(BuildingType::Door, owner, std::iter::once(grid));
    world_map.register_door(grid, owner, state);
    JointDoorTarget {
        owner,
        grid,
        state: match state {
            DoorState::Closed => DoorPresentationState::Closed,
            DoorState::Open => DoorPresentationState::Open,
            DoorState::Locked => DoorPresentationState::Locked,
        },
        initial_axis: axis,
    }
}

pub(crate) fn setup_wall_door_joint_gallery_system(
    mut commands: Commands,
    config: Res<PerfScenarioConfig>,
    handles: Res<Building3dHandles>,
    mut world_map: WorldMapWrite,
    mut acceptance: ResMut<WallDoorJointActualWindowAcceptance>,
) {
    if !acceptance.enabled(&config) || !acceptance.doors.is_empty() {
        return;
    }

    let single_specs = [
        (
            (14, 44),
            DoorState::Closed,
            DoorPresentationAxis::EastWest,
            false,
        ),
        (
            (20, 44),
            DoorState::Open,
            DoorPresentationAxis::NorthSouth,
            false,
        ),
        (
            (26, 44),
            DoorState::Open,
            DoorPresentationAxis::EastWest,
            true,
        ),
        (
            (32, 44),
            DoorState::Locked,
            DoorPresentationAxis::NorthSouth,
            true,
        ),
    ];
    for (grid, state, axis, formwork) in single_specs {
        let supports = match axis {
            DoorPresentationAxis::EastWest => [(grid.0 - 1, grid.1), (grid.0 + 1, grid.1)],
            DoorPresentationAxis::NorthSouth => [(grid.0, grid.1 - 1), (grid.0, grid.1 + 1)],
        };
        acceptance.walls.push(spawn_wall(
            &mut commands,
            &mut world_map,
            &handles,
            supports[0],
            formwork,
        ));
        acceptance.walls.push(spawn_wall(
            &mut commands,
            &mut world_map,
            &handles,
            supports[1],
            false,
        ));
        acceptance.doors.push(spawn_door(
            &mut commands,
            &mut world_map,
            &handles,
            grid,
            state,
            axis,
        ));
    }

    for (grid, state) in [((38, 44), DoorState::Closed), ((39, 44), DoorState::Open)] {
        acceptance.doors.push(spawn_door(
            &mut commands,
            &mut world_map,
            &handles,
            grid,
            state,
            DoorPresentationAxis::EastWest,
        ));
    }
    for grid in [(37, 44), (40, 44)] {
        acceptance.walls.push(spawn_wall(
            &mut commands,
            &mut world_map,
            &handles,
            grid,
            false,
        ));
    }
    for (grid, state) in [((38, 50), DoorState::Closed), ((38, 51), DoorState::Open)] {
        acceptance.doors.push(spawn_door(
            &mut commands,
            &mut world_map,
            &handles,
            grid,
            state,
            DoorPresentationAxis::NorthSouth,
        ));
    }
    acceptance.doors.push(spawn_door(
        &mut commands,
        &mut world_map,
        &handles,
        (26, 50),
        DoorState::Open,
        DoorPresentationAxis::EastWest,
    ));
    for grid in [
        (38, 49),
        (38, 52),
        (25, 50),
        (27, 50),
        PERSISTENT_FORMWORK_GRID,
        (13, 50),
        (15, 50),
    ] {
        acceptance.walls.push(spawn_wall(
            &mut commands,
            &mut world_map,
            &handles,
            grid,
            grid == PERSISTENT_FORMWORK_GRID,
        ));
    }
    let position = WorldMap::grid_to_world(PREVIEW_GRID.0, PREVIEW_GRID.1);
    let blueprint = Blueprint::new(BuildingType::Door, vec![PREVIEW_GRID]);
    let mirror = hw_jobs::visual_sync::blueprint_visual_state(&blueprint);
    let preview = commands
        .spawn((
            blueprint,
            mirror,
            Sprite::default(),
            Anchor::CENTER,
            Transform::from_xyz(position.x, position.y, 10.0),
            Visibility::Inherited,
            Name::new("Joint Door construction preview"),
        ))
        .id();
    world_map.reserve_building_footprint(BuildingType::Door, preview, [PREVIEW_GRID]);
    acceptance.preview = Some(preview);
}

#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct JointDriveParams<'w, 's> {
    commands: Commands<'w, 's>,
    density_fixture: ResMut<'w, super::door_density_fixture::DoorDensityFixtureState>,
    buildings: Query<'w, 's, (Entity, &'static mut Building, &'static mut Transform)>,
    blueprints: Query<'w, 's, (Entity, &'static Blueprint)>,
    handles: Res<'w, Building3dHandles>,
    epoch: Res<'w, WorldEpoch>,
    save_state: ResMut<'w, SaveLoadState>,
    save_root: ResMut<'w, SaveStorageRoot>,
    save_path: ResMut<'w, SavePath>,
    outcomes: MessageReader<'w, 's, SaveLoadOutcome>,
    world_map: WorldMapWrite<'w>,
    exit: MessageWriter<'w, AppExit>,
}

pub(crate) fn drive_wall_door_joint_checkpoint_system(
    time: Res<Time<Real>>,
    config: Res<PerfScenarioConfig>,
    mut params: JointDriveParams,
    mut acceptance: ResMut<WallDoorJointActualWindowAcceptance>,
) {
    if !acceptance.enabled(&config) || acceptance.doors.len() != 9 {
        return;
    }
    acceptance.initial_epoch.get_or_insert(params.epoch.get());
    if acceptance.load_operation.is_some() {
        drive_joint_load(&mut params, &mut acceptance, time.elapsed());
        return;
    }
    let Some(published) = acceptance.published_at else {
        return;
    };
    let now = time.elapsed();
    if !acceptance.acknowledged() {
        if now.saturating_sub(published) >= ACK_TIMEOUT {
            publish_failure(&mut acceptance, "joint capture acknowledgement timed out");
        }
        return;
    }

    match acceptance.checkpoint() {
        JointCheckpoint::Framed => {
            for wall in &acceptance.walls {
                if !wall.initially_provisional || wall.grid == PERSISTENT_FORMWORK_GRID {
                    continue;
                }
                let Ok((_, mut building, _)) = params.buildings.get_mut(wall.owner) else {
                    publish_failure(
                        &mut acceptance,
                        "formwork owner disappeared before completion",
                    );
                    return;
                };
                building.is_provisional = false;
                params
                    .commands
                    .entity(wall.owner)
                    .remove::<ProvisionalWall>();
            }
        }
        JointCheckpoint::Completed => {
            let door = acceptance.doors[0];
            let new_grids = [
                (door.grid.0, door.grid.1 - 1),
                (door.grid.0, door.grid.1 + 1),
            ];
            for (index, new_grid) in new_grids.into_iter().enumerate() {
                let wall = &mut acceptance.walls[index];
                params
                    .world_map
                    .release_building_footprint_if_owned(wall.owner, std::iter::once(wall.grid));
                let Ok((_, _, mut transform)) = params.buildings.get_mut(wall.owner) else {
                    publish_failure(
                        &mut acceptance,
                        "support owner disappeared before relocation",
                    );
                    return;
                };
                let position = WorldMap::grid_to_world(new_grid.0, new_grid.1);
                transform.translation.x = position.x;
                transform.translation.y = position.y;
                params.world_map.reserve_building_footprint(
                    BuildingType::Wall,
                    wall.owner,
                    std::iter::once(new_grid),
                );
                wall.grid = new_grid;
            }
            for (index, grid) in [(15, (14, 49)), (16, (14, 51))] {
                let wall = &mut acceptance.walls[index];
                params
                    .world_map
                    .release_building_footprint_if_owned(wall.owner, [wall.grid]);
                let Ok((_, _, mut transform)) = params.buildings.get_mut(wall.owner) else {
                    publish_failure(&mut acceptance, "preview support disappeared");
                    return;
                };
                let position = WorldMap::grid_to_world(grid.0, grid.1);
                transform.translation.x = position.x;
                transform.translation.y = position.y;
                params
                    .world_map
                    .reserve_building_footprint(BuildingType::Wall, wall.owner, [grid]);
                wall.grid = grid;
            }
        }
        JointCheckpoint::SupportChanged => {
            let wall = acceptance.walls.remove(1);
            params
                .world_map
                .release_building_footprint_if_owned(wall.owner, [wall.grid]);
            params.commands.entity(wall.owner).despawn();
        }
        JointCheckpoint::SupportRemoved => {
            let wall = spawn_wall(
                &mut params.commands,
                &mut params.world_map,
                &params.handles,
                (14, 45),
                false,
            );
            acceptance.walls.insert(1, wall);
        }
        JointCheckpoint::SupportRestored => {
            let root = acceptance
                .status_path
                .as_ref()
                .expect("enabled probe path")
                .parent()
                .expect("absolute probe parent")
                .join("joint-save");
            if root.exists() || std::fs::create_dir(&root).is_err() {
                publish_failure(
                    &mut acceptance,
                    "joint private save directory is unavailable",
                );
                return;
            }
            *params.save_root = SaveStorageRoot::new(root.clone());
            *params.save_path = SavePath::new(root.join(SaveSlotId::Manual1.canonical_file_name()));
            if !params
                .save_state
                .try_set(manual_save_request(SaveSlotId::Manual1, 1))
            {
                publish_failure(&mut acceptance, "joint save dispatcher is busy");
                return;
            }
            acceptance.load_operation = Some(SaveLoadOperation::Save);
        }
        JointCheckpoint::Loaded => {
            if let Some(path) = acceptance.status_path.as_deref() {
                let checkpoint = acceptance.checkpoint();
                if write_json(
                    path,
                    &json!({
                        "schema_version": STATUS_SCHEMA_VERSION,
                        "status": "complete",
                        "session_nonce": acceptance.nonce,
                        "phase": checkpoint.phase(),
                        "generation": checkpoint.generation(),
                    }),
                )
                .is_err()
                {
                    publish_failure(&mut acceptance, "cannot publish final acknowledgement");
                    return;
                }
            }
            acceptance.completed = true;
            params.exit.write(AppExit::Success);
            return;
        }
    }
    acceptance.checkpoint_index += 1;
    acceptance.started_at = Some(now);
    acceptance.published_at = None;
}

fn drive_joint_load(
    params: &mut JointDriveParams,
    acceptance: &mut WallDoorJointActualWindowAcceptance,
    now: Duration,
) {
    let outcomes: Vec<_> = params.outcomes.read().cloned().collect();
    let Some(operation) = acceptance.load_operation else {
        return;
    };
    let Some(outcome) = outcomes
        .iter()
        .find(|outcome| outcome.operation == operation)
    else {
        return;
    };
    if outcome.result != SaveLoadResult::Succeeded {
        publish_failure(
            acceptance,
            &format!("joint {operation:?} failed: {:?}", outcome.result),
        );
        return;
    }
    if operation == SaveLoadOperation::Save {
        if !params.density_fixture.handoff_to_joint_storyboard() {
            publish_failure(acceptance, "joint carrier was not ready for load handoff");
            return;
        }
        if !params
            .save_state
            .try_set(normal_load_request(SaveSlotId::Manual1, 1))
        {
            publish_failure(acceptance, "joint load dispatcher is busy");
            return;
        }
        acceptance.load_operation = Some(SaveLoadOperation::Load);
        return;
    }
    if acceptance.initial_epoch.map(|epoch| epoch + 1) != Some(params.epoch.get()) {
        publish_failure(
            acceptance,
            "joint normal load did not replace exactly one world",
        );
        return;
    }
    // Old Entity IDs are invalid now. Match durable roles and coordinates before
    // any presentation consumer sees the acceptance target collection again.
    for (owner, grid, kind) in acceptance
        .doors
        .iter_mut()
        .map(|target| (&mut target.owner, target.grid, BuildingType::Door))
        .chain(
            acceptance
                .walls
                .iter_mut()
                .map(|target| (&mut target.owner, target.grid, BuildingType::Wall)),
        )
    {
        let matches: Vec<_> = params
            .buildings
            .iter()
            .filter(|(_, building, transform)| {
                building.kind == kind
                    && WorldMap::world_to_grid(transform.translation.truncate()) == grid
            })
            .map(|(entity, ..)| entity)
            .collect();
        if matches.len() != 1 || matches[0] == *owner {
            acceptance.failed = true;
            break;
        }
        *owner = matches[0];
    }
    let previews: Vec<_> = params
        .blueprints
        .iter()
        .filter(|(_, blueprint)| {
            blueprint.kind == BuildingType::Door && blueprint.occupied_grids == [PREVIEW_GRID]
        })
        .map(|(entity, _)| entity)
        .collect();
    if acceptance.failed || previews.len() != 1 || Some(previews[0]) == acceptance.preview {
        publish_failure(acceptance, "joint loaded target rebind differs");
        return;
    }
    acceptance.preview = Some(previews[0]);
    acceptance.visual_entities = None;
    acceptance.loaded_epoch = Some(params.epoch.get());
    acceptance.load_operation = None;
    acceptance.started_at = Some(now);
}

#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct JointViewParams<'w, 's> {
    main_camera: Query<'w, 's, &'static mut Transform, (With<MainCamera>, Without<Camera3dRtt>)>,
    ui_roots: Query<'w, 's, &'static mut Node, Without<ChildOf>>,
    fixture_roots: Query<
        'w,
        's,
        &'static mut Visibility,
        (With<PerfFixtureMarker>, Without<Building3dVisual>),
    >,
    visuals: Query<'w, 's, (&'static Building3dVisual, &'static mut Visibility)>,
}

pub(crate) fn prepare_wall_door_joint_view_system(
    config: Res<PerfScenarioConfig>,
    acceptance: Res<WallDoorJointActualWindowAcceptance>,
    mut params: JointViewParams,
) {
    if !acceptance.enabled(&config) || acceptance.doors.is_empty() {
        return;
    }
    if let Ok(mut camera) = params.main_camera.single_mut() {
        let center = WorldMap::grid_to_world(CAMERA_GRID_CENTER.0, CAMERA_GRID_CENTER.1);
        camera.translation.x = center.x;
        camera.translation.y = center.y;
        camera.scale = Vec3::new(CAMERA_SCALE, CAMERA_SCALE, 1.0);
    }
    for mut node in &mut params.ui_roots {
        node.display = Display::None;
    }
    for mut visibility in &mut params.fixture_roots {
        *visibility = Visibility::Hidden;
    }
    let owners = acceptance
        .doors
        .iter()
        .map(|target| target.owner)
        .chain(acceptance.walls.iter().map(|target| target.owner))
        .collect::<HashSet<_>>();
    for (visual, mut visibility) in &mut params.visuals {
        *visibility = if owners.contains(&visual.owner) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

type DoorVisualQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Building3dVisual,
        &'static DoorPresentationState,
        &'static DoorPresentationAxis,
        &'static Door3dPresentationMode,
        &'static Mesh3d,
        &'static MeshMaterial3d<TopDownStructuralMaterial>,
        &'static GlobalTransform,
    ),
>;

type WallVisualQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Building3dVisual,
        &'static Wall3dPresentationState,
        &'static Mesh3d,
        &'static MeshMaterial3d<TopDownStructuralMaterial>,
        &'static GlobalTransform,
    ),
>;

type JointPreviewQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Sprite,
        &'static Anchor,
        &'static GlobalTransform,
        &'static InheritedVisibility,
    ),
    With<Blueprint>,
>;
type JointMainCameraQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Transform,
        &'static Camera,
        &'static GlobalTransform,
    ),
    (With<MainCamera>, Without<Camera3dRtt>),
>;

#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct JointProbeParams<'w, 's> {
    world_map: Res<'w, WorldMap>,
    window: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    camera: Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<Camera3dRtt>>,
    main_camera: JointMainCameraQuery<'w, 's>,
    previews: JointPreviewQuery<'w, 's>,
    epoch: Res<'w, WorldEpoch>,
    images: Res<'w, Assets<Image>>,
    buildings: Query<'w, 's, (&'static Building, &'static Transform)>,
    visibility: Query<
        'w,
        's,
        (
            &'static Visibility,
            &'static InheritedVisibility,
            &'static RenderLayers,
        ),
    >,
    handles: Res<'w, Building3dHandles>,
    meshes: Res<'w, Assets<Mesh>>,
    materials: Res<'w, Assets<TopDownStructuralMaterial>>,
    virtual_time: Res<'w, Time<Virtual>>,
    door_visuals: DoorVisualQuery<'w, 's>,
    wall_visuals: WallVisualQuery<'w, 's>,
    door_readiness: Res<'w, DoorAssetReadiness>,
    door_assets: Res<'w, ProductionDoorAssetPool>,
    door_materials: Res<'w, ProductionDoorMaterialPool>,
    wall_readiness: Res<'w, WallAssetReadiness>,
    wall_activation: Res<'w, WallProductionActivation>,
    wall_assets: Res<'w, ProductionWallAssetPool>,
    wall_materials: Res<'w, ProductionWallMaterialPool>,
}

pub(crate) fn publish_wall_door_joint_status_system(
    time: Res<Time<Real>>,
    config: Res<PerfScenarioConfig>,
    params: JointProbeParams,
    mut acceptance: ResMut<WallDoorJointActualWindowAcceptance>,
) {
    if !acceptance.enabled(&config)
        || acceptance.doors.len() != 9
        || acceptance.load_operation.is_some()
        || acceptance.published_at.is_some()
    {
        return;
    }
    let now = time.elapsed();
    let started = *acceptance.started_at.get_or_insert(now);
    if now.saturating_sub(started) < SETTLE {
        return;
    }
    let checkpoint = acceptance.checkpoint();
    let status = inspect_joint(&config, &params, &mut acceptance).unwrap_or_else(|reason| {
        json!({
            "schema_version": STATUS_SCHEMA_VERSION,
            "status": "failed",
            "phase": checkpoint.phase(),
            "generation": checkpoint.generation(),
            "reason": reason,
        })
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

fn wall_family_index(family: WallMeshFamily) -> usize {
    match family {
        WallMeshFamily::Isolated => 0,
        WallMeshFamily::End => 1,
        WallMeshFamily::Straight => 2,
        WallMeshFamily::Corner => 3,
        WallMeshFamily::TJunction => 4,
        WallMeshFamily::Cross => 5,
    }
}

fn inspect_joint(
    config: &PerfScenarioConfig,
    params: &JointProbeParams,
    acceptance: &mut WallDoorJointActualWindowAcceptance,
) -> Result<Value, String> {
    let checkpoint = acceptance.checkpoint();
    if !params.virtual_time.is_paused() {
        return Err("joint scene simulation is not paused".to_string());
    }
    let occupied: HashSet<_> = acceptance
        .doors
        .iter()
        .map(|target| target.grid)
        .chain(acceptance.walls.iter().map(|target| target.grid))
        .chain([PREVIEW_GRID])
        .collect();
    let door_identity = match &params.door_readiness.state {
        DoorAssetReadinessState::Eligible(identity) => identity,
        other => return Err(format!("Door candidate is not eligible: {other:?}")),
    };
    let door_assets = params
        .door_assets
        .resolved
        .as_ref()
        .filter(|assets| &assets.identity == door_identity)
        .ok_or_else(|| "Door production pool identity differs".to_string())?;
    if params.door_materials.identity.as_ref() != Some(door_identity) {
        return Err("Door production material identity differs".to_string());
    }
    let wall_identity = match &params.wall_readiness.state {
        WallAssetReadinessState::Eligible {
            asset_set_generation,
            authority,
            manifest_sha256,
        } => (*asset_set_generation, *authority, manifest_sha256),
        other => return Err(format!("Wall candidate is not eligible: {other:?}")),
    };
    let WallProductionActivationState::ReadyToApply {
        asset_set_generation,
        authority,
        manifest_sha256,
        ..
    } = &params.wall_activation.state
    else {
        return Err("Wall production activation is not ready".to_string());
    };
    if (*asset_set_generation, *authority, manifest_sha256) != wall_identity {
        return Err("Wall readiness and activation identities differ".to_string());
    }
    let wall_assets = params
        .wall_assets
        .resolved
        .as_ref()
        .filter(|assets| {
            assets.identity.asset_set_generation == wall_identity.0
                && assets.identity.authority == wall_identity.1
                && &assets.identity.manifest_sha256 == wall_identity.2
        })
        .ok_or_else(|| "Wall production pool identity differs".to_string())?;
    let formwork_meshes = wall_assets
        .formwork_meshes
        .as_ref()
        .ok_or_else(|| "Wall candidate has no formwork mesh inventory".to_string())?;
    if params.wall_materials.identity.as_ref() != Some(&wall_assets.identity) {
        return Err("Wall production material identity differs".to_string());
    }

    let expected_provisional = matches!(checkpoint, JointCheckpoint::Framed);
    let mut visual_entities = HashMap::new();
    let mut observations = Vec::new();
    for target in &acceptance.doors {
        let (building, transform) = params
            .buildings
            .get(target.owner)
            .map_err(|_| "joint Door owner is missing")?;
        if building.kind != BuildingType::Door
            || WorldMap::world_to_grid(transform.translation.truncate()) != target.grid
            || params.world_map.building_entity(target.grid) != Some(target.owner)
        {
            return Err("joint Door durable position or ownership differs".to_string());
        }
        let matches = params
            .door_visuals
            .iter()
            .filter(|(_, visual, ..)| visual.owner == target.owner)
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(format!("Door {:?} visual count differs", target.grid));
        }
        let (visual_entity, _, state, axis, mode, mesh, material, transform) = matches[0];
        require_visible(
            &params.visibility,
            visual_entity,
            &params.handles.render_layers,
        )?;
        let expected_axis = if matches!(
            checkpoint,
            JointCheckpoint::SupportChanged
                | JointCheckpoint::SupportRestored
                | JointCheckpoint::Loaded
        ) && target.owner == acceptance.doors[0].owner
        {
            DoorPresentationAxis::NorthSouth
        } else {
            target.initial_axis
        };
        let mesh_index = match target.state {
            DoorPresentationState::Closed => 0,
            DoorPresentationState::Open => 1,
            DoorPresentationState::Locked => 2,
        };
        if *state != target.state
            || *axis != expected_axis
            || *mode != Door3dPresentationMode::Production
            || mesh.0 != door_assets.meshes[mesh_index]
            || Some(&material.0) != params.door_materials.material.as_ref()
            || !params.meshes.contains(mesh.0.id())
            || !params.materials.contains(material.0.id())
        {
            return Err(format!(
                "Door {:?} production presentation differs",
                target.grid
            ));
        }
        visual_entities.insert(target.owner, visual_entity);
        observations.push((
            target.grid,
            transform.translation(),
            json!({
                "kind": "door", "owner": target.owner.to_bits().to_string(),
                "visual": visual_entity.to_bits().to_string(), "axis": format!("{axis:?}"),
                "state": format!("{state:?}"),
            }),
        ));
    }
    for target in &acceptance.walls {
        let (building, owner_transform) = params
            .buildings
            .get(target.owner)
            .map_err(|_| format!("Wall {:?} owner is missing", target.grid))?;
        let provisional = target.initially_provisional
            && (expected_provisional || target.grid == PERSISTENT_FORMWORK_GRID);
        if building.kind != BuildingType::Wall
            || building.is_provisional != provisional
            || WorldMap::world_to_grid(owner_transform.translation.truncate()) != target.grid
            || params.world_map.building_entity(target.grid) != Some(target.owner)
        {
            return Err(format!("Wall {:?} lifecycle state differs", target.grid));
        }
        let matches = params
            .wall_visuals
            .iter()
            .filter(|(_, visual, ..)| visual.owner == target.owner)
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(format!("Wall {:?} visual count differs", target.grid));
        }
        let (visual_entity, _, state, mesh, material, transform) = matches[0];
        require_visible(
            &params.visibility,
            visual_entity,
            &params.handles.render_layers,
        )?;
        let (x, y) = target.grid;
        let expected_topology = resolve_wall_topology(WallConnectionMask::from_neighbors(
            occupied.contains(&(x, y + 1)),
            occupied.contains(&(x, y - 1)),
            occupied.contains(&(x - 1, y)),
            occupied.contains(&(x + 1, y)),
        ));
        let index = wall_family_index(state.topology.family);
        let expected_mesh = if provisional {
            &formwork_meshes[index]
        } else {
            &wall_assets.meshes[index]
        };
        let expected_material = if provisional {
            params.wall_materials.provisional.as_ref()
        } else {
            params.wall_materials.complete.as_ref()
        };
        if state.mode != Wall3dPresentationMode::Production
            || state.topology != expected_topology
            || mesh.0 != *expected_mesh
            || Some(&material.0) != expected_material
            || !params.meshes.contains(mesh.0.id())
            || !params.materials.contains(material.0.id())
        {
            return Err(format!(
                "Wall {:?} production presentation differs",
                target.grid
            ));
        }
        visual_entities.insert(target.owner, visual_entity);
        observations.push((
            target.grid,
            transform.translation(),
            json!({
                "kind": "wall", "owner": target.owner.to_bits().to_string(),
                "visual": visual_entity.to_bits().to_string(), "provisional": provisional,
                "family": format!("{:?}", state.topology.family),
                "quarter_turns": state.topology.quarter_turns_y.get(),
            }),
        ));
    }
    if let Some(initial) = &acceptance.visual_entities
        && initial.iter().any(|(owner, visual)| {
            visual_entities
                .get(owner)
                .is_some_and(|next| next != visual)
        })
    {
        return Err("joint owner/visual identity changed between checkpoints".to_string());
    }
    acceptance.visual_entities = Some(visual_entities);

    let window = params
        .window
        .single()
        .map_err(|_| "joint gallery requires one primary window".to_string())?;
    let (camera, camera_transform) = params
        .camera
        .single()
        .map_err(|_| "joint gallery requires one Camera3dRtt".to_string())?;
    let width = window.physical_width();
    let height = window.physical_height();
    let mut projected = observations
        .iter()
        .map(|(grid, world, identity)| {
            project_client_point(camera, camera_transform, *world, width, height)
                .filter(|point| point.x >= 20.0 && point.x < width as f32 - 20.0
                    && point.y >= 20.0 && point.y < height as f32 - 20.0)
                .map(|point| json!({"grid": [grid.0, grid.1], "x": point.x, "y": point.y, "identity": identity}))
                .ok_or_else(|| format!("joint target {grid:?} cannot be projected"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let center = WorldMap::grid_to_world(CAMERA_GRID_CENTER.0, CAMERA_GRID_CENTER.1);
    let (main_camera, camera2d, camera2d_transform) = params
        .main_camera
        .single()
        .map_err(|_| "joint gallery requires one MainCamera".to_string())?;
    if main_camera.translation.x != center.x
        || main_camera.translation.y != center.y
        || main_camera.scale != Vec3::new(CAMERA_SCALE, CAMERA_SCALE, 1.0)
    {
        return Err("joint gallery camera differs".to_string());
    }
    let preview = acceptance.preview.ok_or("joint preview owner is missing")?;
    let (sprite, anchor, transform, visibility) = params
        .previews
        .get(preview)
        .map_err(|_| "joint preview shell is missing")?;
    let preview_axis = if matches!(
        checkpoint,
        JointCheckpoint::Framed | JointCheckpoint::Completed
    ) {
        DoorPresentationAxis::EastWest
    } else {
        DoorPresentationAxis::NorthSouth
    };
    let preview_image = match preview_axis {
        DoorPresentationAxis::EastWest => &door_assets.preview_ew,
        DoorPresentationAxis::NorthSouth => &door_assets.preview_ns,
    };
    if &sprite.image != preview_image
        || !params.images.contains(preview_image.id())
        || sprite.custom_size != Some(Vec2::splat(64.0))
        || *anchor != Anchor(Vec2::new(0.0, -0.25))
        || !visibility.get()
    {
        return Err("joint construction preview asset, anchor or visibility differs".to_string());
    }
    let point = camera2d
        .world_to_viewport(camera2d_transform, transform.translation())
        .map_err(|_| "joint preview cannot be projected")?;
    if point.x < 20.0
        || point.x >= width as f32 - 20.0
        || point.y < 20.0
        || point.y >= height as f32 - 20.0
    {
        return Err("joint preview is outside client".to_string());
    }
    projected.push(
        json!({"grid": [PREVIEW_GRID.0, PREVIEW_GRID.1], "x": point.x, "y": point.y,
        "identity": {"kind": "preview", "owner": preview.to_bits().to_string(),
            "visual": preview.to_bits().to_string(), "axis": format!("{preview_axis:?}")}}),
    );

    Ok(json!({
        "schema_version": STATUS_SCHEMA_VERSION,
        "status": "ready",
        "phase": checkpoint.phase(),
        "generation": checkpoint.generation(),
        "session_nonce": acceptance.nonce,
        "world_epoch": params.epoch.get(),
        "paused_load_complete": acceptance.loaded_epoch == Some(params.epoch.get()),
        "window": {"width": width, "height": height, "scale_factor": config.requested_window_scale_factor()},
        "render": {"backend": "vulkan", "rtt_quality": config.requested_rtt_quality().map(|quality| quality.as_str())},
        "candidate_identity": {
            "wall": {"asset_set_generation": wall_identity.0, "authority": format!("{:?}", wall_identity.1), "manifest_sha256": wall_identity.2},
            "door": {"asset_set_generation": door_identity.asset_set_generation, "authority": format!("{:?}", door_identity.authority), "manifest_sha256": door_identity.manifest_sha256},
        },
        "gallery": {
            "door_count": acceptance.doors.len(),
            "wall_count": acceptance.walls.len(),
            "provisional_wall_count": acceptance.walls.iter().filter(|wall| wall.initially_provisional && (expected_provisional || wall.grid == PERSISTENT_FORMWORK_GRID)).count(),
            "continuous_door_count": 4,
            "preview_count": 1,
            "both_axes": true,
            "owner_visual_identity_stable": true,
            "projected_targets": projected,
        },
    }))
}

fn require_visible(
    query: &Query<(&Visibility, &InheritedVisibility, &RenderLayers)>,
    entity: Entity,
    expected_layers: &RenderLayers,
) -> Result<(), String> {
    let (visibility, inherited, layers) = query
        .get(entity)
        .map_err(|_| "joint visual has no visibility state".to_string())?;
    if *visibility == Visibility::Hidden || !inherited.get() || !layers.intersects(expected_layers)
    {
        return Err("joint target visual is hidden".to_string());
    }
    Ok(())
}

fn project_client_point(
    camera: &Camera,
    camera_transform: &GlobalTransform,
    world_position: Vec3,
    physical_width: u32,
    physical_height: u32,
) -> Option<Vec2> {
    let viewport = camera.logical_viewport_size()?;
    let point = camera
        .world_to_viewport(camera_transform, world_position)
        .ok()?;
    let x = point.x / viewport.x * physical_width as f32;
    let normalized_y = point.y / viewport.y;
    let y =
        ((normalized_y - 0.5) * topdown_rtt_vertical_compensation() + 0.5) * physical_height as f32;
    (x.is_finite() && y.is_finite()).then_some(Vec2::new(x, y))
}

fn publish_failure(acceptance: &mut WallDoorJointActualWindowAcceptance, reason: &str) {
    if let Some(path) = acceptance.status_path.as_deref() {
        let checkpoint = acceptance.checkpoint();
        let _ = write_json(
            path,
            &json!({
                "schema_version": STATUS_SCHEMA_VERSION,
                "status": "failed",
                "phase": checkpoint.phase(),
                "generation": checkpoint.generation(),
                "reason": reason,
            }),
        );
    }
    acceptance.failed = true;
}

fn write_json(path: &Path, value: &Value) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("tmp");
    std::fs::write(
        &temporary,
        serde_json::to_vec_pretty(value).map_err(std::io::Error::other)?,
    )?;
    std::fs::rename(temporary, path)
}

pub(crate) fn configure_wall_door_joint_actual_window_probe(app: &mut App) {
    app.add_systems(
        Update,
        (
            setup_wall_door_joint_gallery_system,
            ApplyDeferred,
            drive_wall_door_joint_checkpoint_system,
            ApplyDeferred,
            prepare_wall_door_joint_view_system,
        )
            .chain()
            .before(crate::systems::visual::camera_sync::sync_camera3d_system),
    )
    .add_systems(
        PostUpdate,
        publish_wall_door_joint_status_system
            .after(DoorPresentationSyncSet)
            .after(WallPresentationApplySet)
            .after(TransformSystems::Propagate),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONCE: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn view_queries_do_not_alias_visibility_writes() {
        let mut app = App::new();
        app.init_resource::<PerfScenarioConfig>()
            .init_resource::<WallDoorJointActualWindowAcceptance>()
            .add_systems(Update, prepare_wall_door_joint_view_system);
        app.update();
    }

    #[test]
    fn acknowledgement_is_bound_to_joint_checkpoint_identity() {
        for checkpoint in JointCheckpoint::ALL {
            let value = json!({
                "schema_version": STATUS_SCHEMA_VERSION,
                "status": "captured",
                "session_nonce": NONCE,
                "phase": checkpoint.phase(),
                "generation": checkpoint.generation(),
            });
            assert!(acknowledgement_matches(&value, NONCE, checkpoint));
            assert!(!acknowledgement_matches(
                &value,
                NONCE,
                JointCheckpoint::ALL[(checkpoint.generation() as usize) % 3],
            ));
        }
    }
}
