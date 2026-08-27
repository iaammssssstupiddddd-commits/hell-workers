//! Opt-in actual-window save/load acceptance driver.
//!
//! The driver is dormant unless `HW_NATIVE_SAVE_LOAD_ACCEPTANCE_ARTIFACT` is
//! set. It exists because desktop security may reject synthetic X11 input even
//! when an actual game window and hardware renderer are available. The normal
//! player input and UI paths remain covered by their resolver/handler tests;
//! this driver exercises the production `Last` dispatcher and world replace.

use std::any::TypeId;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bevy::ecs::reflect::AppTypeRegistry;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::reflect::PartialReflect;
use bevy::render::renderer::RenderAdapterInfo;
use bevy::render::view::window::ExtractedWindows;
use bevy::render::{Render, RenderApp};
use bevy::window::PrimaryWindow;
use bevy_world_serialization::DynamicWorld;
use bevy_world_serialization::serde::WorldDeserializer;
use hw_core::WorldEpoch;
use hw_core::familiar::Familiar;
use hw_core::soul::DamnedSoul;
use hw_ui::UiIntent;
use hw_ui::components::{SaveCatalogDialog, UiInputState};
use hw_world::WorldMap;

use crate::systems::settings::SettingsStorageRoot;

use super::format::{
    CURRENT_SAVE_FORMAT_VERSION, SAVE_MAGIC, SaveFormat, SaveHeader, decode_save_file,
    encode_save_header_text,
};
use super::state::{NativeLoadFault, NativeLoadFaultInjection};
use super::{
    SaveCatalog, SaveCatalogMode, SaveCatalogUi, SaveContentStatus, SaveLoadFailureKind,
    SaveLoadOperation, SaveLoadOutcome, SaveLoadResult, SaveLoadState, SavePath, SaveRecoveryMode,
    SaveStorageRoot,
};
use hw_core::{SaveSlotId, SaveSlotRole};

const ARTIFACT_ENV: &str = "HW_NATIVE_SAVE_LOAD_ACCEPTANCE_ARTIFACT";
const RUNTIME_ROOT_ENV: &str = "HW_NATIVE_SAVE_LOAD_ACCEPTANCE_RUNTIME_ROOT";
const RUN_ID_ENV: &str = "HW_NATIVE_SAVE_LOAD_ACCEPTANCE_RUN_ID";
const RESULT_FILE: &str = "driver-result.json";
const CAPTURE_READY_FILE: &str = "capture-ready.txt";
const CAPTURE_ACK_FILE: &str = "capture.done.txt";
const SCREENSHOT_FILE: &str = "paused-after-acceptance.png";
const CAPTURE_SCOPE: &str = "x11-client-window";
const MAX_SCREENSHOT_BYTES: u64 = 16 * 1024 * 1024;
const MIN_SCREENSHOT_WIDTH: u32 = 640;
const MIN_SCREENSHOT_HEIGHT: u32 = 360;
const CAPTURE_MARKER_MIN_PIXELS: u32 = 1_024;
const CAPTURE_MARKER_FRAMES: u32 = 3;
const READY_FRAMES: u32 = 30;
const DRIVER_TIMEOUT: Duration = Duration::from_secs(180);

/// Adds a bounded save/load acceptance sequence to the regular actual-window
/// application. This is never enabled by default.
pub struct NativeSaveLoadAcceptancePlugin {
    artifact_dir: PathBuf,
    runtime_root: PathBuf,
    run_id: String,
}

impl NativeSaveLoadAcceptancePlugin {
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
        let runtime_root =
            PathBuf::from(env::var_os(RUNTIME_ROOT_ENV).ok_or_else(|| {
                format!("{RUNTIME_ROOT_ENV} must name a fresh runtime directory")
            })?);
        if !runtime_root.is_absolute() || !runtime_root.is_dir() {
            return Err(format!(
                "{RUNTIME_ROOT_ENV} must name an existing absolute directory"
            ));
        }
        let canonical_artifact = artifact_dir.canonicalize().map_err(|error| {
            format!(
                "could not canonicalize native acceptance artifact {}: {error}",
                artifact_dir.display()
            )
        })?;
        let canonical_runtime = runtime_root.canonicalize().map_err(|error| {
            format!(
                "could not canonicalize native acceptance runtime root {}: {error}",
                runtime_root.display()
            )
        })?;
        if canonical_runtime.starts_with(&canonical_artifact) {
            return Err(format!(
                "{RUNTIME_ROOT_ENV} must be outside {ARTIFACT_ENV} so screenshots and runtime data remain isolated"
            ));
        }
        if !env::var("HW_WINDOW_BACKEND").is_ok_and(|value| value.eq_ignore_ascii_case("x11")) {
            return Err(format!(
                "{ARTIFACT_ENV} requires HW_WINDOW_BACKEND=x11 so its screenshot can be bound to the game client window"
            ));
        }
        let run_id = env::var(RUN_ID_ENV)
            .map_err(|_| format!("{RUN_ID_ENV} must be set to a fresh run identifier"))?;
        validate_run_id(&run_id)?;

        for owned_path in [
            artifact_dir.join(RESULT_FILE),
            artifact_dir.join(CAPTURE_READY_FILE),
            artifact_dir.join(CAPTURE_ACK_FILE),
            artifact_dir.join(SCREENSHOT_FILE),
            runtime_root.join("saves/manual-1.scn.ron"),
            runtime_root.join("saves/manual-2.scn.ron"),
            runtime_root.join("settings/settings.ron"),
        ] {
            if owned_path.exists() {
                return Err(format!(
                    "native acceptance artifact contains stale driver output: {}",
                    owned_path.display()
                ));
            }
        }
        for runtime_dir in [runtime_root.join("saves"), runtime_root.join("settings")] {
            fs::create_dir_all(&runtime_dir).map_err(|error| {
                format!(
                    "could not initialize native acceptance runtime root {}: {error}",
                    runtime_dir.display()
                )
            })?;
        }

        Ok(Some(Self {
            artifact_dir,
            runtime_root,
            run_id,
        }))
    }
}

impl Plugin for NativeSaveLoadAcceptancePlugin {
    fn build(&self, app: &mut App) {
        let save_root = self.runtime_root.join("saves");
        let settings_root = self.runtime_root.join("settings");
        let render_evidence = NativeRenderEvidence::pending();
        app.insert_resource(SaveStorageRoot::new(save_root.clone()));
        app.insert_resource(SettingsStorageRoot::new(settings_root));
        app.insert_resource(NativeLoadFaultInjection::default());
        app.insert_resource(SavePath::new(
            save_root.join(hw_core::SaveSlotId::Manual1.canonical_file_name()),
        ));
        app.insert_resource(NativeSaveLoadAcceptance::new(
            self.artifact_dir.clone(),
            save_root.join(hw_core::SaveSlotId::Manual1.canonical_file_name()),
            self.run_id.clone(),
            render_evidence.clone(),
            self.runtime_root.clone(),
        ));
        install_render_evidence(app, render_evidence);
        // UiIntent is consumed during Update and the save dispatcher runs in
        // Last. Driving from PostUpdate makes each transition observable
        // without bypassing either production owner.
        app.add_systems(PostUpdate, drive_native_save_load_acceptance);
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
            "RenderApp is unavailable for native save-catalog acceptance".to_owned(),
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
    AwaitInitialSaveCatalog,
    AwaitInitialSave,
    AwaitInitialLoadCatalog,
    AwaitInitialLoadConfirm,
    AwaitInitialLoad,
    WaitForLoadedWorld,
    AwaitBaselineSaveCatalog,
    AwaitBaselineSave,
    AwaitBaselineOverwriteConfirm,
    AwaitBaselineConfirmation,
    AwaitInvalidLoadCatalog,
    AwaitInvalidLoadConfirm,
    AwaitInvalidLoad,
    AwaitPostInvalidSaveCatalog,
    AwaitPostInvalidOverwriteConfirm,
    AwaitPostInvalidSave,
    AwaitV4LoadCatalog,
    AwaitV4LoadConfirm,
    AwaitV4CatalogAfterCancel,
    AwaitV4Closed,
    AwaitV5BackupSaveCatalog,
    AwaitV5BackupSave,
    AwaitV5ApplyRecoveredCatalog,
    AwaitV5ApplyRecoveredConfirm,
    AwaitV5ApplyRecovered,
    AwaitV5ApplyRecoveredReset,
    AwaitV5RecoveryFailureCatalog,
    AwaitV5RecoveryFailureConfirm,
    AwaitV5RecoveryFailure,
    AwaitV5RecoveryCatalog,
    AwaitV5RecoveryInvalidConfirm,
    AwaitV5RecoveryInvalid,
    AwaitV5RecoveryValidCatalog,
    AwaitV5RecoveryApplyFailureConfirm,
    AwaitV5RecoveryApplyFailure,
    AwaitV5RecoveryValidConfirm,
    AwaitV5RecoveryValid,
    AwaitV5Resume,
    AwaitV5Repaused,
    AwaitCaptureCatalog,
    AwaitCaptureMarker,
    AwaitCapture,
    Finished,
}

struct PersistentSnapshot {
    format: SaveFormat,
    world: DynamicWorld,
}

#[derive(Resource)]
struct NativeSaveLoadAcceptance {
    artifact_dir: PathBuf,
    runtime_root: PathBuf,
    save_path: PathBuf,
    run_id: String,
    stage: AcceptanceStage,
    started_at: Instant,
    ready_frames: u32,
    baseline_snapshot: Option<PersistentSnapshot>,
    epoch_before_valid_load: u64,
    baseline_epoch: u64,
    checks: [bool; 5],
    stale_session_checked: bool,
    confirmation_cancel_checked: bool,
    baseline_save_completed: bool,
    recovery_apply_failure_checked: bool,
    render_evidence: NativeRenderEvidence,
}

#[derive(Component)]
struct NativeSaveCatalogCaptureMarker;

impl NativeSaveLoadAcceptance {
    fn new(
        artifact_dir: PathBuf,
        save_path: PathBuf,
        run_id: String,
        render_evidence: NativeRenderEvidence,
        runtime_root: PathBuf,
    ) -> Self {
        Self {
            artifact_dir,
            runtime_root,
            save_path,
            run_id,
            stage: AcceptanceStage::WaitForWorld,
            started_at: Instant::now(),
            ready_frames: 0,
            baseline_snapshot: None,
            epoch_before_valid_load: 0,
            baseline_epoch: 0,
            checks: [false; 5],
            stale_session_checked: false,
            confirmation_cancel_checked: false,
            baseline_save_completed: false,
            recovery_apply_failure_checked: false,
            render_evidence,
        }
    }

