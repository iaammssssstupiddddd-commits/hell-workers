//! Opt-in actual-window acceptance driver for building deconstruction.
//!
//! The driver is dormant unless `HW_NATIVE_DECONSTRUCTION_ACCEPTANCE_ARTIFACT`
//! is set. It drives the production intent, order, finalizer, room, energy,
//! save/load, and dashboard paths without synthetic desktop input. This keeps
//! native acceptance independent of compositor input-injection permissions
//! while still requiring a real window, renderer, and captured frame.

use std::collections::{HashMap, HashSet};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bevy::ecs::message::MessageCursor;
use bevy::input::InputSystems;
use bevy::prelude::*;
use bevy::render::renderer::RenderAdapterInfo;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::render::view::window::ExtractedWindows;
use bevy::render::{Render, RenderApp};
use bevy::window::PrimaryWindow;
use hw_core::WorldEpoch;
use hw_core::area::TaskArea;
use hw_core::familiar::{
    ActiveCommand, Familiar, FamiliarAiState, FamiliarOperation, FamiliarPolicy,
};
use hw_core::relationships::{
    CommandedBy, Commanding, LoadedIn, LoadedItems, ManagedBy, ManagedTasks, ParkedAt,
    RestAreaReservedFor, RestingIn, StoredIn, WorkingOn,
};
use hw_core::soul::{
    DamnedSoul, Destination, DreamState, IdleBehavior, IdleState, Path as SoulPath,
    RestAreaCooldown,
};
use hw_core::world::DoorState;
use hw_energy::{
    ConsumesFrom, GeneratesFor, GridConsumers, GridGenerators, PowerConsumer, PowerConsumerPolicy,
    PowerGenerator, PowerGrid, PowerGridAllocationSummary, PowerPriority, PowerShedReason,
    PowerSupplyState, SoulSpaConstructionCancelOutcome, SoulSpaConstructionCancelResult,
    SoulSpaPhase, SoulSpaSite, SoulSpaTile, Unpowered, YardPowerGrid,
};
use hw_jobs::mud_mixer::{MudMixerStorage, StoredByMixer};
use hw_jobs::{
    ActiveTaskIdentity, AssignedTask, BridgeMarker, Building, BuildingType, DeconstructData,
    DeconstructPhase, DeconstructionBlockReason, DeconstructionBlocker,
    DeconstructionCancelOutcome, DeconstructionCancelResult, DeconstructionCommitClaim,
    DeconstructionCommitOutcome, DeconstructionCommitRequest, DeconstructionCommitResult,
    DeconstructionDesignationOutcome, DeconstructionDesignationResult, DeconstructionOrder,
    DeconstructionOrders, DeconstructionPending, Designation, Door, GeneratePowerData,
    GeneratePowerPhase, HaulToBlueprintData, HaulToBpPhase, PlayerIssuedDesignation, Priority,
    RestArea, TargetDeconstructionRoot, TargetSoulSpaSite, TaskSlots, WorkType,
};
use hw_logistics::transport_request::{TransportPriority, TransportRequest, TransportRequestKind};
use hw_logistics::types::{BucketStorage, WheelbarrowParking};
use hw_logistics::zone::Stockpile;
use hw_logistics::{BelongsTo, Inventory, ResourceItem, ResourceType, Wheelbarrow};
use hw_ui::UiIntent;
use hw_ui::components::{
    LeftPanelMode, LeftPanelTabButton, LoadConfirmDialog, MenuAction, MenuButton, MenuState,
    OrdersSubMenu, TaskListItem, UiInputState, UiNodeRegistry, UiSlot,
};
use hw_ui::help::{HelpEntryId, HelpPanel, HelpPanelContent, HelpPanelState, HelpTopicId};
use hw_ui::panels::task_list::{
    PendingTaskCancellation, TaskActionButton, TaskActionButtonKind, TaskCancelKind,
    TaskDashboardActionState, TaskPriorityAdjustment, TaskPriorityTier,
};
use hw_visual::Building3dVisual;
use hw_world::{
    RoomBoundaryLookup, RoomDetectionState, RoomTileLookup, TerrainType, WorldMap, Yard,
};

use crate::app_contexts::TaskContext;
use crate::input_actions::{
    InputAction, InputModifiers, InputPreUpdateSet, ResolvedInputFrame,
    request_capture_from_resolved_actions_system,
};
use crate::interface::selection::{HoveredEntity, SelectedEntity};
use crate::interface::ui::panels::task_list::{
    TaskActionKind, TaskActionOutcome, TaskActionResult,
};
use crate::plugins::startup::Camera3dRtt;
use crate::systems::command::TaskMode;
use crate::systems::save::{
    SaveLoadOperation, SaveLoadOutcome, SaveLoadResult, SaveLoadState, SavePath,
};

use super::{DeconstructionHoverPreview, DeconstructionHoverStatus};

const ARTIFACT_ENV: &str = "HW_NATIVE_DECONSTRUCTION_ACCEPTANCE_ARTIFACT";
const RUN_ID_ENV: &str = "HW_NATIVE_DECONSTRUCTION_ACCEPTANCE_RUN_ID";
const RESULT_FILE: &str = "driver-result.json";
const SCREENSHOT_FILE: &str = "deconstruction-v1-v5.png";
const SAVE_FILE: &str = "runtime/saves/world.scn.ron";
const READY_FRAMES: u32 = 30;
const DRIVER_TIMEOUT: Duration = Duration::from_secs(180);
const MIN_SCREENSHOT_WIDTH: u32 = 640;
const MIN_SCREENSHOT_HEIGHT: u32 = 360;
const MAX_SCREENSHOT_BYTES: u64 = 16 * 1024 * 1024;
const NATIVE_FAMILIAR_NAME: &str = "C1 Native Acceptance Familiar";
const NATIVE_SOUL_LAZINESS: f32 = 0.314_159;
const NATIVE_WHEELBARROW_CAPACITY: usize = 97;

/// Adds one bounded V1-V5 sequence to the regular actual-window application.
/// This plugin is never enabled by default.
pub struct NativeDeconstructionAcceptancePlugin {
    artifact_dir: PathBuf,
    run_id: String,
}

impl NativeDeconstructionAcceptancePlugin {
    /// Returns an opt-in plugin when the artifact environment variable is set.
    pub fn try_from_process() -> Result<Option<Self>, String> {
        let Some(raw_path) = env::var_os(ARTIFACT_ENV) else {
            return Ok(None);
        };
        if raw_path.is_empty() {
            return Err(format!("{ARTIFACT_ENV} must not be empty"));
        }
        let artifact_dir = PathBuf::from(raw_path);
        if !artifact_dir.is_absolute() {
            return Err(format!("{ARTIFACT_ENV} must be an absolute path"));
        }
        if !artifact_dir.is_dir() {
            return Err(format!("{ARTIFACT_ENV} must name an existing directory"));
        }
        if env::var("HW_WINDOW_BACKEND").is_ok_and(|value| value.eq_ignore_ascii_case("headless")) {
            return Err(format!(
                "{ARTIFACT_ENV} cannot be combined with HW_WINDOW_BACKEND=headless"
            ));
        }
        let run_id = env::var(RUN_ID_ENV)
            .map_err(|_| format!("{RUN_ID_ENV} must be set to a fresh run identifier"))?;
        validate_run_id(&run_id)?;

        for owned_path in [
            artifact_dir.join(RESULT_FILE),
            artifact_dir.join(SCREENSHOT_FILE),
            artifact_dir.join(SAVE_FILE),
        ] {
            if owned_path.exists() {
                return Err(format!(
                    "native deconstruction artifact contains stale driver output: {}",
                    owned_path.display()
                ));
            }
        }

        Ok(Some(Self {
            artifact_dir,
            run_id,
        }))
    }
}

impl Plugin for NativeDeconstructionAcceptancePlugin {
    fn build(&self, app: &mut App) {
        let save_path = self.artifact_dir.join(SAVE_FILE);
        let render_evidence = NativeRenderEvidence::pending();
        app.insert_resource(SavePath::new(save_path.clone()));
        app.insert_resource(NativeDeconstructionAcceptance::new(
            self.artifact_dir.clone(),
            save_path,
            self.run_id.clone(),
            render_evidence.clone(),
            app.world(),
        ));
        crate::systems::save::register_load_reset_hook(
            app,
            "native-deconstruction-acceptance-witness",
            record_world_replace_reset_witness,
        );
        install_render_evidence(app, render_evidence);
        app.add_systems(
            PreUpdate,
            inject_native_resolved_input
                .in_set(InputPreUpdateSet::CaptureTransition)
                .before(request_capture_from_resolved_actions_system),
        );
        app.add_systems(
            PreUpdate,
            inject_native_menu_and_pointer_input
                .after(InputSystems)
                .after(bevy::ui::UiSystems::Focus)
                .before(crate::interface::ui::update_ui_input_state_system)
                .before(InputPreUpdateSet::CaptureRequest),
        );
        app.add_systems(PostUpdate, drive_native_deconstruction_acceptance);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RenderEnvironment {
    adapter_name: String,
    adapter_backend: String,
    display_handle: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum RenderEvidenceState {
    Pending,
    Ready(RenderEnvironment),
    Failed(String),
}

#[derive(Resource, Clone)]
struct NativeRenderEvidence(Arc<Mutex<RenderEvidenceState>>);

impl NativeRenderEvidence {
    fn pending() -> Self {
        Self(Arc::new(Mutex::new(RenderEvidenceState::Pending)))
    }

    fn snapshot(&self) -> RenderEvidenceState {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn replace_pending(&self, next: RenderEvidenceState) {
        let mut state = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if matches!(*state, RenderEvidenceState::Pending) {
            *state = next;
        }
    }
}

fn install_render_evidence(app: &mut App, evidence: NativeRenderEvidence) {
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        evidence.replace_pending(RenderEvidenceState::Failed(
            "RenderApp is unavailable for native deconstruction acceptance".to_owned(),
        ));
        return;
    };
    render_app
        .insert_resource(evidence)
        .add_systems(Render, observe_render_environment);
}

fn observe_render_environment(
    windows: Res<ExtractedWindows>,
    adapter_info: Res<RenderAdapterInfo>,
    evidence: Res<NativeRenderEvidence>,
) {
    if !matches!(evidence.snapshot(), RenderEvidenceState::Pending) {
        return;
    }
    let Some(primary) = windows.primary else {
        return;
    };
    let Some(window) = windows.windows.get(&primary) else {
        return;
    };
    evidence.replace_pending(RenderEvidenceState::Ready(RenderEnvironment {
        adapter_name: adapter_info.name.clone(),
        adapter_backend: adapter_info.backend.to_str().to_owned(),
        display_handle: format!("{:?}", window.handle.get_display_handle()),
    }));
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AcceptanceStage {
    WaitForWorld,
    AwaitV1OrdersMenu,
    AwaitV1Mode,
    AwaitV1Hover,
    AwaitV1PointerPress,
    AwaitV1PointerRelease,
    AwaitV1Designation,
    AwaitV1ProgressAndRoom,
    AwaitV2Commit,
    AwaitV2RoomReconcile,
    AwaitV2Structures,
    AwaitV3Commit,
    AwaitV3Reject,
    AwaitV4Ready,
    AwaitV4Commit,
    AwaitV4LampCommit,
    AwaitV4ConstructingTaskDashboardTab,
    AwaitV4ConstructingTaskDashboardReady,
    AwaitV4ConstructingTaskSelect,
    AwaitV4ConstructingCancelFirstPress,
    AwaitV4ConstructingCancelSecondPress,
    AwaitV4ConstructingCancel,
    AwaitV5SaveInput,
    AwaitV5Save,
    AwaitV5LoadInput,
    AwaitV5LoadConfirm,
    AwaitV5LoadButton,
    AwaitV5Load,
    AwaitV5StaleReplay,
    AwaitV5HelpCapture,
    AwaitV5CapturedPriority,
    AwaitV5HelpClosed,
    AwaitV5TaskDashboardTab,
    AwaitV5TaskDashboardReady,
    AwaitV5TaskSelect,
    AwaitV5PriorityButton,
    AwaitV5PriorityChange,
    AwaitV5Reassignment,
    AwaitV5CancelFirstPress,
    AwaitV5CancelSecondPress,
    AwaitV5Cancel,
    AwaitFinalHelp,
    AwaitScreenshot,
    Finished,
}

#[derive(Clone, Copy, Debug)]
struct CommitFixture {
    target: Entity,
    order: Entity,
    worker: Entity,
    identity: ActiveTaskIdentity,
}

#[derive(Debug)]
struct V3FacilityCommit {
    commit: CommitFixture,
    visual: Entity,
    footprint: Vec<(i32, i32)>,
}

#[derive(Debug)]
struct V3Fixture {
    tank: V3FacilityCommit,
    tank_companion: Entity,
    tank_water: Entity,
    tank_bucket: Entity,
    rest: V3FacilityCommit,
    rest_souls: [Entity; 2],
    parking: V3FacilityCommit,
    wheelbarrow: Entity,
    sand: Entity,
    mud: Entity,
    mixer: V3FacilityCommit,
    mixer_receiver: Entity,
    mixer_receiver_footprint: Vec<(i32, i32)>,
    mixer_mud: Vec<Entity>,
    mixer_water: Entity,
    mixer_reject: V3FacilityCommit,
    mixer_reject_sand: u32,
    wood_before: HashSet<Entity>,
    bone_before: HashSet<Entity>,
    rock_before: HashSet<Entity>,
    sand_before: HashSet<Entity>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum V2StructureCase {
    Door,
    Floor,
    Bridge,
    BridgeNoSafeRecovery,
}

impl V2StructureCase {
    const ALL: [Self; 4] = [
        Self::Door,
        Self::Floor,
        Self::Bridge,
        Self::BridgeNoSafeRecovery,
    ];
}

#[derive(Debug)]
struct V2StructureFixture {
    case: V2StructureCase,
    commit: CommitFixture,
    footprint: Vec<(i32, i32)>,
    visual: Entity,
    stacked_owner: Option<Entity>,
    resource_type: ResourceType,
    salvage_before: HashSet<Entity>,
    terrain_backup: Option<Vec<TerrainType>>,
}

type V1RoomPlacement = (Entity, (i32, i32), (i32, i32));

#[derive(Clone, Debug)]
struct V4Fixture {
    spa_target: Entity,
    spa_position: Vec2,
    spa: Option<CommitFixture>,
    spa_visual: Entity,
    spa_tiles: Vec<Entity>,
    yard: Entity,
    grid: Entity,
    power_worker: Entity,
    delivery_request: Entity,
    target_lamp_target: Entity,
    target_lamp_position: Vec2,
    target_lamp: Option<CommitFixture>,
    survivor_lamp: Entity,
    baseline_generator: Entity,
    bones_before_spa: HashSet<Entity>,
    bones_before_lamp: HashSet<Entity>,
    constructing: Option<V4ConstructingFixture>,
}

#[derive(Clone, Debug)]
struct V4ConstructingFixture {
    target: Entity,
    tiles: Vec<Entity>,
    visual: Entity,
    footprint: Vec<(i32, i32)>,
    request: Entity,
    worker: Entity,
    item: Entity,
    delivered: u32,
    bones_before: HashSet<Entity>,
    action_seen: bool,
}

#[derive(Clone, Copy, Debug)]
struct LoadedV5Fixture {
    target: Entity,
    order: Entity,
    familiar: Entity,
    worker: Entity,
    carrier: Entity,
}

#[derive(Clone, Debug)]
struct V5StaleSnapshot {
    target: Entity,
    order: Entity,
    familiar: Entity,
    worker: Entity,
    carrier: Entity,
    target_building: BuildingType,
    target_pending_order: Entity,
    target_map_owner: Entity,
    order_priority: u32,
    order_target: Entity,
    order_managed_by: Entity,
    resource_counts: HashMap<ResourceType, usize>,
}

#[derive(Default)]
struct FrameReceipts {
    designation: Vec<DeconstructionDesignationOutcome>,
    commits: Vec<DeconstructionCommitOutcome>,
    cancels: Vec<DeconstructionCancelOutcome>,
    task_actions: Vec<TaskActionOutcome>,
    soul_spa_cancels: Vec<SoulSpaConstructionCancelOutcome>,
    save_load: Vec<SaveLoadOutcome>,
}

#[derive(Resource)]
struct NativeDeconstructionAcceptance {
    artifact_dir: PathBuf,
    save_path: PathBuf,
    run_id: String,
    stage: AcceptanceStage,
    started_at: Instant,
    ready_frames: u32,
    pending_ui_release: Option<Entity>,
    v4_constructing_cancel_press_attempts: u8,
    native_relative_speed: Option<f32>,
    base_grid: Option<(i32, i32)>,
    room_target_grid: Option<(i32, i32)>,
    room_interior_grid: Option<(i32, i32)>,
    v1_target: Option<Entity>,
    v1_order: Option<Entity>,
    v1_familiar: Option<Entity>,
    v1_worker: Option<Entity>,
    v2_items_before: HashSet<Entity>,
    v2_structure_index: usize,
    v2_structure: Option<V2StructureFixture>,
    v3: Option<V3Fixture>,
    v4: Option<V4Fixture>,
    v5_grid: Option<(i32, i32)>,
    v5_old_request: Option<DeconstructionCommitRequest>,
    v5_loaded: Option<LoadedV5Fixture>,
    v5_stale_snapshot: Option<V5StaleSnapshot>,
    v5_cancel_action_seen: bool,
    epoch_before_load: u64,
    epoch_after_load: u64,
    world_replace_reset_witness: Option<Result<(), String>>,
    screenshot_requested: bool,
    banner: Option<Entity>,
    checks: [bool; 5],
    render_evidence: NativeRenderEvidence,
    designation_cursor: MessageCursor<DeconstructionDesignationOutcome>,
    commit_cursor: MessageCursor<DeconstructionCommitOutcome>,
    cancel_cursor: MessageCursor<DeconstructionCancelOutcome>,
    task_action_cursor: MessageCursor<TaskActionOutcome>,
    soul_spa_cancel_cursor: MessageCursor<SoulSpaConstructionCancelOutcome>,
    save_load_cursor: MessageCursor<SaveLoadOutcome>,
}

impl NativeDeconstructionAcceptance {
    fn new(
        artifact_dir: PathBuf,
        save_path: PathBuf,
        run_id: String,
        render_evidence: NativeRenderEvidence,
        world: &World,
    ) -> Self {
        Self {
            artifact_dir,
            save_path,
            run_id,
            stage: AcceptanceStage::WaitForWorld,
            started_at: Instant::now(),
            ready_frames: 0,
            pending_ui_release: None,
            v4_constructing_cancel_press_attempts: 0,
            native_relative_speed: None,
            base_grid: None,
            room_target_grid: None,
            room_interior_grid: None,
            v1_target: None,
            v1_order: None,
            v1_familiar: None,
            v1_worker: None,
            v2_items_before: HashSet::new(),
            v2_structure_index: 0,
            v2_structure: None,
            v3: None,
            v4: None,
            v5_grid: None,
            v5_old_request: None,
            v5_loaded: None,
            v5_stale_snapshot: None,
            v5_cancel_action_seen: false,
            epoch_before_load: 0,
            epoch_after_load: 0,
            world_replace_reset_witness: None,
            screenshot_requested: false,
            banner: None,
            checks: [false; 5],
            render_evidence,
            designation_cursor: current_cursor::<DeconstructionDesignationOutcome>(world),
            commit_cursor: current_cursor::<DeconstructionCommitOutcome>(world),
            cancel_cursor: current_cursor::<DeconstructionCancelOutcome>(world),
            task_action_cursor: current_cursor::<TaskActionOutcome>(world),
            soul_spa_cancel_cursor: current_cursor::<SoulSpaConstructionCancelOutcome>(world),
            save_load_cursor: current_cursor::<SaveLoadOutcome>(world),
        }
    }

    fn result_path(&self) -> PathBuf {
        self.artifact_dir.join(RESULT_FILE)
    }

    fn screenshot_path(&self) -> PathBuf {
        self.artifact_dir.join(SCREENSHOT_FILE)
    }
}

fn current_cursor<M: Message>(world: &World) -> MessageCursor<M> {
    world.resource::<Messages<M>>().get_cursor_current()
}

type NativeMenuButtonQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static MenuButton, &'static mut Interaction),
    (With<Button>, Without<TaskActionButton>),
>;
type NativeTaskButtonQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static TaskActionButton, &'static mut Interaction),
    (With<Button>, Without<MenuButton>),
>;
type NativeTaskItemQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static TaskListItem, &'static mut Interaction),
    (With<Button>, Without<MenuButton>, Without<TaskActionButton>),
>;
type NativeTaskTabQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static LeftPanelTabButton,
        &'static mut Interaction,
    ),
    (
        With<Button>,
        Without<MenuButton>,
        Without<TaskActionButton>,
        Without<TaskListItem>,
    ),
>;

/// Keeps the production logic schedule live while retaining the user's
/// pre-acceptance speed for the phases that must advance simulated work.
fn resume_native_simulation(world: &mut World, driver: &mut NativeDeconstructionAcceptance) {
    let mut time = world.resource_mut::<Time<Virtual>>();
    let speed = *driver
        .native_relative_speed
        .get_or_insert_with(|| time.relative_speed());
    time.set_relative_speed(speed);
    time.unpause();
}

/// Freezes ambient simulation without pausing `GameSystemSet::Logic`.
///
/// The V2-V4 cleanup matrix sends regular finalizer requests, which are owned
/// by the production logic set. A paused `Time<Virtual>` would suppress that
/// set and turn every request into an indefinite wait.
fn freeze_native_simulation(world: &mut World, driver: &mut NativeDeconstructionAcceptance) {
    let mut time = world.resource_mut::<Time<Virtual>>();
    driver
        .native_relative_speed
        .get_or_insert_with(|| time.relative_speed());
    time.set_relative_speed(0.0);
    time.unpause();
}

fn inject_native_resolved_input(
    mut driver: ResMut<NativeDeconstructionAcceptance>,
    help: Res<HelpPanelState>,
    mut resolved: ResMut<ResolvedInputFrame>,
) {
    let awaiting_help = matches!(
        driver.stage,
        AcceptanceStage::AwaitV5HelpCapture | AcceptanceStage::AwaitFinalHelp
    );
    let action = if awaiting_help && !help.open {
        Some(InputAction::OpenHelp)
    } else if driver.stage == AcceptanceStage::AwaitV5SaveInput {
        driver.stage = AcceptanceStage::AwaitV5Save;
        Some(InputAction::SaveGame)
    } else if driver.stage == AcceptanceStage::AwaitV5LoadInput {
        driver.stage = AcceptanceStage::AwaitV5LoadConfirm;
        Some(InputAction::RequestLoadGame)
    } else {
        None
    };
    if let Some(action) = action {
        resolved.replace(InputModifiers::default(), vec![action], None, false);
    }
}

fn inject_native_menu_and_pointer_input(
    mut driver: ResMut<NativeDeconstructionAcceptance>,
    mut menu_buttons: NativeMenuButtonQuery,
    mut task_buttons: NativeTaskButtonQuery,
    mut task_items: NativeTaskItemQuery,
    mut task_tabs: NativeTaskTabQuery,
    action_state: Res<TaskDashboardActionState>,
    mut pointer: ResMut<ButtonInput<MouseButton>>,
) {
    if let Some(entity) = driver.pending_ui_release.take() {
        if let Ok((_, _, mut interaction)) = menu_buttons.get_mut(entity) {
            *interaction = Interaction::None;
        } else if let Ok((_, _, mut interaction)) = task_buttons.get_mut(entity) {
            *interaction = Interaction::None;
        } else if let Ok((_, _, mut interaction)) = task_items.get_mut(entity) {
            *interaction = Interaction::None;
        } else if let Ok((_, _, mut interaction)) = task_tabs.get_mut(entity) {
            *interaction = Interaction::None;
        }
        return;
    }

    let wants_menu_action = matches!(
        driver.stage,
        AcceptanceStage::AwaitV1OrdersMenu
            | AcceptanceStage::AwaitV1Mode
            | AcceptanceStage::AwaitV5LoadButton
    );
    if wants_menu_action {
        let mut pressed = false;
        for (entity, button, mut interaction) in &mut menu_buttons {
            let matches_stage = match driver.stage {
                AcceptanceStage::AwaitV1OrdersMenu => {
                    matches!(button.0, MenuAction::ToggleOrders)
                }
                AcceptanceStage::AwaitV1Mode => matches!(
                    button.0,
                    MenuAction::SelectTaskMode(TaskMode::DesignateDeconstruct(None))
                ),
                AcceptanceStage::AwaitV5LoadButton => {
                    matches!(button.0, MenuAction::ConfirmLoadGame)
                }
                _ => false,
            };
            if matches_stage {
                *interaction = Interaction::Pressed;
                driver.pending_ui_release = Some(entity);
                pressed = true;
                break;
            }
        }
        if pressed && driver.stage == AcceptanceStage::AwaitV5LoadButton {
            driver.stage = AcceptanceStage::AwaitV5Load;
        }
    }

    let wants_task_dashboard_tab = matches!(
        driver.stage,
        AcceptanceStage::AwaitV4ConstructingTaskDashboardTab
            | AcceptanceStage::AwaitV5TaskDashboardTab
    );
    if wants_task_dashboard_tab {
        let mut pressed = false;
        for (entity, tab, mut interaction) in &mut task_tabs {
            if tab.0 == LeftPanelMode::TaskList {
                *interaction = Interaction::Pressed;
                driver.pending_ui_release = Some(entity);
                pressed = true;
                break;
            }
        }
        if pressed {
            driver.stage = match driver.stage {
                AcceptanceStage::AwaitV4ConstructingTaskDashboardTab => {
                    AcceptanceStage::AwaitV4ConstructingTaskDashboardReady
                }
                AcceptanceStage::AwaitV5TaskDashboardTab => {
                    AcceptanceStage::AwaitV5TaskDashboardReady
                }
                stage => stage,
            };
        }
    }

    let task_selection = match driver.stage {
        AcceptanceStage::AwaitV4ConstructingTaskSelect => driver
            .v4
            .as_ref()
            .and_then(|fixture| fixture.constructing.as_ref())
            .map(|fixture| fixture.request),
        AcceptanceStage::AwaitV5TaskSelect => driver.v5_loaded.map(|loaded| loaded.order),
        _ => None,
    };
    if let Some(target) = task_selection {
        let mut selected = false;
        for (entity, item, mut interaction) in &mut task_items {
            if item.0 == target {
                *interaction = Interaction::Pressed;
                driver.pending_ui_release = Some(entity);
                selected = true;
                break;
            }
        }
        if selected {
            driver.stage = match driver.stage {
                AcceptanceStage::AwaitV4ConstructingTaskSelect => {
                    AcceptanceStage::AwaitV4ConstructingCancelFirstPress
                }
                AcceptanceStage::AwaitV5TaskSelect => AcceptanceStage::AwaitV5PriorityButton,
                stage => stage,
            };
        }
    }

    let task_action = match driver.stage {
        AcceptanceStage::AwaitV4ConstructingCancelFirstPress
        | AcceptanceStage::AwaitV4ConstructingCancelSecondPress => driver
            .v4
            .as_ref()
            .and_then(|fixture| fixture.constructing.as_ref())
            .map(|fixture| TaskActionButton {
                target: fixture.request,
                expected_work_type: WorkType::Haul,
                kind: TaskActionButtonKind::Cancel(TaskCancelKind::SoulSpaSite(fixture.target)),
            }),
        AcceptanceStage::AwaitV5PriorityButton => driver.v5_loaded.map(|loaded| TaskActionButton {
            target: loaded.order,
            expected_work_type: WorkType::Deconstruct,
            kind: TaskActionButtonKind::AdjustPriority(TaskPriorityAdjustment::Decrease),
        }),
        AcceptanceStage::AwaitV5CancelFirstPress | AcceptanceStage::AwaitV5CancelSecondPress => {
            driver.v5_loaded.map(|loaded| TaskActionButton {
                target: loaded.order,
                expected_work_type: WorkType::Deconstruct,
                kind: TaskActionButtonKind::Cancel(TaskCancelKind::DeconstructionOrder),
            })
        }
        _ => None,
    };
    if let Some(expected) = task_action {
        if driver.stage == AcceptanceStage::AwaitV4ConstructingCancelSecondPress {
            let pending = PendingTaskCancellation {
                target: expected.target,
                expected_work_type: expected.expected_work_type,
                kind: match expected.kind {
                    TaskActionButtonKind::Cancel(kind) => kind,
                    TaskActionButtonKind::AdjustPriority(_) => {
                        return;
                    }
                },
            };
            if action_state.confirmation != Some(pending) {
                if driver.v4_constructing_cancel_press_attempts < 3 {
                    driver.v4_constructing_cancel_press_attempts += 1;
                    driver.stage = AcceptanceStage::AwaitV4ConstructingCancelFirstPress;
                }
                return;
            }
        }
        let mut pressed = false;
        for (entity, button, mut interaction) in &mut task_buttons {
            if *button == expected {
                *interaction = Interaction::Pressed;
                driver.pending_ui_release = Some(entity);
                pressed = true;
                break;
            }
        }
        if pressed {
            driver.stage = match driver.stage {
                AcceptanceStage::AwaitV4ConstructingCancelFirstPress => {
                    AcceptanceStage::AwaitV4ConstructingCancelSecondPress
                }
                AcceptanceStage::AwaitV4ConstructingCancelSecondPress => {
                    AcceptanceStage::AwaitV4ConstructingCancel
                }
                AcceptanceStage::AwaitV5PriorityButton => AcceptanceStage::AwaitV5PriorityChange,
                AcceptanceStage::AwaitV5CancelFirstPress => {
                    AcceptanceStage::AwaitV5CancelSecondPress
                }
                AcceptanceStage::AwaitV5CancelSecondPress => AcceptanceStage::AwaitV5Cancel,
                stage => stage,
            };
        }
    }

    match driver.stage {
        AcceptanceStage::AwaitV1PointerPress => pointer.press(MouseButton::Left),
        AcceptanceStage::AwaitV1PointerRelease => {
            pointer.release(MouseButton::Left);
            driver.stage = AcceptanceStage::AwaitV1Designation;
        }
        _ => {}
    }
}

/// Records the reset state at the only point where it can be observed without
/// a later frame legitimately repopulating hover or pause-overlay capture.
fn record_world_replace_reset_witness(world: &mut World) {
    let mut stale = Vec::new();
    if world.resource::<SelectedEntity>().0.is_some() {
        stale.push("SelectedEntity");
    }
    if world.resource::<HoveredEntity>().0.is_some() {
        stale.push("HoveredEntity");
    }
    if world.resource::<TaskContext>().0 != TaskMode::None {
        stale.push("TaskContext");
    }
    if *world.resource::<DeconstructionHoverPreview>() != DeconstructionHoverPreview::default() {
        stale.push("DeconstructionHoverPreview");
    }
    let ui_input = world.resource::<UiInputState>();
    if ui_input.pointer_over_ui
        || ui_input.text_input_focused
        || ui_input.text_input_consumed_keyboard
        || ui_input.world_input_captured
        || ui_input.world_input_capture_started
        || ui_input.foreground_capture_root.is_some()
    {
        stale.push("UiInputState");
    }
    if *world.resource::<HelpPanelState>() != HelpPanelState::default() {
        stale.push("HelpPanelState");
    }
    if !world
        .resource::<Messages<DeconstructionCommitRequest>>()
        .is_empty()
    {
        stale.push("DeconstructionCommitRequest");
    }

    let witness = if stale.is_empty() {
        Ok(())
    } else {
        Err(stale.join(", "))
    };
    if let Some(mut driver) = world.get_resource_mut::<NativeDeconstructionAcceptance>() {
        driver.world_replace_reset_witness = Some(witness);
    }
}

fn drive_native_deconstruction_acceptance(world: &mut World) {
    let Some(mut driver) = world.remove_resource::<NativeDeconstructionAcceptance>() else {
        return;
    };
    if driver.stage == AcceptanceStage::Finished {
        world.insert_resource(driver);
        return;
    }
    if driver.started_at.elapsed() > DRIVER_TIMEOUT {
        let reason = format!(
            "native deconstruction acceptance timed out during {:?}",
            driver.stage
        );
        fail_driver(world, &mut driver, &reason);
        world.insert_resource(driver);
        return;
    }

    let receipts = collect_receipts(world, &mut driver);
    if let Err(reason) = drive_stage(world, &mut driver, &receipts) {
        fail_driver(world, &mut driver, &reason);
    }
    world.insert_resource(driver);
}

fn collect_receipts(world: &World, driver: &mut NativeDeconstructionAcceptance) -> FrameReceipts {
    FrameReceipts {
        designation: driver
            .designation_cursor
            .read(world.resource::<Messages<DeconstructionDesignationOutcome>>())
            .copied()
            .collect(),
        commits: driver
            .commit_cursor
            .read(world.resource::<Messages<DeconstructionCommitOutcome>>())
            .copied()
            .collect(),
        cancels: driver
            .cancel_cursor
            .read(world.resource::<Messages<DeconstructionCancelOutcome>>())
            .copied()
            .collect(),
        task_actions: driver
            .task_action_cursor
            .read(world.resource::<Messages<TaskActionOutcome>>())
            .copied()
            .collect(),
        soul_spa_cancels: driver
            .soul_spa_cancel_cursor
            .read(world.resource::<Messages<SoulSpaConstructionCancelOutcome>>())
            .copied()
            .collect(),
        save_load: driver
            .save_load_cursor
            .read(world.resource::<Messages<SaveLoadOutcome>>())
            .cloned()
            .collect(),
    }
}

fn drive_stage(
    world: &mut World,
    driver: &mut NativeDeconstructionAcceptance,
    receipts: &FrameReceipts,
) -> Result<(), String> {
    match driver.stage {
        AcceptanceStage::WaitForWorld => wait_for_world(world, driver),
        AcceptanceStage::AwaitV1OrdersMenu => await_v1_orders_menu(world, driver),
        AcceptanceStage::AwaitV1Mode => await_v1_mode(world, driver),
        AcceptanceStage::AwaitV1Hover => await_v1_hover(world, driver),
        AcceptanceStage::AwaitV1PointerPress => await_v1_pointer_press(world, driver),
        AcceptanceStage::AwaitV1PointerRelease => Ok(()),
        AcceptanceStage::AwaitV1Designation => await_v1_designation(world, driver, receipts),
        AcceptanceStage::AwaitV1ProgressAndRoom => await_v1_progress_and_room(world, driver),
        AcceptanceStage::AwaitV2Commit => await_v2_commit(world, driver, receipts),
        AcceptanceStage::AwaitV2RoomReconcile => await_v2_room_reconcile(world, driver),
        AcceptanceStage::AwaitV2Structures => await_v2_structures(world, driver, receipts),
        AcceptanceStage::AwaitV3Commit => await_v3_commit(world, driver, receipts),
        AcceptanceStage::AwaitV3Reject => await_v3_reject(world, driver, receipts),
        AcceptanceStage::AwaitV4Ready => await_v4_ready(world, driver),
        AcceptanceStage::AwaitV4Commit => await_v4_commit(world, driver, receipts),
        AcceptanceStage::AwaitV4LampCommit => await_v4_lamp_commit(world, driver, receipts),
        AcceptanceStage::AwaitV4ConstructingTaskDashboardTab => Ok(()),
        AcceptanceStage::AwaitV4ConstructingTaskDashboardReady => await_task_dashboard_ready(
            world,
            driver,
            AcceptanceStage::AwaitV4ConstructingTaskSelect,
        ),
        AcceptanceStage::AwaitV4ConstructingTaskSelect
        | AcceptanceStage::AwaitV4ConstructingCancelFirstPress
        | AcceptanceStage::AwaitV4ConstructingCancelSecondPress => Ok(()),
        AcceptanceStage::AwaitV4ConstructingCancel => {
            await_v4_constructing_cancel(world, driver, receipts)
        }
        AcceptanceStage::AwaitV5SaveInput => Ok(()),
        AcceptanceStage::AwaitV5Save => await_v5_save(world, driver, receipts),
        AcceptanceStage::AwaitV5LoadInput => Ok(()),
        AcceptanceStage::AwaitV5LoadConfirm => await_v5_load_confirm(world, driver),
        AcceptanceStage::AwaitV5LoadButton => Ok(()),
        AcceptanceStage::AwaitV5Load => await_v5_load(world, driver, receipts),
        AcceptanceStage::AwaitV5StaleReplay => await_v5_stale_replay(world, driver, receipts),
        AcceptanceStage::AwaitV5HelpCapture => await_v5_help_capture(world, driver),
        AcceptanceStage::AwaitV5CapturedPriority => {
            await_v5_captured_priority(world, driver, receipts)
        }
        AcceptanceStage::AwaitV5HelpClosed => await_v5_help_closed(world, driver),
        AcceptanceStage::AwaitV5TaskDashboardTab => Ok(()),
        AcceptanceStage::AwaitV5TaskDashboardReady => {
            await_task_dashboard_ready(world, driver, AcceptanceStage::AwaitV5TaskSelect)
        }
        AcceptanceStage::AwaitV5TaskSelect | AcceptanceStage::AwaitV5PriorityButton => Ok(()),
        AcceptanceStage::AwaitV5PriorityChange => await_v5_priority_change(world, driver, receipts),
        AcceptanceStage::AwaitV5Reassignment => await_v5_reassignment(world, driver),
        AcceptanceStage::AwaitV5CancelFirstPress | AcceptanceStage::AwaitV5CancelSecondPress => {
            Ok(())
        }
        AcceptanceStage::AwaitV5Cancel => await_v5_cancel(world, driver, receipts),
        AcceptanceStage::AwaitFinalHelp => await_final_help(world, driver),
        AcceptanceStage::AwaitScreenshot => await_screenshot(world, driver),
        AcceptanceStage::Finished => Ok(()),
    }
}

fn wait_for_world(
    world: &mut World,
    driver: &mut NativeDeconstructionAcceptance,
) -> Result<(), String> {
    match driver.render_evidence.snapshot() {
        RenderEvidenceState::Failed(reason) => return Err(reason),
        RenderEvidenceState::Pending => {
            driver.ready_frames = 0;
            return Ok(());
        }
        RenderEvidenceState::Ready(_) => {}
    }
    if !native_world_is_ready(world) {
        driver.ready_frames = 0;
        return Ok(());
    }
    driver.ready_frames += 1;
    if driver.ready_frames < READY_FRAMES {
        return Ok(());
    }

    let base = find_clear_region(world, 23, 9)
        .ok_or_else(|| "could not reserve a clear 23x9 native acceptance region".to_owned())?;
    resume_native_simulation(world, driver);
    let (target, target_grid, interior_grid) = spawn_v1_room(world, base)?;
    driver.base_grid = Some(base);
    driver.room_target_grid = Some(target_grid);
    driver.room_interior_grid = Some(interior_grid);
    driver.v1_target = Some(target);
    position_camera_and_cursor(world, target_grid)?;
    driver.stage = AcceptanceStage::AwaitV1OrdersMenu;
    info!("NATIVE_DECONSTRUCTION_ACCEPTANCE: V1 room and target ready");
    Ok(())
}

fn await_v1_orders_menu(
    world: &mut World,
    driver: &mut NativeDeconstructionAcceptance,
) -> Result<(), String> {
    if *world.resource::<MenuState>() != MenuState::Orders {
        return Ok(());
    }
    let mut panels = world.query_filtered::<&Node, With<OrdersSubMenu>>();
    let panel = panels
        .single(world)
        .map_err(|_| "V1 Orders submenu is missing or duplicated".to_owned())?;
    if panel.display == Display::None {
        return Ok(());
    }
    driver.stage = AcceptanceStage::AwaitV1Mode;
    info!("NATIVE_DECONSTRUCTION_ACCEPTANCE: V1 Orders menu opened through its button");
    Ok(())
}

fn await_v1_mode(
    world: &mut World,
    driver: &mut NativeDeconstructionAcceptance,
) -> Result<(), String> {
    if !matches!(
        world.resource::<TaskContext>().0,
        TaskMode::DesignateDeconstruct(None)
    ) {
        return Ok(());
    }
    driver.stage = AcceptanceStage::AwaitV1Hover;
    info!("NATIVE_DECONSTRUCTION_ACCEPTANCE: V1 Deconstruct button entered production mode");
    Ok(())
}

fn native_world_is_ready(world: &mut World) -> bool {
    let map_ready = world.get_resource::<WorldMap>().is_some_and(|map| {
        !map.tile_entities.is_empty() && map.tile_entities.iter().all(Option::is_some)
    });
    let souls_ready = !world
        .query_filtered::<Entity, With<DamnedSoul>>()
        .iter(world)
        .collect::<Vec<_>>()
        .is_empty();
    let familiars_ready = !world
        .query_filtered::<Entity, With<Familiar>>()
        .iter(world)
        .collect::<Vec<_>>()
        .is_empty();
    let window_count = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .iter(world)
        .count();
    let ui_ready = world
        .get_resource::<UiNodeRegistry>()
        .and_then(|registry| registry.get_slot(UiSlot::AreaEditPreview))
        .is_some();
    map_ready && souls_ready && familiars_ready && window_count == 1 && ui_ready
}

fn find_clear_region(world: &World, width: i32, height: i32) -> Option<(i32, i32)> {
    use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};
    let map = world.resource::<WorldMap>();
    (6..MAP_HEIGHT - height - 6).find_map(|y| {
        (6..MAP_WIDTH - width - 6)
            .find(|&x| {
                (y..y + height).all(|cell_y| {
                    (x..x + width).all(|cell_x| {
                        let grid = (cell_x, cell_y);
                        map.is_walkable(cell_x, cell_y)
                            && map.building_entity(grid).is_none()
                            && map.floor_entity(grid).is_none()
                            && map.door_entity(cell_x, cell_y).is_none()
                    })
                })
            })
            .map(|x| (x, y))
    })
}

fn spawn_v1_room(world: &mut World, base: (i32, i32)) -> Result<V1RoomPlacement, String> {
    let shifted = |x: i32, y: i32| (base.0 + x, base.1 + y);
    let target_grid = shifted(4, 2);
    let door_grid = shifted(1, 4);
    let interior_grid = shifted(2, 2);

    let target = spawn_plain_building(world, BuildingType::Wall, target_grid);
    let mut floors = Vec::new();
    for x in 1..=3 {
        for y in 1..=3 {
            let grid = shifted(x, y);
            floors.push((grid, spawn_plain_building(world, BuildingType::Floor, grid)));
        }
    }

    let mut boundary = Vec::new();
    for x in 0..=4 {
        boundary.push(shifted(x, 0));
        boundary.push(shifted(x, 4));
    }
    for y in 0..=4 {
        boundary.push(shifted(0, y));
        boundary.push(shifted(4, y));
    }
    boundary.sort_unstable();
    boundary.dedup();
    let mut walls = Vec::new();
    for grid in boundary.iter().copied() {
        if grid == target_grid || grid == door_grid {
            continue;
        }
        walls.push((grid, spawn_plain_building(world, BuildingType::Wall, grid)));
    }
    let door = world
        .spawn((
            Building {
                kind: BuildingType::Door,
                is_provisional: false,
            },
            Door {
                state: DoorState::Closed,
            },
            Transform::from_translation(
                WorldMap::grid_to_world(door_grid.0, door_grid.1).extend(0.0),
            ),
            Name::new("Native C1 Room Door"),
        ))
        .id();
    world.flush();

    {
        let mut map = world.resource_mut::<WorldMap>();
        map.set_building_occupancy(target_grid, target);
        for (grid, floor) in floors {
            map.set_floor(grid, floor);
        }
        for (grid, wall) in walls {
            map.set_building_occupancy(grid, wall);
        }
        map.register_door(door_grid, door, DoorState::Closed);
    }
    world
        .resource_mut::<RoomDetectionState>()
        .mark_dirty_many(boundary.into_iter().chain([interior_grid]));
    Ok((target, target_grid, interior_grid))
}

fn spawn_plain_building(world: &mut World, kind: BuildingType, grid: (i32, i32)) -> Entity {
    world
        .spawn((
            Building {
                kind,
                is_provisional: false,
            },
            Transform::from_translation(WorldMap::grid_to_world(grid.0, grid.1).extend(0.0)),
            Name::new(format!("Native C1 {kind:?}")),
        ))
        .id()
}

fn position_camera_and_cursor(world: &mut World, grid: (i32, i32)) -> Result<(), String> {
    let target = WorldMap::grid_to_world(grid.0, grid.1);
    {
        let mut camera_query = world.query_filtered::<&mut Transform, With<Camera3dRtt>>();
        if let Ok(mut transform) = camera_query.single_mut(world) {
            transform.translation.x = target.x;
            transform.translation.z = -target.y;
        }
    }
    {
        let mut camera_query =
            world.query_filtered::<&mut Transform, With<hw_ui::camera::MainCamera>>();
        let mut camera = camera_query
            .single_mut(world)
            .map_err(|_| "native acceptance requires exactly one MainCamera".to_owned())?;
        camera.translation.x = target.x;
        camera.translation.y = target.y;
    }

    let mut window_query = world.query_filtered::<&mut Window, With<PrimaryWindow>>();
    let mut window = window_query
        .single_mut(world)
        .map_err(|_| "native acceptance requires exactly one primary window".to_owned())?;
    let cursor = Vec2::new(window.width() * 0.5, window.height() * 0.5);
    window.set_cursor_position(Some(cursor));
    Ok(())
}

fn await_v1_hover(
    world: &mut World,
    driver: &mut NativeDeconstructionAcceptance,
) -> Result<(), String> {
    let target = required(driver.v1_target, "V1 target")?;
    if !matches!(
        world.resource::<TaskContext>().0,
        TaskMode::DesignateDeconstruct(_)
    ) {
        return Ok(());
    }
    let preview = *world.resource::<DeconstructionHoverPreview>();
    if !matches!(
        preview.status,
        Some(DeconstructionHoverStatus::Available {
            target: preview_target,
            kind: BuildingType::Wall,
        }) if preview_target == target
    ) {
        return Ok(());
    }
    let preview_entity = world
        .resource::<UiNodeRegistry>()
        .get_slot(UiSlot::AreaEditPreview)
        .ok_or_else(|| "V1 AreaEditPreview slot disappeared".to_owned())?;
    let preview_node = world
        .get::<Node>(preview_entity)
        .ok_or_else(|| "V1 AreaEditPreview has no Node".to_owned())?;
    let preview_text = world
        .get::<Text>(preview_entity)
        .ok_or_else(|| "V1 AreaEditPreview has no Text".to_owned())?;
    if preview_node.display == Display::None || !preview_text.0.contains("Salvage: Wood x1") {
        return Ok(());
    }

    driver.stage = AcceptanceStage::AwaitV1PointerPress;
    info!("NATIVE_DECONSTRUCTION_ACCEPTANCE: V1 hover and salvage presentation passed");
    Ok(())
}

fn await_v1_pointer_press(
    world: &mut World,
    driver: &mut NativeDeconstructionAcceptance,
) -> Result<(), String> {
    if !matches!(
        world.resource::<TaskContext>().0,
        TaskMode::DesignateDeconstruct(Some(_))
    ) {
        return Ok(());
    }
    driver.stage = AcceptanceStage::AwaitV1PointerRelease;
    Ok(())
}

fn await_v1_designation(
    world: &mut World,
    driver: &mut NativeDeconstructionAcceptance,
    receipts: &FrameReceipts,
) -> Result<(), String> {
    let target = required(driver.v1_target, "V1 target")?;
    let Some(outcome) = receipts.designation.iter().find(|outcome| {
        matches!(
            outcome.result,
            DeconstructionDesignationResult::Designated {
                target: designated_target,
                ..
            } if designated_target == target
        )
    }) else {
        return Ok(());
    };
    let order = match outcome.result {
        DeconstructionDesignationResult::Designated {
            order,
            target: designated_target,
            ..
        } if designated_target == target => order,
        other => return Err(format!("V1 designation returned {other:?}")),
    };
    if world
        .get::<DeconstructionPending>(target)
        .is_none_or(|pending| pending.order != order)
    {
        return Err("V1 designation did not publish the canonical pending relation".to_owned());
    }
    let target_position = world
        .get::<Transform>(target)
        .ok_or_else(|| "V1 target lost Transform before production assignment".to_owned())?
        .translation
        .truncate();
    let familiar = world
        .spawn((
            Familiar {
                name: "C1 Native Production Familiar".to_owned(),
                ..default()
            },
            FamiliarOperation {
                max_controlled_soul: 1,
                ..default()
            },
            FamiliarPolicy::default(),
            ActiveCommand::default(),
            FamiliarAiState::SearchingTask,
            Destination(target_position),
            SoulPath::default(),
            Transform::from_translation((target_position - Vec2::new(16.0, 0.0)).extend(0.0)),
        ))
        .id();
    let worker = world
        .spawn((
            DamnedSoul::default(),
            DreamState::default(),
            IdleState::default(),
            AssignedTask::None,
            Destination(target_position),
            SoulPath::default(),
            Inventory::default(),
            CommandedBy(familiar),
            Visibility::Visible,
            Transform::from_translation((target_position - Vec2::new(16.0, 0.0)).extend(0.0)),
        ))
        .id();
    world.entity_mut(order).insert(ManagedBy(familiar));
    world.flush();
    driver.v1_order = Some(order);
    driver.v1_familiar = Some(familiar);
    driver.v1_worker = Some(worker);
    driver.stage = AcceptanceStage::AwaitV1ProgressAndRoom;
    Ok(())
}

fn await_v1_progress_and_room(
    world: &mut World,
    driver: &mut NativeDeconstructionAcceptance,
) -> Result<(), String> {
    let target = required(driver.v1_target, "V1 target")?;
    let order = required(driver.v1_order, "V1 order")?;
    let worker = required(driver.v1_worker, "V1 worker")?;
    let interior = required(driver.room_interior_grid, "V1 room interior")?;
    let progress_started = matches!(
        world.get::<AssignedTask>(worker),
        Some(AssignedTask::Deconstruct(DeconstructData {
            order: assigned_order,
            target: assigned_target,
            phase: DeconstructPhase::Dismantling { progress },
        })) if *assigned_order == order && *assigned_target == target && *progress > 0.0 && *progress < 1.0
    );
    if !progress_started {
        return Ok(());
    }
    if !world
        .resource::<RoomTileLookup>()
        .tile_to_room
        .contains_key(&interior)
    {
        return Ok(());
    }
    let preview = *world.resource::<DeconstructionHoverPreview>();
    if !matches!(
        preview.status,
        Some(DeconstructionHoverStatus::Rejected {
            target: Some(preview_target),
            reason: hw_jobs::DeconstructionDesignationRejectReason::Target(
                hw_jobs::DeconstructionRejectReason::AlreadyDesignated,
            ),
            ..
        }) if preview_target == target
    ) {
        return Ok(());
    }

    driver.v2_items_before = resource_entities(world, ResourceType::Wood);
    driver.checks[0] = true;
    driver.stage = AcceptanceStage::AwaitV2Commit;
    info!("NATIVE_DECONSTRUCTION_ACCEPTANCE: V1 PASS (production progress observed)");
    Ok(())
}

fn await_v2_commit(
    world: &mut World,
    driver: &mut NativeDeconstructionAcceptance,
    receipts: &FrameReceipts,
) -> Result<(), String> {
    let target = required(driver.v1_target, "V2 target")?;
    let order = required(driver.v1_order, "V2 order")?;
    let target_grid = required(driver.room_target_grid, "V2 target grid")?;
    let committed = receipts.commits.iter().any(|outcome| {
        outcome.order == order
            && outcome.target == target
            && outcome.result == DeconstructionCommitResult::Committed
    });
    if !committed {
        return Ok(());
    }
    if world.get_entity(target).is_ok() || world.get_entity(order).is_ok() {
        return Err("V2 committed target/order still exists".to_owned());
    }
    let map = world.resource::<WorldMap>();
    if map.building_entity(target_grid).is_some() || !map.is_walkable(target_grid.0, target_grid.1)
    {
        return Err("V2 wall cleanup did not restore owner-safe walkability".to_owned());
    }
    let wall_salvage = new_resource_entities(world, ResourceType::Wood, &driver.v2_items_before);
    if wall_salvage.len() != 1 {
        return Err(format!(
            "V2 wall cleanup expected exact Wood x1 salvage, observed {}",
            wall_salvage.len()
        ));
    }
    let salvage_grid = WorldMap::world_to_grid(
        world
            .get::<Transform>(wall_salvage[0])
            .ok_or_else(|| "V2 wall salvage lost Transform".to_owned())?
            .translation
            .truncate(),
    );
    if salvage_grid == target_grid {
        return Err("V2 wall salvage was placed inside the removed footprint".to_owned());
    }
    driver.stage = AcceptanceStage::AwaitV2RoomReconcile;
    Ok(())
}

fn await_v2_room_reconcile(
    world: &mut World,
    driver: &mut NativeDeconstructionAcceptance,
) -> Result<(), String> {
    let target_grid = required(driver.room_target_grid, "V2 target grid")?;
    let interior = required(driver.room_interior_grid, "V2 room interior")?;
    if world
        .resource::<RoomTileLookup>()
        .tile_to_room
        .contains_key(&interior)
        || !world
            .resource::<RoomBoundaryLookup>()
            .rooms_at(target_grid)
            .is_empty()
    {
        return Ok(());
    }
    validate_terminal_worker(
        world,
        required(driver.v1_worker, "V2 wall worker")?,
        "V2 Wall",
    )?;
    if let Some(worker) = driver.v1_worker.take() {
        let _ = world.despawn(worker);
    }
    if let Some(familiar) = driver.v1_familiar.take() {
        let _ = world.despawn(familiar);
    }
    for entity in new_resource_entities(world, ResourceType::Wood, &driver.v2_items_before) {
        let _ = world.despawn(entity);
    }
    freeze_native_simulation(world, driver);
    start_next_v2_structure(world, driver)?;
    driver.stage = AcceptanceStage::AwaitV2Structures;
    Ok(())
}

fn start_next_v2_structure(
    world: &mut World,
    driver: &mut NativeDeconstructionAcceptance,
) -> Result<(), String> {
    let case = V2StructureCase::ALL
        .get(driver.v2_structure_index)
        .copied()
        .ok_or_else(|| "V2 structure sequence advanced past its case table".to_owned())?;
    let base = required(driver.base_grid, "acceptance base grid")?;
    let lower_left = match case {
        V2StructureCase::Door => (base.0 + 7, base.1 + 5),
        V2StructureCase::Floor => (base.0 + 9, base.1 + 5),
        V2StructureCase::Bridge => (base.0 + 11, base.1 + 1),
        V2StructureCase::BridgeNoSafeRecovery => (base.0 + 14, base.1 + 1),
    };
    let kind = match case {
        V2StructureCase::Door => BuildingType::Door,
        V2StructureCase::Floor => BuildingType::Floor,
        V2StructureCase::Bridge | V2StructureCase::BridgeNoSafeRecovery => BuildingType::Bridge,
    };
    let footprint = if kind == BuildingType::Bridge {
        (0..5)
            .flat_map(|dy| (0..2).map(move |dx| (lower_left.0 + dx, lower_left.1 + dy)))
            .collect::<Vec<_>>()
    } else {
        vec![lower_left]
    };
    let terrain_backup = if case == V2StructureCase::BridgeNoSafeRecovery {
        let mut map = world.resource_mut::<WorldMap>();
        let backup = map.tiles.clone();
        map.tiles.fill(TerrainType::River);
        map.bump_obstacle_version();
        Some(backup)
    } else {
        None
    };
    let position = if kind == BuildingType::Bridge {
        WorldMap::grid_to_world(lower_left.0, lower_left.1)
            + Vec2::new(
                hw_core::constants::TILE_SIZE * 0.5,
                hw_core::constants::TILE_SIZE * 2.0,
            )
    } else {
        WorldMap::grid_to_world(lower_left.0, lower_left.1)
    };
    let mut target = world.spawn((
        Building {
            kind,
            is_provisional: false,
        },
        Transform::from_translation(position.extend(0.0)),
        Name::new(format!("Native C1 V2 {case:?}")),
    ));
    match kind {
        BuildingType::Door => {
            target.insert(Door {
                state: DoorState::Locked,
            });
        }
        BuildingType::Bridge => {
            target.insert(BridgeMarker);
        }
        BuildingType::Floor => {}
        _ => unreachable!("V2 structure kind table is exhaustive"),
    }
    let target = target.id();
    world.flush();
    {
        let mut map = world.resource_mut::<WorldMap>();
        match kind {
            BuildingType::Door => map.register_door(lower_left, target, DoorState::Locked),
            BuildingType::Floor => map.set_floor(lower_left, target),
            BuildingType::Bridge => {
                for &grid in &footprint {
                    let index = map
                        .pos_to_idx(grid.0, grid.1)
                        .ok_or_else(|| format!("V2 bridge grid is outside the map: {grid:?}"))?;
                    map.tiles[index] = TerrainType::River;
                    map.register_bridge_tile(grid, target);
                }
            }
            _ => unreachable!("V2 structure kind table is exhaustive"),
        }
    }
    let stacked_owner = (kind == BuildingType::Floor).then(|| {
        let owner = world
            .spawn(Name::new("Native C1 V2 Floor stacked owner"))
            .id();
        world
            .resource_mut::<WorldMap>()
            .set_building(lower_left, owner);
        owner
    });
    let visual = world.spawn(Building3dVisual { owner: target }).id();
    let commit = spawn_direct_commit_fixture(world, target, position, None, 0);
    let resource_type = match kind {
        BuildingType::Door => ResourceType::Wood,
        BuildingType::Floor => ResourceType::Bone,
        BuildingType::Bridge => ResourceType::Rock,
        _ => unreachable!("V2 structure kind table is exhaustive"),
    };
    let salvage_before = resource_entities(world, resource_type);
    write_commit(world, commit);
    driver.v2_structure = Some(V2StructureFixture {
        case,
        commit,
        footprint,
        visual,
        stacked_owner,
        resource_type,
        salvage_before,
        terrain_backup,
    });
    Ok(())
}

fn await_v2_structures(
    world: &mut World,
    driver: &mut NativeDeconstructionAcceptance,
    receipts: &FrameReceipts,
) -> Result<(), String> {
    let Some(fixture) = driver.v2_structure.take() else {
        return Err("V2 current structure fixture is missing".to_owned());
    };
    let Some(outcome) = receipts
        .commits
        .iter()
        .find(|outcome| outcome.order == fixture.commit.order)
    else {
        driver.v2_structure = Some(fixture);
        return Ok(());
    };
    let rejected = fixture.case == V2StructureCase::BridgeNoSafeRecovery;
    let expected_result = if rejected {
        DeconstructionCommitResult::NoSafeRecovery
    } else {
        DeconstructionCommitResult::Committed
    };
    if outcome.result != expected_result {
        return Err(format!(
            "V2 {:?} returned {:?}, expected {:?}",
            fixture.case, outcome.result, expected_result
        ));
    }
    let salvage = new_resource_entities(world, fixture.resource_type, &fixture.salvage_before);
    let expected_salvage = match fixture.case {
        V2StructureCase::Door | V2StructureCase::Floor => 1,
        V2StructureCase::Bridge => 3,
        V2StructureCase::BridgeNoSafeRecovery => 0,
    };
    if salvage.len() != expected_salvage {
        return Err(format!(
            "V2 {:?} expected exact {:?} x{}, observed {}",
            fixture.case,
            fixture.resource_type,
            expected_salvage,
            salvage.len()
        ));
    }
    if salvage.iter().any(|&entity| {
        world.get::<Transform>(entity).is_none_or(|transform| {
            fixture
                .footprint
                .contains(&WorldMap::world_to_grid(transform.translation.truncate()))
        })
    }) {
        return Err(format!(
            "V2 {:?} placed salvage inside its removed footprint",
            fixture.case
        ));
    }

    if rejected {
        if world.get_entity(fixture.commit.target).is_err()
            || world.get_entity(fixture.commit.order).is_err()
            || world
                .get::<DeconstructionBlocker>(fixture.commit.order)
                .is_none_or(|blocker| blocker.reason != DeconstructionBlockReason::NoSafeRecovery)
        {
            return Err("V2 Bridge rejection mutated the durable target/order".to_owned());
        }
        let map = world.resource::<WorldMap>();
        if fixture.footprint.iter().any(|&grid| {
            map.building_entity(grid) != Some(fixture.commit.target)
                || !map.bridged_tiles.contains(&grid)
                || !map.is_walkable(grid.0, grid.1)
        }) {
            return Err("V2 Bridge rejection changed the bridge footprint".to_owned());
        }
        validate_terminal_worker(world, fixture.commit.worker, "V2 Bridge reject")?;
        let backup = fixture
            .terrain_backup
            .ok_or_else(|| "V2 Bridge reject lost terrain backup".to_owned())?;
        {
            let mut map = world.resource_mut::<WorldMap>();
            for &grid in &fixture.footprint {
                if !map.clear_bridge_if_owned(grid, fixture.commit.target) {
                    return Err(format!("V2 Bridge reject cleanup lost owner at {grid:?}"));
                }
            }
            map.tiles = backup;
            map.bump_obstacle_version();
        }
        for entity in [
            fixture.commit.target,
            fixture.commit.order,
            fixture.commit.worker,
            fixture.visual,
        ] {
            let _ = world.despawn(entity);
        }
        world
            .resource_mut::<RoomDetectionState>()
            .mark_dirty_many(fixture.footprint.iter().copied());
    } else {
        if world.get_entity(fixture.commit.target).is_ok()
            || world.get_entity(fixture.commit.order).is_ok()
            || world.get_entity(fixture.visual).is_ok()
        {
            return Err(format!(
                "V2 {:?} left its target/order/visual alive",
                fixture.case
            ));
        }
        validate_terminal_worker(world, fixture.commit.worker, "V2 structure")?;
        let map = world.resource::<WorldMap>();
        match fixture.case {
            V2StructureCase::Door => {
                let grid = fixture.footprint[0];
                if map.building_entity(grid).is_some()
                    || map.door_entity(grid.0, grid.1).is_some()
                    || map.door_state(grid.0, grid.1).is_some()
                    || !map.is_walkable(grid.0, grid.1)
                {
                    return Err("V2 Door did not clear all map/cache layers".to_owned());
                }
            }
            V2StructureCase::Floor => {
                let grid = fixture.footprint[0];
                if map.floor_entity(grid).is_some()
                    || map.building_entity(grid) != fixture.stacked_owner
                {
                    return Err("V2 Floor did not preserve its stacked owner".to_owned());
                }
            }
            V2StructureCase::Bridge => {
                if fixture.footprint.iter().any(|&grid| {
                    map.building_entity(grid).is_some()
                        || map.bridged_tiles.contains(&grid)
                        || map.is_walkable(grid.0, grid.1)
                }) {
                    return Err("V2 Bridge did not restore river topology".to_owned());
                }
            }
            V2StructureCase::BridgeNoSafeRecovery => unreachable!(),
        }
        let _ = world.despawn(fixture.commit.worker);
        if let Some(owner) = fixture.stacked_owner {
            let grid = fixture.footprint[0];
            world
                .resource_mut::<WorldMap>()
                .clear_building_if_owned(grid, owner);
            let _ = world.despawn(owner);
        }
        for entity in salvage {
            let _ = world.despawn(entity);
        }
    }
    world.flush();

    driver.v2_structure_index += 1;
    if driver.v2_structure_index < V2StructureCase::ALL.len() {
        start_next_v2_structure(world, driver)?;
        return Ok(());
    }

    driver.checks[1] = true;
    let base = required(driver.base_grid, "acceptance base grid")?;
    driver.v3 = Some(spawn_v3_fixture(world, base)?);
    driver.stage = AcceptanceStage::AwaitV3Commit;
    info!("NATIVE_DECONSTRUCTION_ACCEPTANCE: V2 PASS (Wall/Door/Floor/Bridge/reject)");
    Ok(())
}

fn spawn_v3_facility(
    world: &mut World,
    kind: BuildingType,
    lower_left: (i32, i32),
) -> V3FacilityCommit {
    let position = WorldMap::grid_to_world(lower_left.0, lower_left.1)
        + Vec2::splat(hw_core::constants::TILE_SIZE * 0.5);
    let mut target = world.spawn((
        Building {
            kind,
            is_provisional: false,
        },
        Transform::from_translation(position.extend(0.0)),
        Name::new(format!("Native C1 V3 {kind:?}")),
    ));
    match kind {
        BuildingType::Tank => {
            target.insert(Stockpile {
                capacity: 50,
                resource_type: Some(ResourceType::Water),
            });
        }
        BuildingType::MudMixer => {
            target.insert((
                MudMixerStorage::default(),
                Stockpile {
                    capacity: hw_core::constants::MUD_MIXER_CAPACITY as usize,
                    resource_type: Some(ResourceType::Water),
                },
            ));
        }
        BuildingType::RestArea => {
            target.insert(RestArea { capacity: 4 });
        }
        BuildingType::WheelbarrowParking => {
            target.insert(WheelbarrowParking { capacity: 2 });
        }
        _ => unreachable!("V3 facility helper only supports facility targets"),
    }
    let target = target.id();
    let footprint = footprint_2x2(lower_left).to_vec();
    for &grid in &footprint {
        world
            .resource_mut::<WorldMap>()
            .set_building_occupancy(grid, target);
    }
    let visual = world.spawn(Building3dVisual { owner: target }).id();
    let commit = spawn_direct_commit_fixture(world, target, position, None, 0);
    V3FacilityCommit {
        commit,
        visual,
        footprint,
    }
}

fn spawn_v3_fixture(world: &mut World, base: (i32, i32)) -> Result<V3Fixture, String> {
    let tank = spawn_v3_facility(world, BuildingType::Tank, (base.0 + 7, base.1 + 1));
    let tank_position = world
        .get::<Transform>(tank.commit.target)
        .expect("V3 Tank transform")
        .translation;
    let tank_companion_grid = (base.0 + 6, base.1 + 1);
    let tank_companion = world
        .spawn((
            BucketStorage,
            BelongsTo(tank.commit.target),
            Stockpile {
                capacity: 10,
                resource_type: None,
            },
            Transform::from_translation(
                WorldMap::grid_to_world(tank_companion_grid.0, tank_companion_grid.1).extend(0.0),
            ),
            Name::new("Native C1 V3 Tank bucket storage"),
        ))
        .id();
    world
        .resource_mut::<WorldMap>()
        .set_stockpile(tank_companion_grid, tank_companion);
    let tank_water = world
        .spawn((
            ResourceItem(ResourceType::Water),
            StoredIn(tank.commit.target),
            Visibility::Hidden,
            Transform::from_translation(tank_position),
        ))
        .id();
    let tank_bucket = world
        .spawn((
            ResourceItem(ResourceType::BucketWater),
            BelongsTo(tank.commit.target),
            StoredIn(tank_companion),
            Visibility::Hidden,
            Transform::from_translation(tank_position),
        ))
        .id();

    let rest = spawn_v3_facility(world, BuildingType::RestArea, (base.0 + 10, base.1 + 1));
    let rest_souls = [
        world
            .spawn((
                RestingIn(rest.commit.target),
                IdleState {
                    behavior: IdleBehavior::Resting,
                    ..default()
                },
                SoulPath {
                    waypoints: vec![Vec2::ONE],
                    ..default()
                },
                Visibility::Hidden,
            ))
            .id(),
        world
            .spawn((
                RestAreaReservedFor(rest.commit.target),
                IdleState {
                    behavior: IdleBehavior::GoingToRest,
                    ..default()
                },
                SoulPath::default(),
                Visibility::Visible,
            ))
            .id(),
    ];

    let parking = spawn_v3_facility(
        world,
        BuildingType::WheelbarrowParking,
        (base.0 + 13, base.1 + 1),
    );
    let parking_position = world
        .get::<Transform>(parking.commit.target)
        .expect("V3 Parking transform")
        .translation;
    let wheelbarrow = world
        .spawn((
            ResourceItem(ResourceType::Wheelbarrow),
            Wheelbarrow { capacity: 8 },
            BelongsTo(parking.commit.target),
            ParkedAt(parking.commit.target),
            LoadedItems::default(),
            Visibility::Hidden,
            Transform::from_translation(parking_position),
            Name::new("Native C1 V3 loaded Wheelbarrow"),
        ))
        .id();
    let sand = world
        .spawn((
            ResourceItem(ResourceType::Sand),
            LoadedIn(wheelbarrow),
            Visibility::Hidden,
            Transform::from_translation(parking_position),
        ))
        .id();
    let mud = world
        .spawn((
            ResourceItem(ResourceType::StasisMud),
            LoadedIn(wheelbarrow),
            Visibility::Hidden,
            Transform::from_translation(parking_position),
        ))
        .id();

    let mixer = spawn_v3_facility(world, BuildingType::MudMixer, (base.0 + 16, base.1 + 1));
    *world
        .get_mut::<MudMixerStorage>(mixer.commit.target)
        .expect("V3 Mixer storage") = MudMixerStorage {
        sand: 2,
        rock: 3,
        mud: 2,
    };
    let mixer_mud = (0..2)
        .map(|_| {
            world
                .spawn((
                    ResourceItem(ResourceType::StasisMud),
                    StoredByMixer(mixer.commit.target),
                    Visibility::Hidden,
                    Transform::default(),
                ))
                .id()
        })
        .collect::<Vec<_>>();
    let mixer_water = world
        .spawn((
            ResourceItem(ResourceType::Water),
            StoredIn(mixer.commit.target),
            Visibility::Hidden,
            Transform::default(),
        ))
        .id();

    let receiver_lower_left = (base.0 + 19, base.1 + 1);
    let receiver_position = WorldMap::grid_to_world(receiver_lower_left.0, receiver_lower_left.1)
        + Vec2::splat(hw_core::constants::TILE_SIZE * 0.5);
    let mixer_receiver = world
        .spawn((
            Building {
                kind: BuildingType::MudMixer,
                is_provisional: false,
            },
            MudMixerStorage {
                sand: hw_core::constants::MUD_MIXER_CAPACITY.saturating_sub(2),
                rock: 0,
                mud: 0,
            },
            Stockpile {
                capacity: hw_core::constants::MUD_MIXER_CAPACITY as usize,
                resource_type: Some(ResourceType::Water),
            },
            Transform::from_translation(receiver_position.extend(0.0)),
            Name::new("Native C1 V3 Mixer receiver"),
        ))
        .id();
    let mixer_receiver_footprint = footprint_2x2(receiver_lower_left).to_vec();
    for &grid in &mixer_receiver_footprint {
        world
            .resource_mut::<WorldMap>()
            .set_building_occupancy(grid, mixer_receiver);
    }

    let mixer_reject = spawn_v3_facility(world, BuildingType::MudMixer, (base.0 + 7, base.1 + 5));
    world.flush();
    let available_after_success = world
        .query::<(&Building, &MudMixerStorage, Option<&DeconstructionPending>)>()
        .iter(world)
        .filter(|(building, _, pending)| {
            building.kind == BuildingType::MudMixer && !building.is_provisional && pending.is_none()
        })
        .map(|(_, storage, _)| hw_core::constants::MUD_MIXER_CAPACITY.saturating_sub(storage.sand))
        .sum::<u32>()
        .saturating_sub(2);
    let mixer_reject_sand = available_after_success.saturating_add(1);
    world
        .get_mut::<MudMixerStorage>(mixer_reject.commit.target)
        .expect("V3 rejected Mixer storage")
        .sand = mixer_reject_sand;

    let wood_before = resource_entities(world, ResourceType::Wood);
    let bone_before = resource_entities(world, ResourceType::Bone);
    let rock_before = resource_entities(world, ResourceType::Rock);
    let sand_before = resource_entities(world, ResourceType::Sand);
    for commit in [tank.commit, rest.commit, parking.commit, mixer.commit] {
        write_commit(world, commit);
    }

    Ok(V3Fixture {
        tank,
        tank_companion,
        tank_water,
        tank_bucket,
        rest,
        rest_souls,
        parking,
        wheelbarrow,
        sand,
        mud,
        mixer,
        mixer_receiver,
        mixer_receiver_footprint,
        mixer_mud,
        mixer_water,
        mixer_reject,
        mixer_reject_sand,
        wood_before,
        bone_before,
        rock_before,
        sand_before,
    })
}

fn await_v3_commit(
    world: &mut World,
    driver: &mut NativeDeconstructionAcceptance,
    receipts: &FrameReceipts,
) -> Result<(), String> {
    let Some(fixture) = driver.v3.take() else {
        return Err("V3 fixture is missing".to_owned());
    };
    let expected = [
        (
            "Tank",
            fixture.tank.commit.order,
            DeconstructionCommitResult::Committed,
        ),
        (
            "RestArea",
            fixture.rest.commit.order,
            DeconstructionCommitResult::Committed,
        ),
        (
            "WheelbarrowParking",
            fixture.parking.commit.order,
            DeconstructionCommitResult::Committed,
        ),
        (
            "MudMixer",
            fixture.mixer.commit.order,
            DeconstructionCommitResult::Committed,
        ),
    ];
    if expected.iter().any(|(_, order, _)| {
        !receipts
            .commits
            .iter()
            .any(|outcome| outcome.order == *order)
    }) {
        driver.v3 = Some(fixture);
        return Ok(());
    }
    for (label, order, expected_result) in expected {
        let outcome = receipts
            .commits
            .iter()
            .find(|outcome| outcome.order == order)
            .expect("V3 outcome presence was checked above");
        if outcome.result != expected_result {
            return Err(format!(
                "V3 {label} order {order:?} returned {:?}, expected {expected_result:?}",
                outcome.result
            ));
        }
    }

    // The rejection intentionally depends on the successful mixer's volatile
    // transfer filling the only receiver. Keep it out of the preceding batch:
    // finalizer ordering is entity-ID order, not fixture creation order.
    write_commit(world, fixture.mixer_reject.commit);
    driver.v3 = Some(fixture);
    driver.stage = AcceptanceStage::AwaitV3Reject;
    Ok(())
}

fn await_v3_reject(
    world: &mut World,
    driver: &mut NativeDeconstructionAcceptance,
    receipts: &FrameReceipts,
) -> Result<(), String> {
    let Some(fixture) = driver.v3.take() else {
        return Err("V3 rejection fixture is missing".to_owned());
    };
    let Some(outcome) = receipts
        .commits
        .iter()
        .find(|outcome| outcome.order == fixture.mixer_reject.commit.order)
    else {
        driver.v3 = Some(fixture);
        return Ok(());
    };
    if outcome.result != DeconstructionCommitResult::NoSafeRecovery {
        return Err(format!(
            "V3 MudMixer full-storage rejection order {:?} returned {:?}, expected NoSafeRecovery",
            fixture.mixer_reject.commit.order, outcome.result
        ));
    }
    complete_v3_cleanup(world, driver, fixture)
}

fn complete_v3_cleanup(
    world: &mut World,
    driver: &mut NativeDeconstructionAcceptance,
    fixture: V3Fixture,
) -> Result<(), String> {
    for (label, facility) in [
        ("Tank", &fixture.tank),
        ("RestArea", &fixture.rest),
        ("WheelbarrowParking", &fixture.parking),
        ("MudMixer", &fixture.mixer),
    ] {
        if world.get_entity(facility.commit.target).is_ok()
            || world.get_entity(facility.commit.order).is_ok()
            || world.get_entity(facility.visual).is_ok()
            || facility
                .footprint
                .iter()
                .any(|&grid| world.resource::<WorldMap>().building_entity(grid).is_some())
        {
            return Err(format!("V3 {label} cleanup left an owner/map entity"));
        }
        validate_terminal_worker(world, facility.commit.worker, &format!("V3 {label}"))?;
    }

    if world.get_entity(fixture.tank_companion).is_ok()
        || world
            .resource::<WorldMap>()
            .stockpile_entries()
            .any(|(_, &owner)| owner == fixture.tank_companion)
    {
        return Err("V3 Tank companion/cache survived cleanup".to_owned());
    }
    for item in [fixture.tank_water, fixture.tank_bucket] {
        if world.get_entity(item).is_err()
            || world.get::<StoredIn>(item).is_some()
            || world.get::<BelongsTo>(item).is_some()
            || world.get::<Visibility>(item) != Some(&Visibility::Visible)
        {
            return Err("V3 Tank did not recover water/bucket exactly".to_owned());
        }
    }
    for soul in fixture.rest_souls {
        if world.get::<RestingIn>(soul).is_some()
            || world.get::<RestAreaReservedFor>(soul).is_some()
            || world.get::<RestAreaCooldown>(soul).is_none()
            || world
                .get::<IdleState>(soul)
                .is_none_or(|state| state.behavior != IdleBehavior::Wandering)
            || world.get::<Visibility>(soul) != Some(&Visibility::Visible)
        {
            return Err("V3 RestArea did not release occupant/reservation state".to_owned());
        }
    }
    if world.get_entity(fixture.parking.commit.target).is_ok()
        || world.get::<ParkedAt>(fixture.wheelbarrow).is_some()
        || world.get::<BelongsTo>(fixture.wheelbarrow).is_some()
        || world.get::<LoadedIn>(fixture.sand).map(|owner| owner.0) != Some(fixture.wheelbarrow)
        || world.get::<LoadedIn>(fixture.mud).map(|owner| owner.0) != Some(fixture.wheelbarrow)
        || world.get::<Visibility>(fixture.wheelbarrow) != Some(&Visibility::Visible)
    {
        return Err("V3 parking cleanup did not preserve and expose loaded cargo".to_owned());
    }
    let receiver = world
        .get::<MudMixerStorage>(fixture.mixer_receiver)
        .ok_or_else(|| "V3 Mixer receiver disappeared".to_owned())?;
    if (receiver.sand, receiver.rock, receiver.mud)
        != (hw_core::constants::MUD_MIXER_CAPACITY, 0, 2)
        || fixture.mixer_mud.iter().any(|&entity| {
            world.get::<StoredByMixer>(entity).map(|owner| owner.0) != Some(fixture.mixer_receiver)
        })
        || world.get::<StoredIn>(fixture.mixer_water).is_some()
        || world.get::<Visibility>(fixture.mixer_water) != Some(&Visibility::Visible)
    {
        return Err("V3 Mixer did not transfer volatile storage and recover water".to_owned());
    }
    if world
        .get_entity(fixture.mixer_reject.commit.target)
        .is_err()
        || world.get_entity(fixture.mixer_reject.commit.order).is_err()
        || world
            .get::<MudMixerStorage>(fixture.mixer_reject.commit.target)
            .is_none_or(|storage| storage.sand != fixture.mixer_reject_sand)
        || world
            .get::<DeconstructionBlocker>(fixture.mixer_reject.commit.order)
            .is_none_or(|blocker| blocker.reason != DeconstructionBlockReason::NoSafeRecovery)
    {
        return Err("V3 full Mixer recovery rejection mutated its target/order".to_owned());
    }
    validate_terminal_worker(
        world,
        fixture.mixer_reject.commit.worker,
        "V3 Mixer no-capacity reject",
    )?;

    let wood = new_resource_entities(world, ResourceType::Wood, &fixture.wood_before);
    let bone = new_resource_entities(world, ResourceType::Bone, &fixture.bone_before);
    let rock = new_resource_entities(world, ResourceType::Rock, &fixture.rock_before);
    let grounded_sand = new_resource_entities(world, ResourceType::Sand, &fixture.sand_before);
    if wood.len() != 6 || !bone.is_empty() || rock.len() != 3 || !grounded_sand.is_empty() {
        return Err(format!(
            "V3 exact recovery mismatch: Wood={}, Bone={}, Rock={}, new Sand={}",
            wood.len(),
            bone.len(),
            rock.len(),
            grounded_sand.len()
        ));
    }
    let occupied = fixture
        .tank
        .footprint
        .iter()
        .chain(&fixture.rest.footprint)
        .chain(&fixture.parking.footprint)
        .chain(&fixture.mixer.footprint)
        .copied()
        .collect::<HashSet<_>>();
    if wood.iter().chain(&bone).chain(&rock).any(|&entity| {
        world.get::<Transform>(entity).is_none_or(|transform| {
            occupied.contains(&WorldMap::world_to_grid(transform.translation.truncate()))
        })
    }) {
        return Err("V3 fixed recovery was placed inside a removed facility".to_owned());
    }

    for grid in &fixture.mixer_reject.footprint {
        world
            .resource_mut::<WorldMap>()
            .clear_building_occupancy_if_owned(*grid, fixture.mixer_reject.commit.target);
    }
    for grid in &fixture.mixer_receiver_footprint {
        world
            .resource_mut::<WorldMap>()
            .clear_building_occupancy_if_owned(*grid, fixture.mixer_receiver);
    }
    let mut cleanup = vec![
        fixture.tank.commit.worker,
        fixture.tank_water,
        fixture.tank_bucket,
        fixture.rest.commit.worker,
        fixture.rest_souls[0],
        fixture.rest_souls[1],
        fixture.parking.commit.worker,
        fixture.sand,
        fixture.mud,
        fixture.wheelbarrow,
        fixture.mixer.commit.worker,
        fixture.mixer_water,
        fixture.mixer_receiver,
        fixture.mixer_reject.commit.target,
        fixture.mixer_reject.commit.order,
        fixture.mixer_reject.commit.worker,
        fixture.mixer_reject.visual,
    ];
    cleanup.extend(fixture.mixer_mud);
    cleanup.extend(wood);
    cleanup.extend(bone);
    cleanup.extend(rock);
    for entity in cleanup {
        let _ = world.despawn(entity);
    }
    world.flush();
    driver.checks[2] = true;
    let base = required(driver.base_grid, "acceptance base grid")?;
    driver.v4 = Some(spawn_v4_fixture(world, base)?);
    // V2/V3 intentionally hold virtual time at zero so their exact cleanup
    // fixtures cannot advance. V4 first waits for the real Power pipeline and
    // its visual mirror to settle, which needs regular fixed ticks.
    resume_native_simulation(world, driver);
    driver.stage = AcceptanceStage::AwaitV4Ready;
    info!("NATIVE_DECONSTRUCTION_ACCEPTANCE: V3 PASS (Tank/Mixer/Rest/Parking/reject)");
    Ok(())
}

fn spawn_v4_fixture(world: &mut World, base: (i32, i32)) -> Result<V4Fixture, String> {
    // The production topology reconciler owns PowerGrid through a Yard. An
    // orphan grid is deliberately removed on the next energy transaction, so
    // keep this isolated acceptance circuit inside an explicit temporary yard.
    let yard = world
        .spawn((
            Yard {
                min: WorldMap::grid_to_world(base.0 + 6, base.1),
                max: WorldMap::grid_to_world(base.0 + 21, base.1 + 7),
            },
            Name::new("Native C1 V4 temporary energy yard"),
        ))
        .id();
    let grid = world
        .spawn((
            PowerGrid::default(),
            YardPowerGrid(yard),
            Name::new("Native C1 V4 temporary power grid"),
        ))
        .id();
    let baseline_position = WorldMap::grid_to_world(base.0 + 8, base.1 + 2);
    let baseline_generator = world
        .spawn((
            PowerGenerator {
                current_output: 0.3,
                output_per_soul: 0.3,
            },
            GeneratesFor(grid),
            Transform::from_translation(baseline_position.extend(0.0)),
            Name::new("Native C1 V4 baseline generator"),
        ))
        .id();
    let spa_lower_left = (base.0 + 11, base.1 + 1);
    let spa_position = WorldMap::grid_to_world(spa_lower_left.0, spa_lower_left.1)
        + Vec2::splat(hw_core::constants::TILE_SIZE * 0.5);
    let spa_target = world
        .spawn((
            Building {
                kind: BuildingType::SoulSpa,
                is_provisional: false,
            },
            SoulSpaSite {
                phase: SoulSpaPhase::Operational,
                bones_required: 12,
                bones_delivered: 12,
                active_slots: 4,
            },
            GeneratesFor(grid),
            Transform::from_translation(spa_position.extend(0.0)),
            Name::new("Native C1 Operational Soul Spa"),
        ))
        .id();
    let spa_visual = world.spawn(Building3dVisual { owner: spa_target }).id();
    let mut spa_tiles = Vec::new();
    for tile_grid in footprint_2x2(spa_lower_left) {
        spa_tiles.push(
            world
                .spawn((
                    SoulSpaTile {
                        parent_site: spa_target,
                        grid_pos: tile_grid,
                    },
                    Designation {
                        work_type: WorkType::GeneratePower,
                    },
                    TaskSlots::new(1),
                    Transform::from_translation(
                        WorldMap::grid_to_world(tile_grid.0, tile_grid.1).extend(0.0),
                    ),
                ))
                .id(),
        );
        world
            .resource_mut::<WorldMap>()
            .set_building(tile_grid, spa_target);
    }
    let power_tile = spa_tiles[0];
    let power_tile_pos = world
        .get::<Transform>(power_tile)
        .expect("V4 Soul Spa tile transform")
        .translation
        .truncate();
    let power_worker = world
        .spawn((
            DamnedSoul {
                // The production GeneratePower executor rejects an empty Dream
                // reserve before the energy pipeline reads TaskWorkers.
                dream: hw_energy::constants::DREAM_GENERATE_FLOOR + 1.0,
                ..default()
            },
            AssignedTask::GeneratePower(GeneratePowerData {
                tile: power_tile,
                tile_pos: power_tile_pos,
                phase: GeneratePowerPhase::Generating,
            }),
            Destination(power_tile_pos),
            SoulPath::default(),
            Inventory::default(),
            ActiveTaskIdentity::new(power_tile, power_tile, WorkType::GeneratePower),
            WorkingOn(power_tile),
            Transform::from_translation(power_tile_pos.extend(0.0)),
            Name::new("Native C1 V4 Soul Spa power worker"),
        ))
        .id();
    let delivery_request = world
        .spawn((
            TransportRequest {
                kind: TransportRequestKind::DeliverToSoulSpa,
                anchor: spa_target,
                resource_type: ResourceType::Bone,
                issued_by: spa_target,
                priority: TransportPriority::Normal,
                stockpile_group: Vec::new(),
            },
            TargetSoulSpaSite(spa_target),
            Designation {
                work_type: WorkType::Haul,
            },
            TaskSlots::new(1),
            Name::new("Native C1 V4 Soul Spa delivery request"),
        ))
        .id();

    let target_lamp_grid = (base.0 + 15, base.1 + 2);
    let target_lamp_position = WorldMap::grid_to_world(target_lamp_grid.0, target_lamp_grid.1);
    let target_lamp_entity = world
        .spawn((
            Building {
                kind: BuildingType::OutdoorLamp,
                is_provisional: false,
            },
            PowerConsumer { demand: 0.2 },
            PowerConsumerPolicy {
                priority: PowerPriority::High,
            },
            ConsumesFrom(grid),
            Sprite::default(),
            Transform::from_translation(target_lamp_position.extend(0.0)),
            Name::new("Native C1 V4 target Outdoor Lamp"),
        ))
        .id();
    world
        .resource_mut::<WorldMap>()
        .set_building(target_lamp_grid, target_lamp_entity);
    let survivor_grid = (base.0 + 17, base.1 + 2);
    let survivor_position = WorldMap::grid_to_world(survivor_grid.0, survivor_grid.1);
    let survivor_lamp = world
        .spawn((
            Building {
                kind: BuildingType::OutdoorLamp,
                is_provisional: false,
            },
            PowerConsumer { demand: 0.2 },
            PowerConsumerPolicy {
                priority: PowerPriority::Low,
            },
            ConsumesFrom(grid),
            Sprite::default(),
            Transform::from_translation(survivor_position.extend(0.0)),
            Name::new("Native C1 V4 surviving Outdoor Lamp"),
        ))
        .id();
    world
        .resource_mut::<WorldMap>()
        .set_building(survivor_grid, survivor_lamp);
    world.flush();
    if world
        .get::<GridGenerators>(grid)
        .map_or(0, GridGenerators::len)
        != 2
        || world
            .get::<GridConsumers>(grid)
            .map_or(0, GridConsumers::len)
            != 2
    {
        return Err("V4 isolated energy relationships did not attach exactly".to_owned());
    }
    Ok(V4Fixture {
        spa_target,
        spa_position,
        spa: None,
        spa_visual,
        spa_tiles,
        yard,
        grid,
        power_worker,
        delivery_request,
        target_lamp_target: target_lamp_entity,
        target_lamp_position,
        target_lamp: None,
        survivor_lamp,
        baseline_generator,
        bones_before_spa: resource_entities(world, ResourceType::Bone),
        bones_before_lamp: HashSet::new(),
        constructing: None,
    })
}

fn await_v4_ready(
    world: &mut World,
    driver: &mut NativeDeconstructionAcceptance,
) -> Result<(), String> {
    let spa = {
        let fixture = driver
            .v4
            .as_mut()
            .ok_or_else(|| "V4 fixture is missing".to_owned())?;
        let spa_output = world
            .get::<PowerGenerator>(fixture.spa_target)
            .map_or(0.0, |generator| generator.current_output);
        let both_supplied = [fixture.target_lamp_target, fixture.survivor_lamp]
            .into_iter()
            .all(|lamp| {
                world.get::<PowerSupplyState>(lamp) == Some(&PowerSupplyState::Supplied)
                    && world.get::<Unpowered>(lamp).is_none()
                    && world
                        .get::<hw_core::visual_mirror::PoweredVisualState>(lamp)
                        .is_some_and(|state| state.is_powered)
                    && world
                        .get::<Sprite>(lamp)
                        .is_some_and(|sprite| sprite.color == Color::WHITE)
            });
        if spa_output < 1.0 || !both_supplied {
            return Ok(());
        }
        if fixture.spa.is_some() || fixture.target_lamp.is_some() {
            return Err(
                "V4 created a deconstruction fixture before its powered witness".to_owned(),
            );
        }
        let spa =
            spawn_direct_commit_fixture(world, fixture.spa_target, fixture.spa_position, None, 0);
        let target_lamp = spawn_direct_commit_fixture(
            world,
            fixture.target_lamp_target,
            fixture.target_lamp_position,
            None,
            0,
        );
        fixture.spa = Some(spa);
        fixture.target_lamp = Some(target_lamp);
        spa
    };
    write_commit(world, spa);
    driver.stage = AcceptanceStage::AwaitV4Commit;
    info!("NATIVE_DECONSTRUCTION_ACCEPTANCE: V4 powered presentation settled");
    Ok(())
}

fn await_v4_commit(
    world: &mut World,
    driver: &mut NativeDeconstructionAcceptance,
    receipts: &FrameReceipts,
) -> Result<(), String> {
    let fixture = driver
        .v4
        .as_mut()
        .ok_or_else(|| "V4 fixture is missing".to_owned())?;
    let spa = fixture
        .spa
        .ok_or_else(|| "V4 Soul Spa commit fixture is missing".to_owned())?;
    let target_lamp = fixture
        .target_lamp
        .ok_or_else(|| "V4 Outdoor Lamp commit fixture is missing".to_owned())?;
    let spa_committed = receipts.commits.iter().any(|outcome| {
        outcome.order == spa.order && outcome.result == DeconstructionCommitResult::Committed
    });
    if !spa_committed {
        return Ok(());
    }
    if world.get_entity(spa.target).is_ok()
        || world.get_entity(spa.order).is_ok()
        || world.get_entity(fixture.spa_visual).is_ok()
        || world.get_entity(fixture.delivery_request).is_ok()
        || fixture
            .spa_tiles
            .iter()
            .any(|&tile| world.get_entity(tile).is_ok())
    {
        return Err("V4 Operational Soul Spa root/order/tiles/request/visual survived".to_owned());
    }
    validate_terminal_worker(world, spa.worker, "V4 Soul Spa deconstruction")?;
    validate_terminal_worker(world, fixture.power_worker, "V4 Soul Spa power worker")?;
    if world
        .get::<GridGenerators>(fixture.grid)
        .map_or(0, GridGenerators::len)
        != 1
        || world
            .get::<GridConsumers>(fixture.grid)
            .map_or(0, GridConsumers::len)
            != 2
    {
        return Err("V4 Soul Spa energy reverse relationships did not reconcile".to_owned());
    }
    let spa_bones = new_resource_entities(world, ResourceType::Bone, &fixture.bones_before_spa);
    if spa_bones.len() != 6 {
        return Err(format!(
            "V4 Soul Spa expected exact Bone x6 salvage, observed {}",
            spa_bones.len()
        ));
    }
    let summary = world
        .get::<PowerGridAllocationSummary>(fixture.grid)
        .ok_or_else(|| "V4 Soul Spa removal produced no grid summary".to_owned())?;
    if (summary.generation - 0.3).abs() > f32::EPSILON
        || summary.consumer_count != 2
        || summary.supplied_count != 1
        || summary.shed_count != 1
        || world.get::<PowerSupplyState>(target_lamp.target) != Some(&PowerSupplyState::Supplied)
        || world.get::<Unpowered>(target_lamp.target).is_some()
        || world.get::<PowerSupplyState>(fixture.survivor_lamp)
            != Some(&PowerSupplyState::Shed {
                reason: PowerShedReason::InsufficientGeneration,
            })
        || world.get::<Unpowered>(fixture.survivor_lamp).is_none()
        || world
            .get::<hw_core::visual_mirror::PoweredVisualState>(fixture.survivor_lamp)
            .is_none_or(|state| state.is_powered)
        || world
            .get::<Sprite>(fixture.survivor_lamp)
            .is_none_or(|sprite| sprite.color != Color::srgba(0.4, 0.4, 0.4, 1.0))
    {
        return Err("V4 Soul Spa removal did not shed the surviving Lamp in-frame".to_owned());
    }
    fixture.bones_before_lamp = resource_entities(world, ResourceType::Bone);
    write_commit(world, target_lamp);
    driver.stage = AcceptanceStage::AwaitV4LampCommit;
    Ok(())
}

fn await_v4_lamp_commit(
    world: &mut World,
    driver: &mut NativeDeconstructionAcceptance,
    receipts: &FrameReceipts,
) -> Result<(), String> {
    let fixture = driver
        .v4
        .as_mut()
        .ok_or_else(|| "V4 fixture is missing".to_owned())?;
    let target_lamp = fixture
        .target_lamp
        .ok_or_else(|| "V4 Outdoor Lamp commit fixture is missing".to_owned())?;
    let committed = receipts.commits.iter().any(|outcome| {
        outcome.order == target_lamp.order
            && outcome.result == DeconstructionCommitResult::Committed
    });
    if !committed {
        return Ok(());
    }
    if world.get_entity(target_lamp.target).is_ok() || world.get_entity(target_lamp.order).is_ok() {
        return Err("V4 target Lamp survived its commit".to_owned());
    }
    validate_terminal_worker(world, target_lamp.worker, "V4 Outdoor Lamp")?;
    let lamp_bones = new_resource_entities(world, ResourceType::Bone, &fixture.bones_before_lamp);
    if lamp_bones.len() != 1 {
        return Err(format!(
            "V4 Outdoor Lamp expected exact Bone x1 salvage, observed {}",
            lamp_bones.len()
        ));
    }
    let summary = world
        .get::<PowerGridAllocationSummary>(fixture.grid)
        .ok_or_else(|| "V4 Lamp removal produced no grid summary".to_owned())?;
    if summary.consumer_count != 1
        || summary.supplied_count != 1
        || summary.shed_count != 0
        || world.get::<PowerSupplyState>(fixture.survivor_lamp) != Some(&PowerSupplyState::Supplied)
        || world.get::<Unpowered>(fixture.survivor_lamp).is_some()
        || world
            .get::<hw_core::visual_mirror::PoweredVisualState>(fixture.survivor_lamp)
            .is_none_or(|state| !state.is_powered)
        || world
            .get::<Sprite>(fixture.survivor_lamp)
            .is_none_or(|sprite| sprite.color != Color::WHITE)
    {
        return Err("V4 Lamp removal did not restore survivor power/visual in-frame".to_owned());
    }

    fixture.constructing = Some(spawn_v4_constructing_fixture(
        world,
        required(driver.base_grid, "acceptance base grid")?,
    ));
    // Keep the constructing request and its row stable while the real task
    // dashboard interaction is exercised. The owner cancellation system is
    // not time-gated, so it still runs while virtual time is held at zero.
    freeze_native_simulation(world, driver);
    driver.stage = AcceptanceStage::AwaitV4ConstructingTaskDashboardTab;
    Ok(())
}

fn await_task_dashboard_ready(
    world: &World,
    driver: &mut NativeDeconstructionAcceptance,
    next_stage: AcceptanceStage,
) -> Result<(), String> {
    if *world.resource::<LeftPanelMode>() != LeftPanelMode::TaskList {
        return Ok(());
    }
    driver.stage = next_stage;
    Ok(())
}

fn spawn_v4_constructing_fixture(world: &mut World, base: (i32, i32)) -> V4ConstructingFixture {
    let lower_left = (base.0 + 14, base.1 + 5);
    let footprint = footprint_2x2(lower_left).to_vec();
    let center = WorldMap::grid_to_world(lower_left.0, lower_left.1)
        + Vec2::splat(hw_core::constants::TILE_SIZE * 0.5);
    let delivered = 3;
    let target = world
        .spawn((
            Building {
                kind: BuildingType::SoulSpa,
                is_provisional: false,
            },
            SoulSpaSite {
                phase: SoulSpaPhase::Constructing,
                bones_required: 12,
                bones_delivered: delivered,
                active_slots: 4,
            },
            Transform::from_translation(center.extend(0.0)),
            Name::new("Native C1 V4 Constructing Soul Spa"),
        ))
        .id();
    let tiles = footprint
        .iter()
        .copied()
        .map(|grid| {
            world
                .spawn((
                    SoulSpaTile {
                        parent_site: target,
                        grid_pos: grid,
                    },
                    Transform::from_translation(
                        WorldMap::grid_to_world(grid.0, grid.1).extend(0.0),
                    ),
                ))
                .id()
        })
        .collect::<Vec<_>>();
    for &grid in &footprint {
        world.resource_mut::<WorldMap>().set_building(grid, target);
    }
    let visual = world.spawn(Building3dVisual { owner: target }).id();
    let request = world
        .spawn((
            TransportRequest {
                kind: TransportRequestKind::DeliverToSoulSpa,
                anchor: target,
                resource_type: ResourceType::Bone,
                issued_by: target,
                priority: TransportPriority::Normal,
                stockpile_group: Vec::new(),
            },
            TargetSoulSpaSite(target),
            Designation {
                work_type: WorkType::Haul,
            },
            TaskSlots::new(1),
            Transform::from_translation(center.extend(0.0)),
            Name::new("Native C1 V4 Constructing Spa delivery"),
        ))
        .id();
    let item = world.spawn(ResourceItem(ResourceType::Bone)).id();
    let worker = world
        .spawn((
            DamnedSoul::default(),
            AssignedTask::HaulToBlueprint(HaulToBlueprintData {
                item,
                blueprint: target,
                phase: HaulToBpPhase::GoingToItem,
            }),
            Destination(center),
            SoulPath::default(),
            Inventory::default(),
            ActiveTaskIdentity::new(request, request, WorkType::Haul),
            WorkingOn(request),
            Transform::from_translation(center.extend(0.0)),
            Name::new("Native C1 V4 Constructing Spa worker"),
        ))
        .id();
    world.flush();
    V4ConstructingFixture {
        target,
        tiles,
        visual,
        footprint,
        request,
        worker,
        item,
        delivered,
        bones_before: resource_entities(world, ResourceType::Bone),
        action_seen: false,
    }
}

fn await_v4_constructing_cancel(
    world: &mut World,
    driver: &mut NativeDeconstructionAcceptance,
    receipts: &FrameReceipts,
) -> Result<(), String> {
    let fixture = driver
        .v4
        .as_mut()
        .and_then(|fixture| fixture.constructing.as_mut())
        .ok_or_else(|| "V4 Constructing Soul Spa fixture is missing".to_owned())?;
    if world
        .get::<SoulSpaSite>(fixture.target)
        .is_some_and(|site| site.phase != SoulSpaPhase::Constructing)
    {
        return Err(
            "V4 Constructing Soul Spa became operational before dashboard cancellation".to_owned(),
        );
    }
    fixture.action_seen |= receipts.task_actions.iter().any(|outcome| {
        outcome.entity == fixture.request
            && outcome.action == TaskActionKind::Cancel
            && outcome.result == TaskActionResult::AwaitingOwnerOutcome
    });
    let Some(outcome) = receipts
        .soul_spa_cancels
        .iter()
        .find(|outcome| outcome.target == fixture.target)
    else {
        return Ok(());
    };
    if !fixture.action_seen
        || outcome.result
            != (SoulSpaConstructionCancelResult::Canceled {
                refunded_bones: fixture.delivered,
            })
    {
        return Err("V4 dashboard did not route Constructing Spa cancel to its owner".to_owned());
    }
    if world.get_entity(fixture.target).is_ok()
        || world.get_entity(fixture.visual).is_ok()
        || world.get_entity(fixture.request).is_ok()
        || fixture
            .tiles
            .iter()
            .any(|&tile| world.get_entity(tile).is_ok())
        || fixture
            .footprint
            .iter()
            .any(|&grid| world.resource::<WorldMap>().building_entity(grid).is_some())
        || world.get_entity(fixture.item).is_err()
    {
        return Err("V4 Constructing Spa cancellation left owner/request/map debris".to_owned());
    }
    validate_terminal_worker(world, fixture.worker, "V4 Constructing Soul Spa")?;
    let refunded = new_resource_entities(world, ResourceType::Bone, &fixture.bones_before);
    if refunded.len() != fixture.delivered as usize {
        return Err(format!(
            "V4 Constructing Spa expected exact Bone x{} refund, observed {}",
            fixture.delivered,
            refunded.len()
        ));
    }

    let mut v4 = driver
        .v4
        .take()
        .ok_or_else(|| "V4 fixture disappeared during cleanup".to_owned())?;
    let constructing = v4
        .constructing
        .take()
        .ok_or_else(|| "V4 constructing fixture disappeared during cleanup".to_owned())?;
    let spa = v4
        .spa
        .ok_or_else(|| "V4 Soul Spa commit fixture disappeared during cleanup".to_owned())?;
    let target_lamp = v4
        .target_lamp
        .ok_or_else(|| "V4 Outdoor Lamp commit fixture disappeared during cleanup".to_owned())?;
    let survivor_grid = world
        .get::<Transform>(v4.survivor_lamp)
        .map(|transform| WorldMap::world_to_grid(transform.translation.truncate()));
    if let Some(grid) = survivor_grid {
        world
            .resource_mut::<WorldMap>()
            .clear_building_if_owned(grid, v4.survivor_lamp);
    }
    let mut cleanup = vec![
        spa.worker,
        v4.power_worker,
        target_lamp.worker,
        v4.survivor_lamp,
        v4.baseline_generator,
        v4.grid,
        v4.yard,
        constructing.worker,
        constructing.item,
    ];
    cleanup.extend(new_resource_entities(
        world,
        ResourceType::Bone,
        &v4.bones_before_spa,
    ));
    for entity in cleanup {
        let _ = world.despawn(entity);
    }
    world.flush();
    driver.checks[3] = true;
    prepare_v5_save(world, driver)?;
    driver.stage = AcceptanceStage::AwaitV5SaveInput;
    info!("NATIVE_DECONSTRUCTION_ACCEPTANCE: V4 PASS (Operational/Constructing Spa + Lamp)");
    Ok(())
}

fn prepare_v5_save(
    world: &mut World,
    driver: &mut NativeDeconstructionAcceptance,
) -> Result<(), String> {
    if *world.resource::<SaveLoadState>() != SaveLoadState::Idle {
        return Err("V5 save/load dispatcher was busy before save".to_owned());
    }
    world.resource_mut::<Time<Virtual>>().pause();
    let base = required(driver.base_grid, "acceptance base grid")?;
    let target_grid = (base.0 + 19, base.1 + 3);
    let target_position = WorldMap::grid_to_world(target_grid.0, target_grid.1);
    let target = spawn_plain_building(world, BuildingType::Wall, target_grid);
    world
        .resource_mut::<WorldMap>()
        .set_building_occupancy(target_grid, target);

    let familiar = Familiar {
        name: NATIVE_FAMILIAR_NAME.to_owned(),
        ..default()
    };
    let familiar_entity = world
        .spawn((
            familiar,
            FamiliarOperation {
                max_controlled_soul: 1,
                ..default()
            },
            FamiliarPolicy::default(),
            Commanding::default(),
            ManagedTasks::default(),
            TaskArea::from_points(
                WorldMap::grid_to_world(base.0, base.1),
                WorldMap::grid_to_world(base.0 + 22, base.1 + 8),
            ),
            ActiveCommand::default(),
            FamiliarAiState::SearchingTask,
            Destination(target_position),
            SoulPath::default(),
            Transform::from_translation(
                WorldMap::grid_to_world(target_grid.0 - 1, target_grid.1).extend(0.0),
            ),
        ))
        .id();
    let order = world
        .spawn((
            DeconstructionOrder,
            Designation {
                work_type: WorkType::Deconstruct,
            },
            PlayerIssuedDesignation,
            Priority(10),
            TaskSlots::new(1),
            TargetDeconstructionRoot(target),
            ManagedBy(familiar_entity),
            Transform::from_translation(target_position.extend(0.0)),
            Name::new("Native C1 Persistent Deconstruction Order"),
        ))
        .id();
    let identity = ActiveTaskIdentity::new(order, order, WorkType::Deconstruct);
    let worker = world
        .spawn((
            DamnedSoul {
                laziness: NATIVE_SOUL_LAZINESS,
                motivation: 1.0,
                ..default()
            },
            DreamState::default(),
            IdleState::default(),
            AssignedTask::Deconstruct(DeconstructData {
                order,
                target,
                phase: DeconstructPhase::Dismantling { progress: 0.25 },
            }),
            identity,
            WorkingOn(order),
            CommandedBy(familiar_entity),
            Inventory::default(),
            Destination(target_position),
            SoulPath::default(),
            Visibility::Visible,
            Transform::from_translation(
                WorldMap::grid_to_world(target_grid.0 - 1, target_grid.1).extend(0.0),
            ),
            Name::new("Native C1 Persistent Soul"),
        ))
        .id();
    world.flush();
    let current_epoch = world.resource::<WorldEpoch>().get();
    world.entity_mut(target).insert((
        DeconstructionPending { order },
        DeconstructionCommitClaim {
            world_epoch: current_epoch,
            order,
        },
    ));

    let parking_position = WorldMap::grid_to_world(base.0 + 19, base.1 + 6);
    let parking = world
        .spawn((
            WheelbarrowParking {
                capacity: NATIVE_WHEELBARROW_CAPACITY,
            },
            Transform::from_translation(parking_position.extend(0.0)),
            Name::new("Native C1 Persistent Parking"),
        ))
        .id();
    let carrier = world
        .spawn((
            ResourceItem(ResourceType::Wheelbarrow),
            Wheelbarrow {
                capacity: NATIVE_WHEELBARROW_CAPACITY,
            },
            BelongsTo(parking),
            LoadedItems::default(),
            Transform::from_translation(parking_position.extend(0.0)),
            Name::new("Native C1 Persistent Wheelbarrow"),
        ))
        .id();
    world.spawn((
        ResourceItem(ResourceType::StasisMud),
        LoadedIn(carrier),
        Transform::from_translation(WorldMap::grid_to_world(1, 1).extend(0.0)),
        Name::new("Native C1 Persistent Cargo"),
    ));
    world.flush();

    world.resource_mut::<SelectedEntity>().0 = Some(target);
    world.resource_mut::<HoveredEntity>().0 = Some(target);
    world.resource_mut::<TaskContext>().0 = TaskMode::DesignateDeconstruct(None);
    *world.resource_mut::<DeconstructionHoverPreview>() = DeconstructionHoverPreview {
        cursor: Some(Vec2::splat(24.0)),
        status: Some(DeconstructionHoverStatus::Available {
            target,
            kind: BuildingType::Wall,
        }),
    };
    world.resource_mut::<UiInputState>().world_input_captured = true;
    world.resource_mut::<HelpPanelState>().open = true;

    let old_request = DeconstructionCommitRequest {
        world_epoch: world.resource::<WorldEpoch>().get(),
        worker,
        identity,
        order,
        target,
    };
    driver.v5_grid = Some(target_grid);
    driver.v5_old_request = Some(old_request);
    driver.epoch_before_load = world.resource::<WorldEpoch>().get();
    Ok(())
}

fn await_v5_save(
    world: &mut World,
    driver: &mut NativeDeconstructionAcceptance,
    receipts: &FrameReceipts,
) -> Result<(), String> {
    let Some(outcome) = receipts
        .save_load
        .iter()
        .find(|outcome| outcome.operation == SaveLoadOperation::Save)
    else {
        return Ok(());
    };
    if outcome.result != SaveLoadResult::Succeeded {
        return Err(format!("V5 save returned {:?}", outcome.result));
    }
    if fs::metadata(&driver.save_path)
        .map_err(|error| format!("V5 save artifact is unavailable: {error}"))?
        .len()
        == 0
    {
        return Err("V5 save artifact is empty".to_owned());
    }
    if *world.resource::<SaveLoadState>() != SaveLoadState::Idle {
        return Err("V5 save/load dispatcher stayed busy after save".to_owned());
    }
    driver.stage = AcceptanceStage::AwaitV5LoadInput;
    Ok(())
}

fn await_v5_load_confirm(
    world: &mut World,
    driver: &mut NativeDeconstructionAcceptance,
) -> Result<(), String> {
    let visible = world
        .query_filtered::<&Node, With<LoadConfirmDialog>>()
        .iter(world)
        .any(|node| node.display != Display::None);
    if !visible {
        return Ok(());
    }
    driver.stage = AcceptanceStage::AwaitV5LoadButton;
    info!("NATIVE_DECONSTRUCTION_ACCEPTANCE: V5 load confirmation dialog visible");
    Ok(())
}

fn await_v5_load(
    world: &mut World,
    driver: &mut NativeDeconstructionAcceptance,
    receipts: &FrameReceipts,
) -> Result<(), String> {
    let Some(outcome) = receipts
        .save_load
        .iter()
        .find(|outcome| outcome.operation == SaveLoadOperation::Load)
    else {
        return Ok(());
    };
    if outcome.result != SaveLoadResult::Succeeded {
        return Err(format!("V5 load returned {:?}", outcome.result));
    }
    let expected_epoch = driver.epoch_before_load.wrapping_add(1);
    let actual_epoch = world.resource::<WorldEpoch>().get();
    if actual_epoch != expected_epoch {
        return Err(format!(
            "V5 load advanced WorldEpoch to {actual_epoch}, expected {expected_epoch}"
        ));
    }
    driver.epoch_after_load = actual_epoch;
    let loaded = validate_loaded_v5(world, driver)?;
    driver.v5_stale_snapshot = Some(capture_v5_stale_snapshot(world, &loaded)?);
    driver.v5_loaded = Some(loaded);
    let old_request = required(driver.v5_old_request, "V5 old commit request")?;
    resume_native_simulation(world, driver);
    world.write_message(old_request);
    driver.stage = AcceptanceStage::AwaitV5StaleReplay;
    Ok(())
}

fn validate_loaded_v5(
    world: &mut World,
    driver: &NativeDeconstructionAcceptance,
) -> Result<LoadedV5Fixture, String> {
    let grid = required(driver.v5_grid, "V5 grid")?;
    let target = world
        .resource::<WorldMap>()
        .building_entity(grid)
        .ok_or_else(|| "V5 loaded WorldMap lost the target owner".to_owned())?;
    if world
        .get::<Building>(target)
        .is_none_or(|building| building.kind != BuildingType::Wall)
    {
        return Err("V5 loaded target is not the persisted Wall".to_owned());
    }
    let order = world
        .get::<DeconstructionOrders>(target)
        .and_then(|orders| orders.iter().copied().next())
        .ok_or_else(|| "V5 loaded target has no durable deconstruction order".to_owned())?;
    if world.get::<Priority>(order).map(|priority| priority.0) != Some(10)
        || world
            .get::<TargetDeconstructionRoot>(order)
            .map(|relation| relation.0)
            != Some(target)
        || world
            .get::<DeconstructionPending>(target)
            .is_none_or(|pending| pending.order != order)
        || world.get::<DeconstructionCommitClaim>(target).is_some()
    {
        return Err("V5 loaded order/pending/claim normalization is invalid".to_owned());
    }

    let familiar = world
        .query::<(Entity, &Familiar)>()
        .iter(world)
        .find_map(|(entity, familiar)| (familiar.name == NATIVE_FAMILIAR_NAME).then_some(entity))
        .ok_or_else(|| "V5 loaded acceptance Familiar is missing".to_owned())?;
    if world.get::<ManagedBy>(order).map(|owner| owner.0) != Some(familiar) {
        return Err("V5 loaded order lost its Familiar owner".to_owned());
    }
    let worker = world
        .query::<(Entity, &DamnedSoul)>()
        .iter(world)
        .find_map(|(entity, soul)| (soul.laziness == NATIVE_SOUL_LAZINESS).then_some(entity))
        .ok_or_else(|| "V5 loaded acceptance Soul is missing".to_owned())?;
    if !matches!(world.get::<AssignedTask>(worker), Some(AssignedTask::None))
        || world.get::<ActiveTaskIdentity>(worker).is_some()
        || world.get::<WorkingOn>(worker).is_some()
        || world.get::<CommandedBy>(worker).map(|owner| owner.0) != Some(familiar)
    {
        return Err("V5 loaded Soul retained runtime task state or lost its roster".to_owned());
    }

    let carrier = world
        .query::<(Entity, &Wheelbarrow)>()
        .iter(world)
        .find_map(|(entity, carrier)| {
            (carrier.capacity == NATIVE_WHEELBARROW_CAPACITY).then_some(entity)
        })
        .ok_or_else(|| "V5 loaded acceptance wheelbarrow is missing".to_owned())?;
    let carrier_position = world
        .get::<Transform>(carrier)
        .ok_or_else(|| "V5 loaded wheelbarrow has no Transform".to_owned())?
        .translation;
    let cargo_grounded = world
        .query::<(Entity, &ResourceItem, &Transform, Option<&LoadedIn>)>()
        .iter(world)
        .any(|(_, item, transform, loaded_in)| {
            item.0 == ResourceType::StasisMud
                && loaded_in.is_none()
                && transform.translation == carrier_position
        });
    if !cargo_grounded
        || world
            .get::<LoadedItems>(carrier)
            .is_some_and(|items| !items.is_empty())
    {
        return Err("V5 load did not ground wheelbarrow cargo at the carrier".to_owned());
    }

    match &driver.world_replace_reset_witness {
        Some(Ok(())) => {}
        Some(Err(stale)) => {
            return Err(format!(
                "V5 world replacement retained runtime/UI references: {stale}"
            ));
        }
        None => {
            return Err("V5 world replacement reset witness did not run".to_owned());
        }
    }

    Ok(LoadedV5Fixture {
        target,
        order,
        familiar,
        worker,
        carrier,
    })
}

fn capture_v5_stale_snapshot(
    world: &World,
    loaded: &LoadedV5Fixture,
) -> Result<V5StaleSnapshot, String> {
    let target_building = world
        .get::<Building>(loaded.target)
        .ok_or_else(|| "V5 stale snapshot target lost its Building".to_owned())?
        .kind;
    let target_pending_order = world
        .get::<DeconstructionPending>(loaded.target)
        .ok_or_else(|| "V5 stale snapshot target lost DeconstructionPending".to_owned())?
        .order;
    let target_map_owner = world
        .resource::<WorldMap>()
        .building_entity(WorldMap::world_to_grid(
            world
                .get::<Transform>(loaded.target)
                .ok_or_else(|| "V5 stale snapshot target lost Transform".to_owned())?
                .translation
                .truncate(),
        ))
        .ok_or_else(|| "V5 stale snapshot target lost WorldMap ownership".to_owned())?;
    let order_priority = world
        .get::<Priority>(loaded.order)
        .ok_or_else(|| "V5 stale snapshot order lost Priority".to_owned())?
        .0;
    let order_target = world
        .get::<TargetDeconstructionRoot>(loaded.order)
        .ok_or_else(|| "V5 stale snapshot order lost target relation".to_owned())?
        .0;
    let order_managed_by = world
        .get::<ManagedBy>(loaded.order)
        .ok_or_else(|| "V5 stale snapshot order lost Familiar ownership".to_owned())?
        .0;
    Ok(V5StaleSnapshot {
        target: loaded.target,
        order: loaded.order,
        familiar: loaded.familiar,
        worker: loaded.worker,
        carrier: loaded.carrier,
        target_building,
        target_pending_order,
        target_map_owner,
        order_priority,
        order_target,
        order_managed_by,
        resource_counts: resource_counts(world),
    })
}

fn validate_v5_stale_snapshot(world: &World, snapshot: &V5StaleSnapshot) -> Result<(), String> {
    if world.get_entity(snapshot.target).is_err()
        || world.get_entity(snapshot.order).is_err()
        || world.get_entity(snapshot.familiar).is_err()
        || world.get_entity(snapshot.worker).is_err()
        || world.get_entity(snapshot.carrier).is_err()
    {
        return Err("V5 stale request removed a persisted fixture entity".to_owned());
    }
    if world
        .get::<Building>(snapshot.target)
        .is_none_or(|building| building.kind != snapshot.target_building)
        || world
            .get::<DeconstructionPending>(snapshot.target)
            .is_none_or(|pending| pending.order != snapshot.target_pending_order)
        || world
            .resource::<WorldMap>()
            .building_entity(snapshot.target_map_owner_grid(world))
            != Some(snapshot.target_map_owner)
        || world
            .get::<Priority>(snapshot.order)
            .is_none_or(|priority| priority.0 != snapshot.order_priority)
        || world
            .get::<TargetDeconstructionRoot>(snapshot.order)
            .is_none_or(|target| target.0 != snapshot.order_target)
        || world
            .get::<ManagedBy>(snapshot.order)
            .is_none_or(|owner| owner.0 != snapshot.order_managed_by)
        || !matches!(
            world.get::<AssignedTask>(snapshot.worker),
            Some(AssignedTask::None)
        )
        || world.get::<ActiveTaskIdentity>(snapshot.worker).is_some()
        || world.get::<WorkingOn>(snapshot.worker).is_some()
        || resource_counts(world) != snapshot.resource_counts
    {
        return Err(
            "V5 stale request mutated persisted order, worker, map, or resources".to_owned(),
        );
    }
    Ok(())
}

impl V5StaleSnapshot {
    fn target_map_owner_grid(&self, world: &World) -> (i32, i32) {
        world
            .get::<Transform>(self.target)
            .map(|transform| WorldMap::world_to_grid(transform.translation.truncate()))
            .unwrap_or((i32::MIN, i32::MIN))
    }
}

fn await_v5_stale_replay(
    world: &mut World,
    driver: &mut NativeDeconstructionAcceptance,
    receipts: &FrameReceipts,
) -> Result<(), String> {
    let old = required(driver.v5_old_request, "V5 old commit request")?;
    let stale = receipts.commits.iter().any(|outcome| {
        outcome.worker == old.worker
            && outcome.order == old.order
            && outcome.result == DeconstructionCommitResult::StaleWorld
    });
    if !stale {
        return Ok(());
    }
    let snapshot = driver
        .v5_stale_snapshot
        .as_ref()
        .ok_or_else(|| "V5 stale replay snapshot is missing".to_owned())?;
    validate_v5_stale_snapshot(world, snapshot)?;
    world.resource_mut::<Time<Virtual>>().pause();
    driver.stage = AcceptanceStage::AwaitV5HelpCapture;
    info!("NATIVE_DECONSTRUCTION_ACCEPTANCE: V5 stale replay rejected");
    Ok(())
}

fn await_v5_help_capture(
    world: &mut World,
    driver: &mut NativeDeconstructionAcceptance,
) -> Result<(), String> {
    let loaded = required(driver.v5_loaded, "V5 loaded fixture")?;
    let topic = HelpTopicId::new("orders-areas");
    if !world.resource::<HelpPanelState>().open {
        return Ok(());
    }
    if world.resource::<HelpPanelState>().active_topic != Some(topic) {
        world.write_message(UiIntent::SelectHelpTopic(topic));
        return Ok(());
    }
    if !world.resource::<UiInputState>().world_input_captured {
        return Ok(());
    }
    resume_native_simulation(world, driver);
    world.write_message(UiIntent::AdjustTaskPriority {
        entity: loaded.order,
        expected_work_type: WorkType::Deconstruct,
        adjustment: TaskPriorityAdjustment::Decrease,
    });
    driver.stage = AcceptanceStage::AwaitV5CapturedPriority;
    info!("NATIVE_DECONSTRUCTION_ACCEPTANCE: V5 Help capture active");
    Ok(())
}

fn await_v5_captured_priority(
    world: &mut World,
    driver: &mut NativeDeconstructionAcceptance,
    receipts: &FrameReceipts,
) -> Result<(), String> {
    let loaded = required(driver.v5_loaded, "V5 loaded fixture")?;
    let captured = receipts.task_actions.iter().any(|outcome| {
        outcome.entity == loaded.order
            && outcome.action == TaskActionKind::AdjustPriority(TaskPriorityAdjustment::Decrease)
            && outcome.result == TaskActionResult::Captured
    });
    if !captured {
        return Ok(());
    }
    if world
        .get::<Priority>(loaded.order)
        .map(|priority| priority.0)
        != Some(10)
    {
        return Err("V5 captured priority action mutated the order".to_owned());
    }
    world.write_message(UiIntent::CloseHelp);
    driver.stage = AcceptanceStage::AwaitV5HelpClosed;
    info!("NATIVE_DECONSTRUCTION_ACCEPTANCE: V5 captured action rejected");
    Ok(())
}

fn await_v5_help_closed(
    world: &mut World,
    driver: &mut NativeDeconstructionAcceptance,
) -> Result<(), String> {
    if world.resource::<HelpPanelState>().open
        || world.resource::<UiInputState>().world_input_captured
    {
        return Ok(());
    }
    driver.stage = AcceptanceStage::AwaitV5TaskDashboardTab;
    info!("NATIVE_DECONSTRUCTION_ACCEPTANCE: V5 Help closed");
    Ok(())
}

fn await_v5_priority_change(
    world: &mut World,
    driver: &mut NativeDeconstructionAcceptance,
    receipts: &FrameReceipts,
) -> Result<(), String> {
    let loaded = required(driver.v5_loaded, "V5 loaded fixture")?;
    let changed = receipts.task_actions.iter().any(|outcome| {
        outcome.entity == loaded.order
            && outcome.action == TaskActionKind::AdjustPriority(TaskPriorityAdjustment::Decrease)
            && outcome.result == TaskActionResult::PriorityChanged(TaskPriorityTier::High)
    });
    if !changed {
        return Ok(());
    }
    if world
        .get::<Priority>(loaded.order)
        .map(|priority| priority.0)
        != Some(5)
    {
        return Err("V5 live priority action did not produce priority 5".to_owned());
    }
    if let Some(mut state) = world.get_mut::<FamiliarAiState>(loaded.familiar) {
        *state = FamiliarAiState::SearchingTask;
    }
    driver.stage = AcceptanceStage::AwaitV5Reassignment;
    info!("NATIVE_DECONSTRUCTION_ACCEPTANCE: V5 priority changed");
    Ok(())
}

fn await_v5_reassignment(
    world: &mut World,
    driver: &mut NativeDeconstructionAcceptance,
) -> Result<(), String> {
    let loaded = required(driver.v5_loaded, "V5 loaded fixture")?;
    if !matches!(
        world.get::<AssignedTask>(loaded.worker),
        Some(AssignedTask::Deconstruct(DeconstructData {
            order,
            target,
            ..
        })) if *order == loaded.order && *target == loaded.target
    ) {
        if let Some(mut state) = world.get_mut::<FamiliarAiState>(loaded.familiar) {
            *state = FamiliarAiState::SearchingTask;
        }
        return Ok(());
    }
    driver.stage = AcceptanceStage::AwaitV5CancelFirstPress;
    info!("NATIVE_DECONSTRUCTION_ACCEPTANCE: V5 order reassigned");
    Ok(())
}

fn await_v5_cancel(
    world: &mut World,
    driver: &mut NativeDeconstructionAcceptance,
    receipts: &FrameReceipts,
) -> Result<(), String> {
    let loaded = required(driver.v5_loaded, "V5 loaded fixture")?;
    driver.v5_cancel_action_seen |= receipts.task_actions.iter().any(|outcome| {
        outcome.entity == loaded.order
            && outcome.action == TaskActionKind::Cancel
            && outcome.result == TaskActionResult::AwaitingOwnerOutcome
    });
    let canceled = receipts.cancels.iter().any(|outcome| {
        outcome.order == loaded.order && outcome.result == DeconstructionCancelResult::Canceled
    });
    if !driver.v5_cancel_action_seen || !canceled {
        return Ok(());
    }
    if world.get_entity(loaded.order).is_ok()
        || world.get_entity(loaded.target).is_err()
        || world.get::<DeconstructionPending>(loaded.target).is_some()
        || !matches!(
            world.get::<AssignedTask>(loaded.worker),
            Some(AssignedTask::None)
        )
        || world.get::<ActiveTaskIdentity>(loaded.worker).is_some()
        || world.get::<WorkingOn>(loaded.worker).is_some()
        || world.get_entity(loaded.carrier).is_err()
    {
        return Err(
            "V5 dashboard cancel did not preserve target and terminalize worker".to_owned(),
        );
    }
    driver.checks[4] = true;
    driver.stage = AcceptanceStage::AwaitFinalHelp;
    info!("NATIVE_DECONSTRUCTION_ACCEPTANCE: V5 PASS");
    Ok(())
}

fn await_final_help(
    world: &mut World,
    driver: &mut NativeDeconstructionAcceptance,
) -> Result<(), String> {
    let topic = HelpTopicId::new("orders-areas");
    if !world.resource::<HelpPanelState>().open {
        return Ok(());
    }
    if world.resource::<HelpPanelState>().active_topic != Some(topic) {
        world.write_message(UiIntent::SelectHelpTopic(topic));
        return Ok(());
    }
    let mut help_query = world.query_filtered::<&Node, With<HelpPanel>>();
    let node = help_query
        .single(world)
        .map_err(|_| "final Help overlay root is missing or duplicated".to_owned())?;
    if node.display == Display::None || !help_recovery_copy_is_complete(world) {
        return Ok(());
    }
    if driver.banner.is_none() {
        driver.banner = Some(spawn_pass_banner(world));
        return Ok(());
    }
    if !driver.screenshot_requested {
        world
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(driver.screenshot_path()));
        driver.screenshot_requested = true;
    }
    driver.stage = AcceptanceStage::AwaitScreenshot;
    Ok(())
}

fn help_recovery_copy_is_complete(world: &World) -> bool {
    let entry_id = HelpEntryId::new("building-deconstruction");
    world
        .resource::<HelpPanelContent>()
        .topics()
        .find(|topic| topic.id() == HelpTopicId::new("orders-areas"))
        .and_then(|topic| topic.entries().iter().find(|entry| entry.id() == entry_id))
        .is_some_and(|entry| {
            let text = entry.paragraphs().join(" ");
            text.contains("Wall・Door・Tank・WheelbarrowParking は Wood×1")
                && text.contains("SoulSpa は Bone×6")
                && text.contains("Bridge は Rock×3")
        })
}

fn spawn_pass_banner(world: &mut World) -> Entity {
    world
        .spawn((
            Text::new("Native C1 V1-V5 PASS"),
            TextFont {
                font_size: FontSize::Px(24.0),
                ..default()
            },
            TextColor(Color::WHITE),
            BackgroundColor(Color::srgba(0.0, 0.35, 0.12, 0.94)),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(16.0),
                top: Val::Px(16.0),
                padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BorderColor::all(Color::srgb(0.25, 1.0, 0.45)),
            ZIndex(10_000),
            Name::new("Native C1 Acceptance PASS"),
        ))
        .id()
}

fn await_screenshot(
    world: &mut World,
    driver: &mut NativeDeconstructionAcceptance,
) -> Result<(), String> {
    let screenshot = driver.screenshot_path();
    if !screenshot.is_file() {
        return Ok(());
    }
    let evidence = validate_png(&screenshot)?;
    let renderer = match driver.render_evidence.snapshot() {
        RenderEvidenceState::Ready(renderer) => renderer,
        RenderEvidenceState::Pending => return Ok(()),
        RenderEvidenceState::Failed(reason) => return Err(reason),
    };
    if !driver.checks.into_iter().all(|passed| passed) {
        return Err("terminal native acceptance reached capture before V1-V5 passed".to_owned());
    }
    write_success_result(driver, &renderer, evidence)?;
    driver.stage = AcceptanceStage::Finished;
    info!("NATIVE_DECONSTRUCTION_ACCEPTANCE: PASS");
    world.write_message(AppExit::Success);
    Ok(())
}

#[derive(Clone, Copy, Debug)]
struct PngEvidence {
    width: u32,
    height: u32,
    bytes: u64,
}

fn validate_png(path: &Path) -> Result<PngEvidence, String> {
    let bytes = fs::read(path)
        .map_err(|error| format!("could not read screenshot {}: {error}", path.display()))?;
    let (width, height) = validate_png_structure(&bytes)?;
    let byte_count = bytes.len() as u64;
    if width < MIN_SCREENSHOT_WIDTH || height < MIN_SCREENSHOT_HEIGHT {
        return Err(format!("native screenshot is too small: {width}x{height}"));
    }
    if byte_count == 0 || byte_count > MAX_SCREENSHOT_BYTES {
        return Err(format!(
            "native screenshot size is invalid: {byte_count} bytes"
        ));
    }
    Ok(PngEvidence {
        width,
        height,
        bytes: byte_count,
    })
}

fn validate_png_structure(bytes: &[u8]) -> Result<(u32, u32), String> {
    const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < SIGNATURE.len() || &bytes[..SIGNATURE.len()] != SIGNATURE {
        return Err("native screenshot is not a PNG".to_owned());
    }

    let mut offset = SIGNATURE.len();
    let mut dimensions = None;
    let mut saw_idat = false;
    while offset < bytes.len() {
        let header_end = offset
            .checked_add(8)
            .ok_or_else(|| "native screenshot PNG chunk offset overflowed".to_owned())?;
        if header_end > bytes.len() {
            return Err("native screenshot PNG has a truncated chunk header".to_owned());
        }
        let data_len = u32::from_be_bytes(
            bytes[offset..offset + 4]
                .try_into()
                .expect("four-byte PNG chunk length"),
        ) as usize;
        let chunk_type = &bytes[offset + 4..offset + 8];
        let data_end = header_end
            .checked_add(data_len)
            .ok_or_else(|| "native screenshot PNG chunk length overflowed".to_owned())?;
        let chunk_end = data_end
            .checked_add(4)
            .ok_or_else(|| "native screenshot PNG CRC offset overflowed".to_owned())?;
        if chunk_end > bytes.len() {
            return Err("native screenshot PNG has a truncated chunk".to_owned());
        }
        let expected_crc = u32::from_be_bytes(
            bytes[data_end..chunk_end]
                .try_into()
                .expect("four-byte PNG chunk CRC"),
        );
        let actual_crc = png_crc32(&bytes[offset + 4..data_end]);
        if actual_crc != expected_crc {
            return Err("native screenshot PNG has a corrupt chunk CRC".to_owned());
        }

        match chunk_type {
            b"IHDR" => {
                if offset != SIGNATURE.len() || data_len != 13 || dimensions.is_some() {
                    return Err("native screenshot PNG has an invalid IHDR".to_owned());
                }
                let width = u32::from_be_bytes(
                    bytes[header_end..header_end + 4]
                        .try_into()
                        .expect("four-byte PNG width"),
                );
                let height = u32::from_be_bytes(
                    bytes[header_end + 4..header_end + 8]
                        .try_into()
                        .expect("four-byte PNG height"),
                );
                if width == 0 || height == 0 {
                    return Err("native screenshot PNG has zero dimensions".to_owned());
                }
                dimensions = Some((width, height));
            }
            b"IDAT" => saw_idat = true,
            b"IEND" => {
                if data_len != 0 || dimensions.is_none() || !saw_idat || chunk_end != bytes.len() {
                    return Err("native screenshot PNG has an invalid terminal IEND".to_owned());
                }
                return Ok(dimensions.expect("dimensions checked above"));
            }
            _ => {
                if dimensions.is_none() {
                    return Err("native screenshot PNG does not start with IHDR".to_owned());
                }
            }
        }
        offset = chunk_end;
    }
    Err("native screenshot PNG is missing its terminal IEND".to_owned())
}

fn png_crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = 0u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn spawn_direct_commit_fixture(
    world: &mut World,
    target: Entity,
    position: Vec2,
    managed_by: Option<Entity>,
    priority: u32,
) -> CommitFixture {
    let mut order = world.spawn((
        DeconstructionOrder,
        Designation {
            work_type: WorkType::Deconstruct,
        },
        TaskSlots::new(1),
        TargetDeconstructionRoot(target),
        Priority(priority),
        Transform::from_translation(position.extend(0.0)),
    ));
    if let Some(familiar) = managed_by {
        order.insert(ManagedBy(familiar));
    }
    let order = order.id();
    world.flush();
    world
        .entity_mut(target)
        .insert(DeconstructionPending { order });
    let worker = spawn_commit_worker(world, order, target, DeconstructPhase::AwaitingCommit);
    let identity = *world
        .get::<ActiveTaskIdentity>(worker)
        .expect("native commit worker identity");
    CommitFixture {
        target,
        order,
        worker,
        identity,
    }
}

fn spawn_commit_worker(
    world: &mut World,
    order: Entity,
    target: Entity,
    phase: DeconstructPhase,
) -> Entity {
    let position = world
        .get::<Transform>(target)
        .map_or(Vec3::ZERO, |transform| transform.translation);
    let identity = ActiveTaskIdentity::new(order, order, WorkType::Deconstruct);
    let worker = world
        .spawn((
            DamnedSoul::default(),
            AssignedTask::Deconstruct(DeconstructData {
                order,
                target,
                phase,
            }),
            Destination(position.truncate()),
            SoulPath::default(),
            Inventory::default(),
            identity,
            WorkingOn(order),
            Transform::from_translation(position),
            Name::new("Native C1 Commit Worker"),
        ))
        .id();
    world.flush();
    worker
}

fn write_commit(world: &mut World, fixture: CommitFixture) {
    world.write_message(DeconstructionCommitRequest {
        world_epoch: world.resource::<WorldEpoch>().get(),
        worker: fixture.worker,
        identity: fixture.identity,
        order: fixture.order,
        target: fixture.target,
    });
}

fn footprint_2x2(lower_left: (i32, i32)) -> [(i32, i32); 4] {
    [
        lower_left,
        (lower_left.0 + 1, lower_left.1),
        (lower_left.0, lower_left.1 + 1),
        (lower_left.0 + 1, lower_left.1 + 1),
    ]
}

fn resource_entities(world: &mut World, resource_type: ResourceType) -> HashSet<Entity> {
    world
        .query::<(Entity, &ResourceItem)>()
        .iter(world)
        .filter_map(|(entity, item)| (item.0 == resource_type).then_some(entity))
        .collect()
}

fn resource_counts(world: &World) -> HashMap<ResourceType, usize> {
    let mut counts = HashMap::new();
    for entity in world.iter_entities() {
        if let Some(item) = entity.get::<ResourceItem>() {
            *counts.entry(item.0).or_insert(0) += 1;
        }
    }
    counts
}

fn new_resource_entities(
    world: &mut World,
    resource_type: ResourceType,
    before: &HashSet<Entity>,
) -> Vec<Entity> {
    let mut entities = resource_entities(world, resource_type)
        .difference(before)
        .copied()
        .collect::<Vec<_>>();
    entities.sort_unstable_by_key(|entity| entity.to_bits());
    entities
}

fn validate_terminal_worker(world: &World, worker: Entity, label: &str) -> Result<(), String> {
    if !matches!(world.get::<AssignedTask>(worker), Some(AssignedTask::None))
        || world.get::<ActiveTaskIdentity>(worker).is_some()
        || world.get::<WorkingOn>(worker).is_some()
    {
        return Err(format!("{label} did not terminalize its exact worker"));
    }
    Ok(())
}

fn required<T: Copy>(value: Option<T>, label: &str) -> Result<T, String> {
    value.ok_or_else(|| format!("{label} is missing"))
}

fn write_success_result(
    driver: &NativeDeconstructionAcceptance,
    renderer: &RenderEnvironment,
    screenshot: PngEvidence,
) -> Result<(), String> {
    let save_bytes = fs::metadata(&driver.save_path)
        .map_err(|error| format!("could not stat native save artifact: {error}"))?
        .len();
    let body = format!(
        concat!(
            "{{\n",
            "  \"status\": \"PASS\",\n",
            "  \"profile\": \"building-deconstruction\",\n",
            "  \"run_id\": \"{}\",\n",
            "  \"checks\": {{\"V1\": \"PASS\", \"V2\": \"PASS\", \"V3\": \"PASS\", \"V4\": \"PASS\", \"V5\": \"PASS\"}},\n",
            "  \"world_epoch_before_load\": {},\n",
            "  \"world_epoch_after_load\": {},\n",
            "  \"save\": {{\"path\": \"{}\", \"bytes\": {}}},\n",
            "  \"screenshot\": {{\"path\": \"{}\", \"width\": {}, \"height\": {}, \"bytes\": {}}},\n",
            "  \"renderer\": {{\"adapter_name\": \"{}\", \"backend\": \"{}\", \"display_handle\": \"{}\"}}\n",
            "}}\n"
        ),
        json_escape(&driver.run_id),
        driver.epoch_before_load,
        driver.epoch_after_load,
        json_escape(&driver.save_path.display().to_string()),
        save_bytes,
        json_escape(&driver.screenshot_path().display().to_string()),
        screenshot.width,
        screenshot.height,
        screenshot.bytes,
        json_escape(&renderer.adapter_name),
        json_escape(&renderer.adapter_backend),
        json_escape(&renderer.display_handle),
    );
    write_new_atomic(&driver.result_path(), body.as_bytes())
        .map_err(|error| format!("could not publish native acceptance result: {error}"))
}

fn fail_driver(world: &mut World, driver: &mut NativeDeconstructionAcceptance, reason: &str) {
    if driver.stage == AcceptanceStage::Finished {
        return;
    }
    let body = format!(
        "{{\n  \"status\": \"FAIL\",\n  \"profile\": \"building-deconstruction\",\n  \"run_id\": \"{}\",\n  \"reason\": \"{}\"\n}}\n",
        json_escape(&driver.run_id),
        json_escape(reason),
    );
    if let Err(error) = write_new_atomic(&driver.result_path(), body.as_bytes()) {
        error!("NATIVE_DECONSTRUCTION_ACCEPTANCE: {reason}; result write failed: {error}");
    } else {
        error!("NATIVE_DECONSTRUCTION_ACCEPTANCE: {reason}");
    }
    driver.stage = AcceptanceStage::Finished;
    world.write_message(AppExit::error());
}

fn validate_run_id(run_id: &str) -> Result<(), String> {
    if run_id.is_empty()
        || run_id.len() > 64
        || !run_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(format!(
            "{RUN_ID_ENV} must contain 1..=64 ASCII letters, digits, '-' or '_'"
        ));
    }
    Ok(())
}

fn write_new_atomic(path: &Path, contents: &[u8]) -> io::Result<()> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid output filename"))?;
    let temporary_path = path.with_file_name(format!(".{file_name}.{}.tmp", std::process::id()));
    let write_result = (|| {
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)?;
        output.write_all(contents)?;
        output.sync_all()
    })();
    if let Err(error) = write_result {
        let _ = fs::remove_file(&temporary_path);
        return Err(error);
    }
    match fs::rename(&temporary_path, path) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = fs::remove_file(&temporary_path);
            Err(error)
        }
    }
}