    fn result_path(&self) -> PathBuf {
        self.artifact_dir.join(RESULT_FILE)
    }

    fn capture_ack_path(&self) -> PathBuf {
        self.artifact_dir.join(CAPTURE_ACK_FILE)
    }

    fn capture_ready_path(&self) -> PathBuf {
        self.artifact_dir.join(CAPTURE_READY_FILE)
    }
}

#[derive(SystemParam)]
struct NativeAcceptanceContext<'w, 's> {
    driver: ResMut<'w, NativeSaveLoadAcceptance>,
    world_map: Option<Res<'w, WorldMap>>,
    souls: Query<'w, 's, (), With<DamnedSoul>>,
    familiars: Query<'w, 's, (), With<Familiar>>,
    primary_windows: Query<'w, 's, (), With<PrimaryWindow>>,
    type_registry: Res<'w, AppTypeRegistry>,
    asset_server: Res<'w, AssetServer>,
    world_epoch: Res<'w, WorldEpoch>,
    virtual_time: ResMut<'w, Time<Virtual>>,
    save_load_state: Res<'w, SaveLoadState>,
    save_recovery_mode: Res<'w, SaveRecoveryMode>,
    native_faults: ResMut<'w, NativeLoadFaultInjection>,
    save_catalog: Res<'w, SaveCatalog>,
    save_catalog_ui: Res<'w, SaveCatalogUi>,
    ui_input_state: Res<'w, UiInputState>,
    catalog_dialogs: Query<'w, 's, (Entity, &'static Node), With<SaveCatalogDialog>>,
    commands: Commands<'w, 's>,
    ui_intents: MessageWriter<'w, UiIntent>,
    outcomes: MessageReader<'w, 's, SaveLoadOutcome>,
    exit: MessageWriter<'w, AppExit>,
}

fn drive_native_save_load_acceptance(context: NativeAcceptanceContext) {
    let NativeAcceptanceContext {
        mut driver,
        world_map,
        souls,
        familiars,
        primary_windows,
        type_registry,
        asset_server,
        world_epoch,
        mut virtual_time,
        save_load_state,
        save_recovery_mode,
        mut native_faults,
        save_catalog,
        save_catalog_ui,
        ui_input_state,
        catalog_dialogs,
        mut commands,
        mut ui_intents,
        mut outcomes,
        mut exit,
    } = context;
    if driver.stage == AcceptanceStage::Finished {
        return;
    }
    if driver.started_at.elapsed() > DRIVER_TIMEOUT {
        fail_driver(
            &mut driver,
            "native save/load acceptance timed out",
            &mut exit,
        );
        return;
    }

    let terminal_outcomes = outcomes.read().cloned().collect::<Vec<_>>();
    if terminal_outcomes.len() > 1 {
        fail_driver(
            &mut driver,
            "received multiple terminal outcomes in one frame",
            &mut exit,
        );
        return;
    }
    let terminal = terminal_outcomes.first();

    match driver.stage {
        AcceptanceStage::WaitForWorld => {
            if terminal.is_some() {
                fail_driver(
                    &mut driver,
                    "received an outcome before the acceptance sequence started",
                    &mut exit,
                );
                return;
            }
            let ready = persistent_world_is_ready(
                world_map.as_deref(),
                &souls,
                &familiars,
                &primary_windows,
            );
            if !ready {
                driver.ready_frames = 0;
                return;
            }
            driver.ready_frames += 1;
            if driver.ready_frames < READY_FRAMES {
                return;
            }
            virtual_time.pause();
            ui_intents.write(UiIntent::SaveGame);
            driver.stage = AcceptanceStage::AwaitInitialSaveCatalog;
            info!("NATIVE_SAVE_LOAD_ACCEPTANCE: initial save catalog requested");
        }
        AcceptanceStage::AwaitInitialSaveCatalog => {
            if terminal.is_some() {
                fail_driver(
                    &mut driver,
                    "received an outcome before selecting the initial save slot",
                    &mut exit,
                );
                return;
            }
            if !matches!(save_catalog_ui.mode, SaveCatalogMode::SaveCatalog)
                || !catalog_capture_is_ready(&catalog_dialogs, &ui_input_state)
            {
                return;
            }
            if !catalog_entry_matches(
                &save_catalog,
                SaveSlotId::Manual1,
                SaveSlotRole::Manual,
                SaveContentStatus::Empty,
                true,
                false,
            ) {
                fail_driver(
                    &mut driver,
                    "initial Save catalog did not expose Manual 1 as an empty manual slot",
                    &mut exit,
                );
                return;
            }
            ui_intents.write(UiIntent::SelectSaveCatalogSlot {
                slot: SaveSlotId::Manual1,
                session: save_catalog_ui.session,
            });
            driver.stage = AcceptanceStage::AwaitInitialSave;
            info!("NATIVE_SAVE_LOAD_ACCEPTANCE: initial Manual 1 save selected through UiIntent");
        }
        AcceptanceStage::AwaitInitialSave => {
            let Some(outcome) = terminal else {
                return;
            };
            if !matches_outcome(outcome, SaveLoadOperation::Save, SaveLoadResult::Succeeded) {
                fail_unexpected_outcome(&mut driver, outcome, &mut exit);
                return;
            }
            driver.epoch_before_valid_load = world_epoch.get();
            ui_intents.write(UiIntent::RequestLoadGame);
            driver.stage = AcceptanceStage::AwaitInitialLoadCatalog;
            info!("NATIVE_SAVE_LOAD_ACCEPTANCE: initial load catalog requested");
        }
        AcceptanceStage::AwaitInitialLoadCatalog => {
            if terminal.is_some() {
                fail_driver(
                    &mut driver,
                    "received an outcome before selecting the initial load slot",
                    &mut exit,
                );
                return;
            }
            if !matches!(save_catalog_ui.mode, SaveCatalogMode::LoadCatalog)
                || !catalog_capture_is_ready(&catalog_dialogs, &ui_input_state)
            {
                return;
            }
            if !catalog_entry_matches(
                &save_catalog,
                SaveSlotId::Manual1,
                SaveSlotRole::Manual,
                SaveContentStatus::CurrentV1 { worldgen_seed: 0 },
                true,
                true,
            ) {
                fail_driver(
                    &mut driver,
                    "initial Load catalog did not expose Manual 1 as loadable",
                    &mut exit,
                );
                return;
            }
            ui_intents.write(UiIntent::SelectLoadCatalogSlot {
                slot: SaveSlotId::Manual1,
                session: save_catalog_ui.session,
            });
            driver.stage = AcceptanceStage::AwaitInitialLoadConfirm;
        }
        AcceptanceStage::AwaitInitialLoadConfirm => {
            if terminal.is_some() {
                fail_driver(
                    &mut driver,
                    "received an outcome before confirming the initial load",
                    &mut exit,
                );
                return;
            }
            if !matches!(
                save_catalog_ui.mode,
                SaveCatalogMode::LoadConfirm {
                    slot: SaveSlotId::Manual1,
                    recovery: false,
                }
            ) || !catalog_capture_is_ready(&catalog_dialogs, &ui_input_state)
            {
                return;
            }
            ui_intents.write(UiIntent::ConfirmLoadCatalogSlot {
                slot: SaveSlotId::Manual1,
                session: save_catalog_ui.session,
            });
            driver.stage = AcceptanceStage::AwaitInitialLoad;
            info!("NATIVE_SAVE_LOAD_ACCEPTANCE: initial load confirmed through UiIntent");
        }
        AcceptanceStage::AwaitInitialLoad => {
            let Some(outcome) = terminal else {
                return;
            };
            if !matches_outcome(outcome, SaveLoadOperation::Load, SaveLoadResult::Succeeded) {
                fail_unexpected_outcome(&mut driver, outcome, &mut exit);
                return;
            }
            let expected_epoch = driver.epoch_before_valid_load.wrapping_add(1);
            if world_epoch.get() != expected_epoch {
                fail_driver(
                    &mut driver,
                    &format!(
                        "successful load did not advance WorldEpoch exactly once (expected {expected_epoch}, got {})",
                        world_epoch.get()
                    ),
                    &mut exit,
                );
                return;
            }
            if !persistent_world_is_ready(
                world_map.as_deref(),
                &souls,
                &familiars,
                &primary_windows,
            ) {
                fail_driver(
                    &mut driver,
                    "successful load did not restore a complete persistent world and primary window",
                    &mut exit,
                );
                return;
            }
            if !virtual_time.is_paused() {
                fail_driver(
                    &mut driver,
                    "successful load unexpectedly resumed virtual time",
                    &mut exit,
                );
                return;
            }
            driver.baseline_epoch = world_epoch.get();
            driver.ready_frames = 0;
            driver.checks[1] = true;
            driver.stage = AcceptanceStage::WaitForLoadedWorld;
            info!("NATIVE_SAVE_LOAD_ACCEPTANCE: waiting for post-load domain convergence");
        }
        AcceptanceStage::WaitForLoadedWorld => {
            if terminal.is_some() {
                fail_driver(
                    &mut driver,
                    "received an outcome while waiting for post-load domain convergence",
                    &mut exit,
                );
                return;
            }
            if world_epoch.get() != driver.baseline_epoch
                || !virtual_time.is_paused()
                || !persistent_world_is_ready(
                    world_map.as_deref(),
                    &souls,
                    &familiars,
                    &primary_windows,
                )
            {
                fail_driver(
                    &mut driver,
                    "loaded world changed epoch, resumed, or became incomplete while converging",
                    &mut exit,
                );
                return;
            }
            driver.ready_frames += 1;
            if driver.ready_frames < READY_FRAMES {
                return;
            }
            ui_intents.write(UiIntent::SaveGame);
            driver.stage = AcceptanceStage::AwaitBaselineSaveCatalog;
            info!("NATIVE_SAVE_LOAD_ACCEPTANCE: paused baseline Save catalog requested");
        }
        AcceptanceStage::AwaitBaselineSaveCatalog => {
            if terminal.is_some() {
                fail_driver(
                    &mut driver,
                    "received an outcome while an occupied Manual 1 catalog was open",
                    &mut exit,
                );
                return;
            }
            if !matches!(save_catalog_ui.mode, SaveCatalogMode::SaveCatalog)
                || !catalog_capture_is_ready(&catalog_dialogs, &ui_input_state)
            {
                return;
            }
            if !catalog_entry_matches(
                &save_catalog,
                SaveSlotId::Manual1,
                SaveSlotRole::Manual,
                SaveContentStatus::CurrentV1 { worldgen_seed: 0 },
                true,
                true,
            ) {
                fail_driver(
                    &mut driver,
                    "occupied Manual 1 did not retain manual overwrite and load capabilities",
                    &mut exit,
                );
                return;
            }
            if !driver.stale_session_checked {
                ui_intents.write(UiIntent::SelectSaveCatalogSlot {
                    slot: SaveSlotId::Manual1,
                    session: save_catalog_ui.session.saturating_add(1),
                });
                driver.stale_session_checked = true;
                return;
            }
            if !save_load_state.is_idle() {
                fail_driver(
                    &mut driver,
                    "a stale catalog session issued a save request",
                    &mut exit,
                );
                return;
            }
            ui_intents.write(UiIntent::SelectSaveCatalogSlot {
                slot: SaveSlotId::Manual1,
                session: save_catalog_ui.session,
            });
            driver.stage = AcceptanceStage::AwaitBaselineOverwriteConfirm;
        }
        AcceptanceStage::AwaitBaselineOverwriteConfirm => {
            if terminal.is_some() {
                fail_driver(
                    &mut driver,
                    "received an outcome before confirming Manual 1 overwrite",
                    &mut exit,
                );
                return;
            }
            if !matches!(
                save_catalog_ui.mode,
                SaveCatalogMode::OverwriteConfirm {
                    slot: SaveSlotId::Manual1,
                }
            ) || !catalog_capture_is_ready(&catalog_dialogs, &ui_input_state)
            {
                return;
            }
            if !driver.confirmation_cancel_checked {
                ui_intents.write(UiIntent::CancelSaveCatalogConfirm);
                driver.confirmation_cancel_checked = true;
                driver.stage = AcceptanceStage::AwaitBaselineSaveCatalog;
                return;
            }
            ui_intents.write(UiIntent::ConfirmSaveCatalogSlot {
                slot: SaveSlotId::Manual1,
                session: save_catalog_ui.session,
            });
            driver.stage = if driver.baseline_save_completed {
                AcceptanceStage::AwaitBaselineConfirmation
            } else {
                AcceptanceStage::AwaitBaselineSave
            };
            info!(
                "NATIVE_SAVE_LOAD_ACCEPTANCE: occupied Manual 1 overwrite confirmed through UiIntent"
            );
        }
        AcceptanceStage::AwaitBaselineSave => {
            let Some(outcome) = terminal else {
                return;
            };
            if !matches_outcome(outcome, SaveLoadOperation::Save, SaveLoadResult::Succeeded) {
                fail_unexpected_outcome(&mut driver, outcome, &mut exit);
                return;
            }
            let baseline =
                match read_persistent_snapshot(&driver.save_path, &type_registry, &asset_server) {
                    Ok(snapshot) => snapshot,
                    Err(error) => {
                        fail_driver(
                            &mut driver,
                            &format!("could not decode baseline save: {error}"),
                            &mut exit,
                        );
                        return;
                    }
                };
            driver.baseline_snapshot = Some(baseline);
            driver.baseline_save_completed = true;
            ui_intents.write(UiIntent::SaveGame);
            driver.stage = AcceptanceStage::AwaitBaselineSaveCatalog;
            info!("NATIVE_SAVE_LOAD_ACCEPTANCE: confirming stable paused baseline through catalog");
        }
        AcceptanceStage::AwaitBaselineConfirmation => {
            let Some(outcome) = terminal else {
                return;
            };
            if !matches_outcome(outcome, SaveLoadOperation::Save, SaveLoadResult::Succeeded) {
                fail_unexpected_outcome(&mut driver, outcome, &mut exit);
                return;
            }
            let confirmed_baseline =
                match read_persistent_snapshot(&driver.save_path, &type_registry, &asset_server) {
                    Ok(snapshot) => snapshot,
                    Err(error) => {
                        fail_driver(
                            &mut driver,
                            &format!("could not decode baseline confirmation: {error}"),
                            &mut exit,
                        );
                        return;
                    }
                };
            let baseline_comparison = driver
                .baseline_snapshot
                .as_ref()
                .ok_or_else(|| "baseline snapshot was missing".to_string())
                .and_then(|baseline| compare_persistent_snapshots(baseline, &confirmed_baseline));
            if let Err(error) = baseline_comparison {
                fail_driver(
                    &mut driver,
                    &format!("paused loaded world did not reach a stable baseline: {error}"),
                    &mut exit,
                );
                return;
            }
            driver.baseline_snapshot = Some(confirmed_baseline);
            driver.checks[0] = true;
            if let Err(error) = prepare_native_catalog_status_fixtures(&driver.save_path) {
                fail_driver(
                    &mut driver,
                    &format!("could not create isolated catalog status fixtures: {error}"),
                    &mut exit,
                );
                return;
            }
            ui_intents.write(UiIntent::RequestLoadGame);
            driver.stage = AcceptanceStage::AwaitInvalidLoadCatalog;
            info!("NATIVE_SAVE_LOAD_ACCEPTANCE: invalid-body load catalog requested");
        }
        AcceptanceStage::AwaitInvalidLoadCatalog => {
            if terminal.is_some() {
                fail_driver(
                    &mut driver,
                    "received an outcome before selecting the invalid-body load",
                    &mut exit,
                );
                return;
            }
            if !matches!(save_catalog_ui.mode, SaveCatalogMode::LoadCatalog)
                || !catalog_capture_is_ready(&catalog_dialogs, &ui_input_state)
            {
                return;
            }
            if !catalog_has_native_status_matrix(&save_catalog) {
                fail_driver(
                    &mut driver,
                    "catalog did not retain the required visible legacy/error status matrix",
                    &mut exit,
                );
                return;
            }
            ui_intents.write(UiIntent::SelectLoadCatalogSlot {
                slot: SaveSlotId::Manual1,
                session: save_catalog_ui.session,
            });
            driver.stage = AcceptanceStage::AwaitInvalidLoadConfirm;
        }
        AcceptanceStage::AwaitInvalidLoadConfirm => {
            if terminal.is_some() {
                fail_driver(
                    &mut driver,
                    "received an outcome before confirming the invalid-body load",
                    &mut exit,
                );
                return;
            }
            if !matches!(
                save_catalog_ui.mode,
                SaveCatalogMode::LoadConfirm {
                    slot: SaveSlotId::Manual1,
                    recovery: false,
                }
            ) || !catalog_capture_is_ready(&catalog_dialogs, &ui_input_state)
            {
                return;
            }
            ui_intents.write(UiIntent::ConfirmLoadCatalogSlot {
                slot: SaveSlotId::Manual1,
                session: save_catalog_ui.session,
            });
            driver.stage = AcceptanceStage::AwaitInvalidLoad;
        }
        AcceptanceStage::AwaitInvalidLoad => {
            let Some(outcome) = terminal else {
                return;
            };
            if !matches_outcome(
                outcome,
                SaveLoadOperation::Load,
                SaveLoadResult::Failed(SaveLoadFailureKind::InvalidData),
            ) {
                fail_unexpected_outcome(&mut driver, outcome, &mut exit);
                return;
            }
            if world_epoch.get() != driver.baseline_epoch {
                fail_driver(
                    &mut driver,
                    "invalid load advanced the world epoch",
                    &mut exit,
                );
                return;
            }
            if !catalog_entry_matches(
                &save_catalog,
                SaveSlotId::Manual1,
                SaveSlotRole::Manual,
                SaveContentStatus::BodyInvalid,
                true,
                true,
            ) {
                fail_driver(
                    &mut driver,
                    "invalid full load did not update Manual 1 to BodyInvalid while retaining overwrite",
                    &mut exit,
                );
                return;
            }
            ui_intents.write(UiIntent::SaveGame);
            driver.stage = AcceptanceStage::AwaitPostInvalidSaveCatalog;
            info!("NATIVE_SAVE_LOAD_ACCEPTANCE: BodyInvalid Manual 1 overwrite catalog requested");
        }
        AcceptanceStage::AwaitPostInvalidSaveCatalog => {
            if terminal.is_some() {
                fail_driver(
                    &mut driver,
                    "received an outcome before choosing the BodyInvalid overwrite",
                    &mut exit,
                );
                return;
            }
            if !matches!(save_catalog_ui.mode, SaveCatalogMode::SaveCatalog)
                || !catalog_capture_is_ready(&catalog_dialogs, &ui_input_state)
            {
                return;
            }
            if !catalog_entry_matches(
                &save_catalog,
                SaveSlotId::Manual1,
                SaveSlotRole::Manual,
                SaveContentStatus::BodyInvalid,
                true,
                true,
            ) {
                fail_driver(
                    &mut driver,
                    "BodyInvalid Manual 1 disappeared from the overwrite catalog",
                    &mut exit,
                );
                return;
            }
            ui_intents.write(UiIntent::SelectSaveCatalogSlot {
                slot: SaveSlotId::Manual1,
                session: save_catalog_ui.session,
            });
            driver.stage = AcceptanceStage::AwaitPostInvalidOverwriteConfirm;
        }
        AcceptanceStage::AwaitPostInvalidOverwriteConfirm => {
            if terminal.is_some() {
                fail_driver(
                    &mut driver,
                    "received an outcome before confirming the BodyInvalid overwrite",
                    &mut exit,
                );
                return;
            }
            if !matches!(
                save_catalog_ui.mode,
                SaveCatalogMode::OverwriteConfirm {
                    slot: SaveSlotId::Manual1,
                }
            ) || !catalog_capture_is_ready(&catalog_dialogs, &ui_input_state)
            {
                return;
            }
            ui_intents.write(UiIntent::ConfirmSaveCatalogSlot {
                slot: SaveSlotId::Manual1,
                session: save_catalog_ui.session,
            });
            driver.stage = AcceptanceStage::AwaitPostInvalidSave;
        }
        AcceptanceStage::AwaitPostInvalidSave => {
            let Some(outcome) = terminal else {
                return;
            };
            if !matches_outcome(outcome, SaveLoadOperation::Save, SaveLoadResult::Succeeded) {
                fail_unexpected_outcome(&mut driver, outcome, &mut exit);
                return;
            }
            let post_rejection =
                match read_persistent_snapshot(&driver.save_path, &type_registry, &asset_server) {
                    Ok(snapshot) => snapshot,
                    Err(error) => {
                        fail_driver(
                            &mut driver,
                            &format!("could not decode post-rejection save: {error}"),
                            &mut exit,
                        );
                        return;
                    }
                };
            let rejection_comparison = driver
                .baseline_snapshot
                .as_ref()
                .ok_or_else(|| "baseline snapshot was missing".to_string())
                .and_then(|baseline| compare_persistent_snapshots(baseline, &post_rejection));
            if let Err(error) = rejection_comparison {
                fail_driver(
                    &mut driver,
                    &format!("invalid load changed the paused persistent world: {error}"),
                    &mut exit,
                );
                return;
            }
            driver.checks[2] = true;
            ui_intents.write(UiIntent::RequestLoadGame);
            driver.stage = AcceptanceStage::AwaitV4LoadCatalog;
        }
        AcceptanceStage::AwaitV4LoadCatalog => {
            if terminal.is_some() {
                fail_driver(
                    &mut driver,
                    "received an outcome while exercising Load catalog capture",
                    &mut exit,
                );
                return;
            }
            if !matches!(save_catalog_ui.mode, SaveCatalogMode::LoadCatalog)
                || !catalog_capture_is_ready(&catalog_dialogs, &ui_input_state)
            {
                return;
            }
            ui_intents.write(UiIntent::SelectLoadCatalogSlot {
                slot: SaveSlotId::Manual1,
                session: save_catalog_ui.session,
            });
            driver.stage = AcceptanceStage::AwaitV4LoadConfirm;
        }
        AcceptanceStage::AwaitV4LoadConfirm => {
            if terminal.is_some() {
                fail_driver(
                    &mut driver,
                    "received an outcome while exercising Load confirmation capture",
                    &mut exit,
                );
                return;
            }
            if !matches!(
                save_catalog_ui.mode,
                SaveCatalogMode::LoadConfirm {
                    slot: SaveSlotId::Manual1,
                    recovery: false,
                }
            ) || !catalog_capture_is_ready(&catalog_dialogs, &ui_input_state)
            {
                return;
            }
            // The physical Esc binding is asserted by resolver tests. The
            // actual window proves its production intent target returns the
            // foreground confirmation to the owning catalog.
            ui_intents.write(UiIntent::CancelLoadConfirm);
            driver.stage = AcceptanceStage::AwaitV4CatalogAfterCancel;
        }
        AcceptanceStage::AwaitV4CatalogAfterCancel => {
            if terminal.is_some() {
                fail_driver(
                    &mut driver,
                    "canceling Load confirmation unexpectedly issued a transaction",
                    &mut exit,
                );
                return;
            }
            if !matches!(save_catalog_ui.mode, SaveCatalogMode::LoadCatalog)
                || !catalog_capture_is_ready(&catalog_dialogs, &ui_input_state)
            {
                return;
            }
            ui_intents.write(UiIntent::CancelLoadConfirm);
            driver.stage = AcceptanceStage::AwaitV4Closed;
        }
        AcceptanceStage::AwaitV4Closed => {
            if terminal.is_some() {
                fail_driver(
                    &mut driver,
                    "closing Load catalog unexpectedly issued a transaction",
                    &mut exit,
                );
                return;
            }
            if !matches!(save_catalog_ui.mode, SaveCatalogMode::Closed)
                || !catalog_capture_is_released(&catalog_dialogs, &ui_input_state)
            {
                return;
            }
            driver.checks[3] = true;
            ui_intents.write(UiIntent::SaveGame);
            driver.stage = AcceptanceStage::AwaitV5BackupSaveCatalog;
        }
        AcceptanceStage::AwaitV5BackupSaveCatalog => {
            if terminal.is_some() {
                fail_driver(
                    &mut driver,
                    "received an outcome before opening the alternate-slot Save catalog",
                    &mut exit,
                );
                return;
            }
            if !matches!(save_catalog_ui.mode, SaveCatalogMode::SaveCatalog)
                || !catalog_capture_is_ready(&catalog_dialogs, &ui_input_state)
            {
                return;
            }
            if !catalog_entry_matches(
                &save_catalog,
                SaveSlotId::Manual2,
                SaveSlotRole::Manual,
                SaveContentStatus::Empty,
                true,
                false,
            ) {
                fail_driver(
                    &mut driver,
                    "alternate Manual 2 slot was not an empty selectable save target",
                    &mut exit,
                );
                return;
            }
            ui_intents.write(UiIntent::SelectSaveCatalogSlot {
                slot: SaveSlotId::Manual2,
                session: save_catalog_ui.session,
            });
            driver.stage = AcceptanceStage::AwaitV5BackupSave;
        }
        AcceptanceStage::AwaitV5BackupSave => {
            let Some(outcome) = terminal else {
                return;
            };
            if !matches_outcome(outcome, SaveLoadOperation::Save, SaveLoadResult::Succeeded) {
                fail_unexpected_outcome(&mut driver, outcome, &mut exit);
                return;
            }
            native_faults.arm(NativeLoadFault::ApplyRecovered);
            ui_intents.write(UiIntent::RequestLoadGame);
            driver.stage = AcceptanceStage::AwaitV5ApplyRecoveredCatalog;
        }
        AcceptanceStage::AwaitV5ApplyRecoveredCatalog => {
            if terminal.is_some() {
                fail_driver(
                    &mut driver,
                    "received an outcome before arming the ApplyRecovered load",
                    &mut exit,
                );
                return;
            }
            if !matches!(save_catalog_ui.mode, SaveCatalogMode::LoadCatalog)
                || !catalog_capture_is_ready(&catalog_dialogs, &ui_input_state)
            {
                return;
            }
            ui_intents.write(UiIntent::SelectLoadCatalogSlot {
                slot: SaveSlotId::Manual1,
                session: save_catalog_ui.session,
            });
            driver.stage = AcceptanceStage::AwaitV5ApplyRecoveredConfirm;
        }
        AcceptanceStage::AwaitV5ApplyRecoveredConfirm => {
            if terminal.is_some() {
                fail_driver(
                    &mut driver,
                    "received an outcome before confirming the ApplyRecovered load",
                    &mut exit,
                );
                return;
            }
            if !matches!(
                save_catalog_ui.mode,
                SaveCatalogMode::LoadConfirm {
                    slot: SaveSlotId::Manual1,
                    recovery: false,
                }
            ) || !catalog_capture_is_ready(&catalog_dialogs, &ui_input_state)
            {
                return;
            }
            ui_intents.write(UiIntent::ConfirmLoadCatalogSlot {
                slot: SaveSlotId::Manual1,
                session: save_catalog_ui.session,
            });
            driver.stage = AcceptanceStage::AwaitV5ApplyRecovered;
        }
        AcceptanceStage::AwaitV5ApplyRecovered => {
            let Some(outcome) = terminal else {
                return;
            };
            if !matches_outcome(
                outcome,
                SaveLoadOperation::Load,
                SaveLoadResult::Failed(SaveLoadFailureKind::ApplyRecovered),
            ) {
                fail_unexpected_outcome(&mut driver, outcome, &mut exit);
                return;
            }
            if *save_recovery_mode != SaveRecoveryMode::Healthy || !virtual_time.is_paused() {
                fail_driver(
                    &mut driver,
                    "ApplyRecovered did not retain a healthy, paused rollback world",
                    &mut exit,
                );
                return;
            }
            driver.stage = AcceptanceStage::AwaitV5ApplyRecoveredReset;
        }
        AcceptanceStage::AwaitV5ApplyRecoveredReset => {
            if terminal.is_some() {
                fail_driver(
                    &mut driver,
                    "ApplyRecovered emitted more than one terminal outcome",
                    &mut exit,
                );
                return;
            }
            if !matches!(save_catalog_ui.mode, SaveCatalogMode::Closed)
                || !catalog_capture_is_released(&catalog_dialogs, &ui_input_state)
            {
                return;
            }
            native_faults.arm(NativeLoadFault::RecoveryFailedNormalApply);
            ui_intents.write(UiIntent::RequestLoadGame);
            driver.stage = AcceptanceStage::AwaitV5RecoveryFailureCatalog;
        }
        AcceptanceStage::AwaitV5RecoveryFailureCatalog => {
            if terminal.is_some() {
                fail_driver(
                    &mut driver,
                    "received an outcome before confirming the rollback-failure load",
                    &mut exit,
                );
                return;
            }
            if !matches!(save_catalog_ui.mode, SaveCatalogMode::LoadCatalog)
                || !catalog_capture_is_ready(&catalog_dialogs, &ui_input_state)
            {
                return;
            }
            ui_intents.write(UiIntent::SelectLoadCatalogSlot {
                slot: SaveSlotId::Manual1,
                session: save_catalog_ui.session,
            });
            driver.stage = AcceptanceStage::AwaitV5RecoveryFailureConfirm;
        }
        AcceptanceStage::AwaitV5RecoveryFailureConfirm => {
            if terminal.is_some() {
                fail_driver(
                    &mut driver,
                    "received an outcome before executing the rollback-failure load",
                    &mut exit,
                );
                return;
            }
            if !matches!(
                save_catalog_ui.mode,
                SaveCatalogMode::LoadConfirm {
                    slot: SaveSlotId::Manual1,
                    recovery: false,
                }
            ) || !catalog_capture_is_ready(&catalog_dialogs, &ui_input_state)
            {
                return;
            }
            ui_intents.write(UiIntent::ConfirmLoadCatalogSlot {
                slot: SaveSlotId::Manual1,
                session: save_catalog_ui.session,
            });
            driver.stage = AcceptanceStage::AwaitV5RecoveryFailure;
        }
        AcceptanceStage::AwaitV5RecoveryFailure => {
            let Some(outcome) = terminal else {
                return;
            };
            if !matches_outcome(
                outcome,
                SaveLoadOperation::Load,
                SaveLoadResult::Failed(SaveLoadFailureKind::RecoveryFailed),
            ) {
                fail_unexpected_outcome(&mut driver, outcome, &mut exit);
                return;
            }
            if *save_recovery_mode != SaveRecoveryMode::RecoveryFailed || !virtual_time.is_paused()
            {
                fail_driver(
                    &mut driver,
                    "rollback failure did not enter the paused RecoveryFailed boundary",
                    &mut exit,
                );
                return;
            }
            // Direct raw intents prove the root ingress rejects save/resume;
            // only RequestLoadGame may open the recovery catalog.
            ui_intents.write(UiIntent::SaveGame);
            ui_intents.write(UiIntent::TogglePause);
            ui_intents.write(UiIntent::RequestLoadGame);
            driver.stage = AcceptanceStage::AwaitV5RecoveryCatalog;
        }
        AcceptanceStage::AwaitV5RecoveryCatalog => {
            if terminal.is_some() {
                fail_driver(
                    &mut driver,
                    "RecoveryFailed raw intents unexpectedly issued a transaction",
                    &mut exit,
                );
                return;
            }
            if *save_recovery_mode != SaveRecoveryMode::RecoveryFailed
                || !virtual_time.is_paused()
                || !save_load_state.is_idle()
            {
                fail_driver(
                    &mut driver,
                    "RecoveryFailed allowed a save/resume request before recovery catalog ownership",
                    &mut exit,
                );
                return;
            }
            if !matches!(save_catalog_ui.mode, SaveCatalogMode::RecoveryLoadCatalog)
                || !catalog_capture_is_ready(&catalog_dialogs, &ui_input_state)
            {
                return;
            }
            if let Err(error) = replace_save_with_invalid_body(&driver.save_path) {
                fail_driver(
                    &mut driver,
                    &format!("could not prepare recovery-only invalid body fixture: {error}"),
                    &mut exit,
                );
                return;
            }
            ui_intents.write(UiIntent::SelectLoadCatalogSlot {
                slot: SaveSlotId::Manual1,
                session: save_catalog_ui.session,
            });
            driver.stage = AcceptanceStage::AwaitV5RecoveryInvalidConfirm;
        }
        AcceptanceStage::AwaitV5RecoveryInvalidConfirm => {
            if terminal.is_some() {
                fail_driver(
                    &mut driver,
                    "received an outcome before confirming invalid recovery-only load",
                    &mut exit,
                );
                return;
            }
            if !matches!(
                save_catalog_ui.mode,
                SaveCatalogMode::LoadConfirm {
                    slot: SaveSlotId::Manual1,
                    recovery: true,
                }
            ) || !catalog_capture_is_ready(&catalog_dialogs, &ui_input_state)
            {
                return;
            }
            ui_intents.write(UiIntent::ConfirmLoadCatalogSlot {
                slot: SaveSlotId::Manual1,
                session: save_catalog_ui.session,
            });
            driver.stage = AcceptanceStage::AwaitV5RecoveryInvalid;
        }
        AcceptanceStage::AwaitV5RecoveryInvalid => {
            let Some(outcome) = terminal else {
                return;
            };
            if !matches_outcome(
                outcome,
                SaveLoadOperation::Load,
                SaveLoadResult::Failed(SaveLoadFailureKind::InvalidData),
            ) {
                fail_unexpected_outcome(&mut driver, outcome, &mut exit);
                return;
            }
            if *save_recovery_mode != SaveRecoveryMode::RecoveryFailed || !virtual_time.is_paused()
            {
                fail_driver(
                    &mut driver,
                    "invalid recovery-only load did not remain fail-closed",
                    &mut exit,
                );
                return;
            }
            driver.stage = AcceptanceStage::AwaitV5RecoveryValidCatalog;
        }
        AcceptanceStage::AwaitV5RecoveryValidCatalog => {
            if terminal.is_some() {
                fail_driver(
                    &mut driver,
                    "invalid recovery-only load emitted an unexpected second outcome",
                    &mut exit,
                );
                return;
            }
            if !matches!(save_catalog_ui.mode, SaveCatalogMode::RecoveryLoadCatalog)
                || !catalog_capture_is_ready(&catalog_dialogs, &ui_input_state)
            {
                return;
            }
            if !catalog_entry_matches(
                &save_catalog,
                SaveSlotId::Manual2,
                SaveSlotRole::Manual,
                SaveContentStatus::CurrentV1 { worldgen_seed: 0 },
                true,
                true,
            ) {
                fail_driver(
                    &mut driver,
                    "Recovery catalog did not retain alternate Manual 2 as a valid recovery target",
                    &mut exit,
                );
                return;
            }
            if !driver.recovery_apply_failure_checked {
                native_faults.arm(NativeLoadFault::RecoveryOnlyApply);
            }
            ui_intents.write(UiIntent::SelectLoadCatalogSlot {
                slot: SaveSlotId::Manual2,
                session: save_catalog_ui.session,
            });
            driver.stage = if driver.recovery_apply_failure_checked {
                AcceptanceStage::AwaitV5RecoveryValidConfirm
            } else {
                AcceptanceStage::AwaitV5RecoveryApplyFailureConfirm
            };
        }
        AcceptanceStage::AwaitV5RecoveryApplyFailureConfirm => {
            if terminal.is_some() {
                fail_driver(
                    &mut driver,
                    "received an outcome before confirming recovery-only apply failure",
                    &mut exit,
                );
                return;
            }
            if !matches!(
                save_catalog_ui.mode,
                SaveCatalogMode::LoadConfirm {
                    slot: SaveSlotId::Manual2,
                    recovery: true,
                }
            ) || !catalog_capture_is_ready(&catalog_dialogs, &ui_input_state)
            {
                return;
            }
            ui_intents.write(UiIntent::ConfirmLoadCatalogSlot {
                slot: SaveSlotId::Manual2,
                session: save_catalog_ui.session,
            });
            driver.stage = AcceptanceStage::AwaitV5RecoveryApplyFailure;
        }
        AcceptanceStage::AwaitV5RecoveryApplyFailure => {
            let Some(outcome) = terminal else {
                return;
            };
            if !matches_outcome(
                outcome,
                SaveLoadOperation::Load,
                SaveLoadResult::Failed(SaveLoadFailureKind::RecoveryFailed),
            ) {
                fail_unexpected_outcome(&mut driver, outcome, &mut exit);
                return;
            }
            if *save_recovery_mode != SaveRecoveryMode::RecoveryFailed || !virtual_time.is_paused()
            {
                fail_driver(
                    &mut driver,
                    "failed recovery-only live apply did not remain fail-closed",
                    &mut exit,
                );
                return;
            }
            driver.recovery_apply_failure_checked = true;
            ui_intents.write(UiIntent::RequestLoadGame);
            driver.stage = AcceptanceStage::AwaitV5RecoveryValidCatalog;
        }
        AcceptanceStage::AwaitV5RecoveryValidConfirm => {
            if terminal.is_some() {
                fail_driver(
                    &mut driver,
                    "received an outcome before confirming the alternate recovery load",
                    &mut exit,
                );
                return;
            }
            if !matches!(
                save_catalog_ui.mode,
                SaveCatalogMode::LoadConfirm {
                    slot: SaveSlotId::Manual2,
                    recovery: true,
                }
            ) || !catalog_capture_is_ready(&catalog_dialogs, &ui_input_state)
            {
                return;
            }
            ui_intents.write(UiIntent::ConfirmLoadCatalogSlot {
                slot: SaveSlotId::Manual2,
                session: save_catalog_ui.session,
            });
            driver.stage = AcceptanceStage::AwaitV5RecoveryValid;
        }
        AcceptanceStage::AwaitV5RecoveryValid => {
            let Some(outcome) = terminal else {
                return;
            };
            if !matches_outcome(outcome, SaveLoadOperation::Load, SaveLoadResult::Succeeded) {
                fail_unexpected_outcome(&mut driver, outcome, &mut exit);
                return;
            }
            if *save_recovery_mode != SaveRecoveryMode::Healthy || !virtual_time.is_paused() {
                fail_driver(
                    &mut driver,
                    "successful recovery-only load did not restore Healthy while remaining paused",
                    &mut exit,
                );
                return;
            }
            ui_intents.write(UiIntent::TogglePause);
            driver.stage = AcceptanceStage::AwaitV5Resume;
        }
        AcceptanceStage::AwaitV5Resume => {
            if terminal.is_some() {
                fail_driver(
                    &mut driver,
                    "resume after recovery unexpectedly issued a transaction",
                    &mut exit,
                );
                return;
            }
            if virtual_time.is_paused() {
                return;
            }
            ui_intents.write(UiIntent::TogglePause);
            driver.stage = AcceptanceStage::AwaitV5Repaused;
        }
        AcceptanceStage::AwaitV5Repaused => {
            if terminal.is_some() {
                fail_driver(
                    &mut driver,
                    "re-pausing after recovery unexpectedly issued a transaction",
                    &mut exit,
                );
                return;
            }
            if !virtual_time.is_paused() {
                return;
            }
            driver.checks[4] = true;
            ui_intents.write(UiIntent::SaveGame);
            driver.stage = AcceptanceStage::AwaitCaptureCatalog;
        }
        AcceptanceStage::AwaitCaptureCatalog => {
            if terminal.is_some() {
                fail_driver(
                    &mut driver,
                    "received an outcome while opening the final Save catalog for capture",
                    &mut exit,
                );
                return;
            }
            if !virtual_time.is_paused() {
                fail_driver(
                    &mut driver,
                    "virtual time resumed while opening the final Save catalog for capture",
                    &mut exit,
                );
                return;
            }
            if !matches!(save_catalog_ui.mode, SaveCatalogMode::SaveCatalog)
                || !catalog_capture_is_ready(&catalog_dialogs, &ui_input_state)
            {
                return;
            }
            commands.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(12.0),
                    top: Val::Px(12.0),
                    width: Val::Px(48.0),
                    height: Val::Px(48.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(1.0, 0.0, 1.0)),
                ZIndex(10_000),
                Name::new("Native Save Catalog Capture Marker"),
                NativeSaveCatalogCaptureMarker,
            ));
            driver.ready_frames = 0;
            driver.stage = AcceptanceStage::AwaitCaptureMarker;
        }
        AcceptanceStage::AwaitCaptureMarker => {
            if terminal.is_some() {
                fail_driver(
                    &mut driver,
                    "received an outcome while waiting for the final catalog marker",
                    &mut exit,
                );
                return;
            }
            if !final_catalog_capture_is_held(
                virtual_time.is_paused(),
                save_catalog_ui.mode,
                catalog_capture_is_ready(&catalog_dialogs, &ui_input_state),
            ) {
                fail_driver(
                    &mut driver,
                    "final Save catalog lost paused foreground capture before renderer evidence",
                    &mut exit,
                );
                return;
            }
            driver.ready_frames += 1;
            if driver.ready_frames < CAPTURE_MARKER_FRAMES {
                return;
            }
            if let Err(error) = write_capture_ready(&driver.capture_ready_path(), &driver.run_id) {
                fail_driver(
                    &mut driver,
                    &format!("could not publish the capture-ready marker: {error}"),
                    &mut exit,
                );
                return;
            }
            driver.stage = AcceptanceStage::AwaitCapture;
            info!(
                "NATIVE_SAVE_LOAD_ACCEPTANCE: V1-V5 checks passed; final Save catalog awaiting screenshot"
            );
        }
        AcceptanceStage::AwaitCapture => {
            if terminal.is_some() {
                fail_driver(
                    &mut driver,
                    "received an outcome while awaiting the renderer capture",
                    &mut exit,
                );
                return;
            }
            if !final_catalog_capture_is_held(
                virtual_time.is_paused(),
                save_catalog_ui.mode,
                catalog_capture_is_ready(&catalog_dialogs, &ui_input_state),
            ) {
                fail_driver(
                    &mut driver,
                    "final Save catalog lost paused foreground capture while awaiting renderer evidence",
                    &mut exit,
                );
                return;
            }
            if driver.checks.iter().any(|passed| !passed) {
                fail_driver(
                    &mut driver,
                    "native save-catalog driver reached renderer capture before every V1-V5 check passed",
                    &mut exit,
                );
                return;
            }
            let renderer = match driver.render_evidence.snapshot() {
                RenderEvidenceState::Ready(renderer) => renderer,
                RenderEvidenceState::Pending => return,
                RenderEvidenceState::Failed(error) => {
                    fail_driver(
                        &mut driver,
                        &format!("could not observe actual renderer environment: {error}"),
                        &mut exit,
                    );
                    return;
                }
            };
            let evidence = match read_capture_evidence(&driver) {
                Ok(Some(evidence)) => evidence,
                Ok(None) => return,
                Err(error) => {
                    fail_driver(&mut driver, &error, &mut exit);
                    return;
                }
            };
            let recovery_save_bytes = match final_recovery_save_bytes(&driver) {
                Ok(bytes) => bytes,
                Err(error) => {
                    fail_driver(&mut driver, &error, &mut exit);
                    return;
                }
            };
            let result = SaveCatalogAcceptanceResult {
                run_id: &driver.run_id,
                save_bytes: recovery_save_bytes,
                world_epoch_before: driver.epoch_before_valid_load,
                world_epoch_after: driver.baseline_epoch,
                checks: driver.checks,
                runtime_root: &driver.runtime_root,
                renderer: &renderer,
                evidence: &evidence,
            };
            if let Err(error) = write_success_result(&driver.result_path(), &result) {
                fail_driver(
                    &mut driver,
                    &format!("could not publish the terminal acceptance result: {error}"),
                    &mut exit,
                );
                return;
            }
            driver.stage = AcceptanceStage::Finished;
            info!("NATIVE_SAVE_LOAD_ACCEPTANCE: PASS");
            exit.write(AppExit::Success);
        }
        AcceptanceStage::Finished => {}
    }
}

fn read_persistent_snapshot(
    path: &Path,
    type_registry: &AppTypeRegistry,
    asset_server: &AssetServer,
) -> Result<PersistentSnapshot, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    if contents.is_empty() {
        return Err("save file was empty".to_string());
    }
    let decoded =
        decode_save_file(&contents).map_err(|error| format!("save header was invalid: {error}"))?;
    let registry = type_registry.read();
    let mut ron_deserializer = ron::de::Deserializer::from_str(decoded.body)
        .map_err(|error| format!("save body syntax was invalid: {error}"))?;
    let mut asset_server = asset_server.clone();
    let world = {
        use serde::de::DeserializeSeed;
        WorldDeserializer {
            type_registry: &registry,
            load_from_path: &mut asset_server,
        }
        .deserialize(&mut ron_deserializer)
        .map_err(|error| format!("save body could not be deserialized: {error}"))?
    };
    ron_deserializer
        .end()
        .map_err(|error| format!("save body had trailing data: {error}"))?;
    Ok(PersistentSnapshot {
        format: decoded.format,
        world,
    })
}

/// V5 intentionally leaves Manual 1 as a rejected recovery probe. The final
/// durable artifact is the separate Manual 2 target that successfully restores
/// the Healthy world, so result bytes must describe that target rather than
/// the deliberately-invalid probe file.
fn final_recovery_save_bytes(driver: &NativeSaveLoadAcceptance) -> Result<usize, String> {
    let path = driver
        .runtime_root
        .join("saves")
        .join(SaveSlotId::Manual2.canonical_file_name());
    let metadata = fs::metadata(&path).map_err(|error| {
        format!(
            "could not inspect final recovery save {}: {error}",
            path.display()
        )
    })?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Err(format!(
            "final recovery save is not a nonempty regular file: {}",
            path.display()
        ));
    }
    usize::try_from(metadata.len()).map_err(|_| {
        format!(
            "final recovery save is too large for this platform: {}",
            path.display()
        )
    })
}