fn json_escape(value: &str) -> String {
    value
        .chars()
        .flat_map(|character| match character {
            '"' => "\\\"".chars().collect::<Vec<_>>(),
            '\\' => "\\\\".chars().collect(),
            '\n' => "\\n".chars().collect(),
            '\r' => "\\r".chars().collect(),
            '\t' => "\\t".chars().collect(),
            character if character.is_control() => {
                format!("\\u{:04x}", character as u32).chars().collect()
            }
            character => vec![character],
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png_chunk(chunk_type: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut chunk = Vec::new();
        chunk.extend_from_slice(&(data.len() as u32).to_be_bytes());
        chunk.extend_from_slice(chunk_type);
        chunk.extend_from_slice(data);
        let crc_start = 4;
        chunk.extend_from_slice(&png_crc32(&chunk[crc_start..]).to_be_bytes());
        chunk
    }

    fn structural_png(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&width.to_be_bytes());
        ihdr.extend_from_slice(&height.to_be_bytes());
        ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
        bytes.extend(png_chunk(b"IHDR", &ihdr));
        bytes.extend(png_chunk(
            b"IDAT",
            &[0x78, 0x9c, 0x03, 0x00, 0x00, 0x00, 0x00, 0x01],
        ));
        bytes.extend(png_chunk(b"IEND", &[]));
        bytes
    }

    #[test]
    fn run_id_validation_rejects_paths_and_accepts_fresh_tokens() {
        assert!(validate_run_id("c1-native-20260805_ab12").is_ok());
        assert!(validate_run_id("../stale").is_err());
        assert!(validate_run_id("").is_err());
    }

    #[test]
    fn json_escape_keeps_driver_results_well_formed() {
        assert_eq!(json_escape("x\n\"y\\z"), "x\\n\\\"y\\\\z");
    }

    #[test]
    fn png_validation_requires_complete_crc_checked_structure() {
        let valid = structural_png(1280, 720);
        assert_eq!(validate_png_structure(&valid), Ok((1280, 720)));

        let mut truncated = valid.clone();
        truncated.truncate(24);
        assert!(validate_png_structure(&truncated).is_err());

        let mut corrupt = valid;
        let last = corrupt.len() - 1;
        corrupt[last] ^= 0x01;
        assert!(validate_png_structure(&corrupt).is_err());
    }
}