fn compare_persistent_snapshots(
    baseline: &PersistentSnapshot,
    candidate: &PersistentSnapshot,
) -> Result<(), String> {
    if baseline.format != candidate.format {
        return Err("save format/header changed".to_string());
    }
    compare_reflected_values(
        "resources",
        &baseline.world.resources,
        &candidate.world.resources,
    )?;

    let mut baseline_entities = baseline.world.entities.iter().collect::<Vec<_>>();
    let mut candidate_entities = candidate.world.entities.iter().collect::<Vec<_>>();
    baseline_entities.sort_by_key(|entity| entity.entity.to_bits());
    candidate_entities.sort_by_key(|entity| entity.entity.to_bits());
    if baseline_entities
        .windows(2)
        .any(|pair| pair[0].entity == pair[1].entity)
        || candidate_entities
            .windows(2)
            .any(|pair| pair[0].entity == pair[1].entity)
    {
        return Err("snapshot contained duplicate entity identifiers".to_string());
    }
    if baseline_entities.len() != candidate_entities.len() {
        return Err(format!(
            "entity count changed from {} to {}",
            baseline_entities.len(),
            candidate_entities.len()
        ));
    }
    for (baseline_entity, candidate_entity) in baseline_entities.into_iter().zip(candidate_entities)
    {
        if baseline_entity.entity != candidate_entity.entity {
            return Err(format!(
                "entity identity changed at {}",
                baseline_entity.entity.to_bits()
            ));
        }
        compare_reflected_values(
            &format!("entity {} components", baseline_entity.entity.to_bits()),
            &baseline_entity.components,
            &candidate_entity.components,
        )?;
    }
    Ok(())
}

fn compare_reflected_values(
    label: &str,
    baseline: &[Box<dyn PartialReflect>],
    candidate: &[Box<dyn PartialReflect>],
) -> Result<(), String> {
    let mut baseline = reflected_values_by_type(baseline)?;
    let mut candidate = reflected_values_by_type(candidate)?;
    baseline.sort_by_key(|(_, type_path, _)| *type_path);
    candidate.sort_by_key(|(_, type_path, _)| *type_path);
    if baseline.windows(2).any(|pair| pair[0].0 == pair[1].0)
        || candidate.windows(2).any(|pair| pair[0].0 == pair[1].0)
    {
        return Err(format!("{label} contained duplicate represented types"));
    }
    if baseline.len() != candidate.len() {
        return Err(format!(
            "{label} count changed from {} to {}",
            baseline.len(),
            candidate.len()
        ));
    }
    for (
        (baseline_type_id, baseline_type, baseline_value),
        (candidate_type_id, candidate_type, candidate_value),
    ) in baseline.into_iter().zip(candidate)
    {
        if baseline_type_id != candidate_type_id || baseline_type != candidate_type {
            return Err(format!(
                "{label} type changed from {baseline_type} to {candidate_type}"
            ));
        }
        if baseline_value.reflect_partial_eq(candidate_value) != Some(true) {
            return Err(format!("{label} value changed for {baseline_type}"));
        }
    }
    Ok(())
}

type ReflectedValueByType<'a> = (TypeId, &'static str, &'a dyn PartialReflect);

fn reflected_values_by_type(
    values: &[Box<dyn PartialReflect>],
) -> Result<Vec<ReflectedValueByType<'_>>, String> {
    values
        .iter()
        .map(|value| {
            let type_info = value
                .get_represented_type_info()
                .ok_or_else(|| "persistent reflected value had no represented type".to_string())?;
            Ok((type_info.type_id(), type_info.type_path(), value.as_ref()))
        })
        .collect()
}

fn persistent_world_is_ready(
    world_map: Option<&WorldMap>,
    souls: &Query<'_, '_, (), With<DamnedSoul>>,
    familiars: &Query<'_, '_, (), With<Familiar>>,
    primary_windows: &Query<'_, '_, (), With<PrimaryWindow>>,
) -> bool {
    world_map.is_some_and(|map| {
        !map.tile_entities.is_empty() && map.tile_entities.iter().all(Option::is_none)
    }) && !souls.is_empty()
        && !familiars.is_empty()
        && primary_windows.iter().count() == 1
}

fn catalog_capture_is_ready(
    dialogs: &Query<'_, '_, (Entity, &Node), With<SaveCatalogDialog>>,
    ui_input_state: &UiInputState,
) -> bool {
    let mut visible = dialogs
        .iter()
        .filter_map(|(entity, node)| (node.display != Display::None).then_some(entity));
    let Some(root) = visible.next() else {
        return false;
    };
    ui_input_state.world_input_captured
        && ui_input_state.foreground_capture_root == Some(root)
        && visible.next().is_none()
}

/// The final renderer screenshot is meaningful only while the visible Save
/// catalog still owns foreground capture. The marker remains mounted until
/// the acknowledgement arrives, so it cannot establish this on its own.
fn final_catalog_capture_is_held(
    virtual_time_paused: bool,
    mode: SaveCatalogMode,
    catalog_capture_ready: bool,
) -> bool {
    virtual_time_paused && matches!(mode, SaveCatalogMode::SaveCatalog) && catalog_capture_ready
}

/// Closing a catalog while paused correctly leaves the Pause overlay as the
/// foreground input owner. Acceptance must therefore prove that the catalog
/// root released capture, rather than requiring *all* world capture to end.
fn catalog_capture_is_released(
    dialogs: &Query<'_, '_, (Entity, &Node), With<SaveCatalogDialog>>,
    ui_input_state: &UiInputState,
) -> bool {
    dialogs.iter().all(|(entity, node)| {
        node.display == Display::None && ui_input_state.foreground_capture_root != Some(entity)
    })
}

fn catalog_entry_matches(
    catalog: &SaveCatalog,
    slot: SaveSlotId,
    role: SaveSlotRole,
    expected_content: SaveContentStatus,
    can_manual_save: bool,
    can_load: bool,
) -> bool {
    catalog.entry(slot).is_some_and(|entry| {
        entry.role == role
            && same_native_content_class(entry.content, expected_content)
            && entry.capabilities.can_manual_save == can_manual_save
            && entry.capabilities.can_load == can_load
    })
}

fn same_native_content_class(actual: SaveContentStatus, expected: SaveContentStatus) -> bool {
    matches!(
        (actual, expected),
        (
            SaveContentStatus::CurrentV1 { .. },
            SaveContentStatus::CurrentV1 { .. }
        ) | (
            SaveContentStatus::SeedMismatch { .. },
            SaveContentStatus::SeedMismatch { .. }
        )
    ) || actual == expected
}

fn catalog_has_native_status_matrix(catalog: &SaveCatalog) -> bool {
    catalog
        .entry(SaveSlotId::LegacyDefault)
        .is_some_and(|entry| {
            entry.content == SaveContentStatus::LegacyV0Candidate && entry.capabilities.can_load
        })
        && catalog.entry(SaveSlotId::Manual3).is_some_and(|entry| {
            entry.content == SaveContentStatus::CorruptHeader
                && entry.capabilities.can_manual_save
                && !entry.capabilities.can_load
        })
        && catalog.entry(SaveSlotId::Autosave1).is_some_and(|entry| {
            entry.content == SaveContentStatus::Unreadable && !entry.capabilities.can_load
        })
        && catalog.entry(SaveSlotId::Autosave2).is_some_and(|entry| {
            matches!(entry.content, SaveContentStatus::UnsupportedVersion { .. })
                && !entry.capabilities.can_load
        })
        && catalog.entry(SaveSlotId::Autosave3).is_some_and(|entry| {
            matches!(entry.content, SaveContentStatus::SeedMismatch { .. })
                && !entry.capabilities.can_load
        })
}

/// Creates bounded, artifact-local C2 catalog fixtures after the two valid
/// manual slots exist. It deliberately preserves the v1 header on Manual 1 so
/// the UI may select it and the full loader alone discovers the invalid body.
fn prepare_native_catalog_status_fixtures(manual_one: &Path) -> Result<(), String> {
    let root = manual_one
        .parent()
        .ok_or_else(|| "Manual 1 save path had no storage root".to_owned())?;
    let original = fs::read_to_string(manual_one)
        .map_err(|error| format!("could not read Manual 1 fixture: {error}"))?;
    let header = match decode_save_file(&original)
        .map_err(|error| format!("could not decode Manual 1 fixture header: {error}"))?
        .format
    {
        SaveFormat::V1(header) => header,
        SaveFormat::LegacyV0 => {
            return Err("Manual 1 fixture unexpectedly used the legacy container".to_owned());
        }
    };

    fs::write(
        manual_one,
        format!(
            "{}this is intentionally not a DynamicWorld body\n",
            encode_save_header_text(header)
        ),
    )
    .map_err(|error| format!("could not prepare invalid Manual 1 body: {error}"))?;
    fs::write(
        root.join(SaveSlotId::Manual3.canonical_file_name()),
        format!("{SAVE_MAGIC}\nmalformed header\n---\nbody\n"),
    )
    .map_err(|error| format!("could not prepare corrupt Manual 3 header: {error}"))?;
    fs::create_dir(root.join(SaveSlotId::Autosave1.canonical_file_name()))
        .map_err(|error| format!("could not prepare unreadable Autosave 1 fixture: {error}"))?;
    fs::write(
        root.join(SaveSlotId::Autosave2.canonical_file_name()),
        format!(
            "{}body\n",
            encode_save_header_text(SaveHeader {
                format_version: CURRENT_SAVE_FORMAT_VERSION.saturating_add(1),
                worldgen_seed: header.worldgen_seed,
            })
        ),
    )
    .map_err(|error| format!("could not prepare unsupported Autosave 2 header: {error}"))?;
    fs::write(
        root.join(SaveSlotId::Autosave3.canonical_file_name()),
        format!(
            "{}body\n",
            encode_save_header_text(SaveHeader::current(header.worldgen_seed.wrapping_add(1)))
        ),
    )
    .map_err(|error| format!("could not prepare seed-mismatch Autosave 3 header: {error}"))?;
    fs::write(
        root.join(SaveSlotId::LegacyDefault.canonical_file_name()),
        "legacy candidate fixture\n",
    )
    .map_err(|error| format!("could not prepare legacy fixture: {error}"))?;
    Ok(())
}

fn replace_save_with_invalid_body(path: &Path) -> Result<(), String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    let header = match decode_save_file(&contents)
        .map_err(|error| format!("could not decode {}: {error}", path.display()))?
        .format
    {
        SaveFormat::V1(header) => header,
        SaveFormat::LegacyV0 => {
            return Err(format!("{} was unexpectedly legacy", path.display()));
        }
    };
    fs::write(
        path,
        format!(
            "{}this is intentionally not a DynamicWorld body\n",
            encode_save_header_text(header)
        ),
    )
    .map_err(|error| format!("could not overwrite {}: {error}", path.display()))
}

fn matches_outcome(
    outcome: &SaveLoadOutcome,
    operation: SaveLoadOperation,
    result: SaveLoadResult,
) -> bool {
    outcome.operation == operation && outcome.result == result
}

fn fail_unexpected_outcome(
    driver: &mut NativeSaveLoadAcceptance,
    outcome: &SaveLoadOutcome,
    exit: &mut MessageWriter<AppExit>,
) {
    fail_driver(
        driver,
        &format!("unexpected terminal outcome: {outcome:?}"),
        exit,
    );
}

fn fail_driver(
    driver: &mut NativeSaveLoadAcceptance,
    reason: &str,
    exit: &mut MessageWriter<AppExit>,
) {
    if driver.stage == AcceptanceStage::Finished {
        return;
    }
    // Persist the owner stage with a failure. The actual-window process is
    // intentionally opaque to the orchestrator while it runs, so this is the
    // bounded evidence needed to distinguish an input-capture stall from a
    // transaction or renderer failure after it exits.
    let reason = format!("{reason} (stage {:?})", driver.stage);
    let escaped = json_escape(&reason);
    let run_id = json_escape(&driver.run_id);
    let body = format!(
        "{{\n  \"status\": \"FAIL\",\n  \"run_id\": \"{run_id}\",\n  \"reason\": \"{escaped}\"\n}}\n"
    );
    if let Err(error) = write_new_atomic(&driver.result_path(), body.as_bytes()) {
        error!("NATIVE_SAVE_LOAD_ACCEPTANCE: {reason}; result write also failed: {error}");
    } else {
        error!("NATIVE_SAVE_LOAD_ACCEPTANCE: {reason}");
    }
    driver.stage = AcceptanceStage::Finished;
    exit.write(AppExit::error());
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
        output.sync_all()?;
        fs::hard_link(&temporary_path, path)?;
        Ok(())
    })();
    let _ = fs::remove_file(temporary_path);
    write_result
}

fn write_capture_ready(path: &Path, run_id: &str) -> io::Result<()> {
    write_new_atomic(
        path,
        format!(
            "run_id={run_id}\nmarker_rgb=255,0,255\nmarker_min_pixels={CAPTURE_MARKER_MIN_PIXELS}\n"
        )
        .as_bytes(),
    )
}

#[derive(Debug)]
struct CaptureEvidence {
    screenshot_bytes: u64,
    screenshot_width: u32,
    screenshot_height: u32,
    screenshot_sha256: String,
    marker_pixels: u32,
    capture_scope: String,
    capture_window_id: String,
    capture_window_pid: u32,
}

struct SaveCatalogAcceptanceResult<'a> {
    run_id: &'a str,
    save_bytes: usize,
    world_epoch_before: u64,
    world_epoch_after: u64,
    checks: [bool; 5],
    runtime_root: &'a Path,
    renderer: &'a RenderEnvironment,
    evidence: &'a CaptureEvidence,
}

fn read_capture_evidence(
    driver: &NativeSaveLoadAcceptance,
) -> Result<Option<CaptureEvidence>, String> {
    let ack_path = driver.capture_ack_path();
    let metadata = match fs::metadata(&ack_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "could not inspect capture acknowledgement: {error}"
            ));
        }
    };
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > 512 {
        return Err("capture acknowledgement was not a bounded regular file".to_string());
    }
    let body = fs::read_to_string(&ack_path)
        .map_err(|error| format!("could not read capture acknowledgement: {error}"))?;
    let lines = body.lines().collect::<Vec<_>>();
    if lines.len() != 10 {
        return Err("capture acknowledgement had an unexpected schema".to_string());
    }
    let acknowledged_run = lines[0]
        .strip_prefix("run_id=")
        .ok_or_else(|| "capture acknowledgement omitted run_id".to_string())?;
    if acknowledged_run != driver.run_id {
        return Err("capture acknowledgement belonged to another run".to_string());
    }
    let screenshot = lines[1]
        .strip_prefix("screenshot=")
        .ok_or_else(|| "capture acknowledgement omitted screenshot".to_string())?;
    if screenshot != SCREENSHOT_FILE {
        return Err("capture acknowledgement named an unexpected screenshot".to_string());
    }
    let screenshot_bytes = lines[2]
        .strip_prefix("bytes=")
        .ok_or_else(|| "capture acknowledgement omitted screenshot size".to_string())?
        .parse::<u64>()
        .map_err(|_| "capture acknowledgement had an invalid screenshot size".to_string())?;
    let screenshot_width = lines[3]
        .strip_prefix("width=")
        .ok_or_else(|| "capture acknowledgement omitted screenshot width".to_string())?
        .parse::<u32>()
        .map_err(|_| "capture acknowledgement had an invalid screenshot width".to_string())?;
    let screenshot_height = lines[4]
        .strip_prefix("height=")
        .ok_or_else(|| "capture acknowledgement omitted screenshot height".to_string())?
        .parse::<u32>()
        .map_err(|_| "capture acknowledgement had an invalid screenshot height".to_string())?;
    let screenshot_sha256 = lines[5]
        .strip_prefix("sha256=")
        .ok_or_else(|| "capture acknowledgement omitted screenshot hash".to_string())?;
    if screenshot_sha256.len() != 64
        || !screenshot_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("capture acknowledgement had an invalid SHA-256 digest".to_string());
    }
    let marker_pixels = lines[6]
        .strip_prefix("marker_pixels=")
        .ok_or_else(|| "capture acknowledgement omitted marker pixel count".to_string())?
        .parse::<u32>()
        .map_err(|_| "capture acknowledgement had an invalid marker pixel count".to_string())?;
    if marker_pixels < CAPTURE_MARKER_MIN_PIXELS {
        return Err(
            "renderer screenshot did not contain the final Save catalog marker".to_string(),
        );
    }
    let capture_scope = lines[7]
        .strip_prefix("capture_scope=")
        .ok_or_else(|| "capture acknowledgement omitted capture scope".to_string())?;
    if capture_scope != CAPTURE_SCOPE {
        return Err("capture acknowledgement was not bound to an X11 client window".to_string());
    }
    let capture_window_id = lines[8]
        .strip_prefix("capture_window_id=")
        .ok_or_else(|| "capture acknowledgement omitted X11 window ID".to_string())?;
    if capture_window_id.len() <= 2
        || !capture_window_id.starts_with("0x")
        || !capture_window_id[2..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || capture_window_id != capture_window_id.to_ascii_lowercase()
    {
        return Err("capture acknowledgement had an invalid X11 window ID".to_string());
    }
    let capture_window_pid = lines[9]
        .strip_prefix("capture_window_pid=")
        .ok_or_else(|| "capture acknowledgement omitted X11 window PID".to_string())?
        .parse::<u32>()
        .map_err(|_| "capture acknowledgement had an invalid X11 window PID".to_string())?;
    if capture_window_pid == 0 {
        return Err("capture acknowledgement had an invalid X11 window PID".to_string());
    }
    let screenshot_metadata = fs::metadata(driver.artifact_dir.join(SCREENSHOT_FILE))
        .map_err(|error| format!("could not inspect renderer screenshot: {error}"))?;
    if !screenshot_metadata.is_file()
        || screenshot_bytes == 0
        || screenshot_bytes > MAX_SCREENSHOT_BYTES
        || screenshot_metadata.len() != screenshot_bytes
    {
        return Err("renderer screenshot size did not match its acknowledgement".to_string());
    }
    let mut screenshot = fs::File::open(driver.artifact_dir.join(SCREENSHOT_FILE))
        .map_err(|error| format!("could not open renderer screenshot: {error}"))?;
    let mut header = [0_u8; 24];
    screenshot
        .read_exact(&mut header)
        .map_err(|error| format!("could not read renderer screenshot header: {error}"))?;
    if header[..8] != [137, 80, 78, 71, 13, 10, 26, 10] || &header[12..16] != b"IHDR" {
        return Err("renderer screenshot was not a PNG image".to_string());
    }
    let png_width = u32::from_be_bytes(header[16..20].try_into().unwrap());
    let png_height = u32::from_be_bytes(header[20..24].try_into().unwrap());
    if screenshot_width < MIN_SCREENSHOT_WIDTH
        || screenshot_height < MIN_SCREENSHOT_HEIGHT
        || png_width != screenshot_width
        || png_height != screenshot_height
    {
        return Err("renderer screenshot dimensions did not match its acknowledgement".to_string());
    }
    Ok(Some(CaptureEvidence {
        screenshot_bytes,
        screenshot_width,
        screenshot_height,
        screenshot_sha256: screenshot_sha256.to_ascii_lowercase(),
        marker_pixels,
        capture_scope: capture_scope.to_owned(),
        capture_window_id: capture_window_id.to_owned(),
        capture_window_pid,
    }))
}

fn write_success_result(path: &Path, result: &SaveCatalogAcceptanceResult<'_>) -> io::Result<()> {
    let checks = result
        .checks
        .into_iter()
        .enumerate()
        .map(|(index, passed)| {
            format!(
                "    \"V{}\": \"{}\"",
                index + 1,
                if passed { "PASS" } else { "FAIL" }
            )
        })
        .collect::<Vec<_>>()
        .join(",\n");
    let body = format!(
        concat!(
            "{{\n",
            "  \"status\": \"PASS\",\n",
            "  \"profile\": \"save-catalog\",\n",
            "  \"run_id\": \"{}\",\n",
            "  \"save_bytes\": {},\n",
            "  \"world_epoch_before_valid_load\": {},\n",
            "  \"world_epoch_after_valid_load\": {},\n",
            "  \"checks\": {{\n{}\n  }},\n",
            "  \"runtime\": {{\n",
            "    \"save_root\": \"{}\",\n",
            "    \"settings_root\": \"{}\"\n",
            "  }},\n",
            "  \"renderer\": {{\n",
            "    \"adapter_name\": \"{}\",\n",
            "    \"backend\": \"{}\",\n",
            "    \"display_handle\": \"{}\"\n",
            "  }},\n",
            "  \"screenshot\": \"{}\",\n",
            "  \"screenshot_bytes\": {},\n",
            "  \"screenshot_width\": {},\n",
            "  \"screenshot_height\": {},\n",
            "  \"screenshot_sha256\": \"{}\",\n",
            "  \"screenshot_marker_pixels\": {},\n",
            "  \"screenshot_capture\": {{\n",
            "    \"scope\": \"{}\",\n",
            "    \"window_id\": \"{}\",\n",
            "    \"window_pid\": {}\n",
            "  }}\n",
            "}}\n"
        ),
        json_escape(result.run_id),
        result.save_bytes,
        result.world_epoch_before,
        result.world_epoch_after,
        checks,
        json_escape(&result.runtime_root.join("saves").display().to_string()),
        json_escape(&result.runtime_root.join("settings").display().to_string()),
        json_escape(&result.renderer.adapter_name),
        json_escape(&result.renderer.adapter_backend),
        json_escape(&result.renderer.display_handle),
        SCREENSHOT_FILE,
        result.evidence.screenshot_bytes,
        result.evidence.screenshot_width,
        result.evidence.screenshot_height,
        result.evidence.screenshot_sha256,
        result.evidence.marker_pixels,
        json_escape(&result.evidence.capture_scope),
        json_escape(&result.evidence.capture_window_id),
        result.evidence.capture_window_pid,
    );
    write_new_atomic(path, body.as_bytes())
}

fn json_escape(value: &str) -> String {
    value
        .chars()
        .flat_map(|character| character.escape_default())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_writer_emits_bounded_machine_readable_evidence() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "hell-workers-native-save-load-result-{}-{nonce}.json",
            std::process::id(),
        ));
        let evidence = CaptureEvidence {
            screenshot_bytes: 1_024,
            screenshot_width: 1280,
            screenshot_height: 720,
            screenshot_sha256: "a".repeat(64),
            marker_pixels: CAPTURE_MARKER_MIN_PIXELS,
            capture_scope: CAPTURE_SCOPE.to_owned(),
            capture_window_id: "0x1234".to_owned(),
            capture_window_pid: 1_234,
        };
        let renderer = RenderEnvironment {
            adapter_name: "Intel(R) Arc Graphics".to_owned(),
            adapter_backend: "Vulkan".to_owned(),
            display_handle: "Xlib(XlibDisplayHandle)".to_owned(),
        };
        let runtime_root = std::env::temp_dir();
        let acceptance = SaveCatalogAcceptanceResult {
            run_id: "test-run",
            save_bytes: 42,
            world_epoch_before: 3,
            world_epoch_after: 4,
            checks: [true; 5],
            runtime_root: &runtime_root,
            renderer: &renderer,
            evidence: &evidence,
        };
        write_success_result(&path, &acceptance).unwrap();
        let result = fs::read_to_string(&path).unwrap();
        fs::remove_file(path).unwrap();

        assert!(result.contains("\"status\": \"PASS\""));
        assert!(result.contains("\"run_id\": \"test-run\""));
        assert!(result.contains("\"save_bytes\": 42"));
        assert!(result.contains("\"world_epoch_before_valid_load\": 3"));
        assert!(result.contains("\"world_epoch_after_valid_load\": 4"));
        assert!(result.contains("\"profile\": \"save-catalog\""));
        assert!(result.contains("\"V5\": \"PASS\""));
        assert!(result.contains("\"adapter_name\": \"Intel(R) Arc Graphics\""));
        assert!(result.contains(&format!("\"screenshot_sha256\": \"{}\"", "a".repeat(64))));
        assert!(result.contains(&format!(
            "\"screenshot_marker_pixels\": {CAPTURE_MARKER_MIN_PIXELS}"
        )));
        assert!(result.contains("\"scope\": \"x11-client-window\""));
        assert!(result.contains("\"window_id\": \"0x1234\""));
        assert!(result.contains("\"window_pid\": 1234"));
        assert!(!result.contains(",\n}\n"));
        assert!(result.len() < 2_048);
    }

    #[test]
    fn run_id_rejects_paths_and_multiline_values() {
        assert!(validate_run_id("c3-native_20260804").is_ok());
        assert!(validate_run_id("").is_err());
        assert!(validate_run_id("../other-run").is_err());
        assert!(validate_run_id("first\nsecond").is_err());
    }

    #[test]
    fn final_capture_requires_the_visible_save_catalog_through_acknowledgement() {
        assert!(final_catalog_capture_is_held(
            true,
            SaveCatalogMode::SaveCatalog,
            true,
        ));
        assert!(!final_catalog_capture_is_held(
            false,
            SaveCatalogMode::SaveCatalog,
            true,
        ));
        assert!(!final_catalog_capture_is_held(
            true,
            SaveCatalogMode::Closed,
            true,
        ));
        assert!(!final_catalog_capture_is_held(
            true,
            SaveCatalogMode::SaveCatalog,
            false,
        ));
    }
}
