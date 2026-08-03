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
use std::time::{Duration, Instant};

use bevy::ecs::reflect::AppTypeRegistry;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::reflect::PartialReflect;
use bevy::window::PrimaryWindow;
use bevy_world_serialization::DynamicWorld;
use bevy_world_serialization::serde::WorldDeserializer;
use hw_core::WorldEpoch;
use hw_core::familiar::Familiar;
use hw_core::soul::DamnedSoul;
use hw_world::WorldMap;

use super::format::{SaveFormat, decode_save_file};
use super::{
    SaveLoadFailureKind, SaveLoadOperation, SaveLoadOutcome, SaveLoadResult, SaveLoadState,
    SavePath,
};

const ARTIFACT_ENV: &str = "HW_NATIVE_SAVE_LOAD_ACCEPTANCE_ARTIFACT";
const RUN_ID_ENV: &str = "HW_NATIVE_SAVE_LOAD_ACCEPTANCE_RUN_ID";
const RESULT_FILE: &str = "driver-result.json";
const CAPTURE_READY_FILE: &str = "capture-ready.txt";
const CAPTURE_ACK_FILE: &str = "capture.done.txt";
const SCREENSHOT_FILE: &str = "paused-after-acceptance.png";
const MAX_SCREENSHOT_BYTES: u64 = 16 * 1024 * 1024;
const MIN_SCREENSHOT_WIDTH: u32 = 640;
const MIN_SCREENSHOT_HEIGHT: u32 = 360;
const READY_FRAMES: u32 = 30;
const DRIVER_TIMEOUT: Duration = Duration::from_secs(180);

/// Adds a bounded save/load acceptance sequence to the regular actual-window
/// application. This is never enabled by default.
pub struct NativeSaveLoadAcceptancePlugin {
    artifact_dir: PathBuf,
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
            artifact_dir.join(CAPTURE_READY_FILE),
            artifact_dir.join(CAPTURE_ACK_FILE),
            artifact_dir.join(SCREENSHOT_FILE),
            artifact_dir.join("runtime/saves/world.scn.ron"),
        ] {
            if owned_path.exists() {
                return Err(format!(
                    "native acceptance artifact contains stale driver output: {}",
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

impl Plugin for NativeSaveLoadAcceptancePlugin {
    fn build(&self, app: &mut App) {
        let save_path = self.artifact_dir.join("runtime/saves/world.scn.ron");
        app.insert_resource(SavePath::new(save_path.clone()));
        app.insert_resource(NativeSaveLoadAcceptance::new(
            self.artifact_dir.clone(),
            save_path,
            self.run_id.clone(),
        ));
        app.add_systems(Update, drive_native_save_load_acceptance);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AcceptanceStage {
    WaitForWorld,
    AwaitInitialSave,
    AwaitInitialLoad,
    WaitForLoadedWorld,
    AwaitBaselineSave,
    AwaitBaselineConfirmation,
    AwaitInvalidLoad,
    AwaitPostInvalidSave,
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
    save_path: PathBuf,
    run_id: String,
    stage: AcceptanceStage,
    started_at: Instant,
    ready_frames: u32,
    baseline_snapshot: Option<PersistentSnapshot>,
    baseline_save_bytes: usize,
    epoch_before_valid_load: u64,
    baseline_epoch: u64,
}

impl NativeSaveLoadAcceptance {
    fn new(artifact_dir: PathBuf, save_path: PathBuf, run_id: String) -> Self {
        Self {
            artifact_dir,
            save_path,
            run_id,
            stage: AcceptanceStage::WaitForWorld,
            started_at: Instant::now(),
            ready_frames: 0,
            baseline_snapshot: None,
            baseline_save_bytes: 0,
            epoch_before_valid_load: 0,
            baseline_epoch: 0,
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
    save_load_state: ResMut<'w, SaveLoadState>,
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
        mut save_load_state,
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
            if !request_operation(&mut save_load_state, SaveLoadState::SaveRequested) {
                fail_driver(
                    &mut driver,
                    "save/load dispatcher was busy before the initial save",
                    &mut exit,
                );
                return;
            }
            driver.stage = AcceptanceStage::AwaitInitialSave;
            info!("NATIVE_SAVE_LOAD_ACCEPTANCE: initial save requested");
        }
        AcceptanceStage::AwaitInitialSave => {
            let Some(outcome) = terminal else {
                return;
            };
            if !matches_outcome(outcome, SaveLoadOperation::Save, SaveLoadResult::Succeeded) {
                fail_unexpected_outcome(&mut driver, outcome, &mut exit);
                return;
            }
            if !request_operation(&mut save_load_state, SaveLoadState::LoadRequested) {
                fail_driver(
                    &mut driver,
                    "save/load dispatcher stayed busy after the initial save",
                    &mut exit,
                );
                return;
            }
            driver.epoch_before_valid_load = world_epoch.get();
            driver.stage = AcceptanceStage::AwaitInitialLoad;
            info!("NATIVE_SAVE_LOAD_ACCEPTANCE: initial load requested");
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
            if !request_operation(&mut save_load_state, SaveLoadState::SaveRequested) {
                fail_driver(
                    &mut driver,
                    "save/load dispatcher stayed busy after the loaded world converged",
                    &mut exit,
                );
                return;
            }
            driver.stage = AcceptanceStage::AwaitBaselineSave;
            info!("NATIVE_SAVE_LOAD_ACCEPTANCE: first paused baseline save requested");
        }
        AcceptanceStage::AwaitBaselineSave => {
            let Some(outcome) = terminal else {
                return;
            };
            if !matches_outcome(outcome, SaveLoadOperation::Save, SaveLoadResult::Succeeded) {
                fail_unexpected_outcome(&mut driver, outcome, &mut exit);
                return;
            }
            let (baseline, baseline_bytes) =
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
            driver.baseline_save_bytes = baseline_bytes;
            if !request_operation(&mut save_load_state, SaveLoadState::SaveRequested) {
                fail_driver(
                    &mut driver,
                    "save/load dispatcher stayed busy before baseline confirmation",
                    &mut exit,
                );
                return;
            }
            driver.stage = AcceptanceStage::AwaitBaselineConfirmation;
            info!("NATIVE_SAVE_LOAD_ACCEPTANCE: confirming stable paused baseline");
        }
        AcceptanceStage::AwaitBaselineConfirmation => {
            let Some(outcome) = terminal else {
                return;
            };
            if !matches_outcome(outcome, SaveLoadOperation::Save, SaveLoadResult::Succeeded) {
                fail_unexpected_outcome(&mut driver, outcome, &mut exit);
                return;
            }
            let (confirmed_baseline, confirmed_baseline_bytes) =
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
            driver.baseline_save_bytes = confirmed_baseline_bytes;
            if let Err(error) = fs::write(&driver.save_path, b"not a valid Hell Workers save\n") {
                fail_driver(
                    &mut driver,
                    &format!("could not corrupt acceptance save: {error}"),
                    &mut exit,
                );
                return;
            }
            if !request_operation(&mut save_load_state, SaveLoadState::LoadRequested) {
                fail_driver(
                    &mut driver,
                    "save/load dispatcher stayed busy before the invalid load",
                    &mut exit,
                );
                return;
            }
            driver.stage = AcceptanceStage::AwaitInvalidLoad;
            info!("NATIVE_SAVE_LOAD_ACCEPTANCE: invalid load requested");
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
            if !request_operation(&mut save_load_state, SaveLoadState::SaveRequested) {
                fail_driver(
                    &mut driver,
                    "save/load dispatcher stayed busy after the invalid load",
                    &mut exit,
                );
                return;
            }
            driver.stage = AcceptanceStage::AwaitPostInvalidSave;
            info!("NATIVE_SAVE_LOAD_ACCEPTANCE: post-rejection save requested");
        }
        AcceptanceStage::AwaitPostInvalidSave => {
            let Some(outcome) = terminal else {
                return;
            };
            if !matches_outcome(outcome, SaveLoadOperation::Save, SaveLoadResult::Succeeded) {
                fail_unexpected_outcome(&mut driver, outcome, &mut exit);
                return;
            }
            let (post_rejection, _) =
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
            if let Err(error) = write_capture_ready(&driver.capture_ready_path(), &driver.run_id) {
                fail_driver(
                    &mut driver,
                    &format!("could not publish the capture-ready marker: {error}"),
                    &mut exit,
                );
                return;
            }
            driver.stage = AcceptanceStage::AwaitCapture;
            info!("NATIVE_SAVE_LOAD_ACCEPTANCE: driver checks passed; awaiting screenshot");
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
            if !virtual_time.is_paused() {
                fail_driver(
                    &mut driver,
                    "virtual time resumed while awaiting renderer evidence",
                    &mut exit,
                );
                return;
            }
            let evidence = match read_capture_evidence(&driver) {
                Ok(Some(evidence)) => evidence,
                Ok(None) => return,
                Err(error) => {
                    fail_driver(&mut driver, &error, &mut exit);
                    return;
                }
            };
            if let Err(error) = write_success_result(
                &driver.result_path(),
                &driver.run_id,
                driver.baseline_save_bytes,
                driver.epoch_before_valid_load,
                driver.baseline_epoch,
                &evidence,
            ) {
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
) -> Result<(PersistentSnapshot, usize), String> {
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
    Ok((
        PersistentSnapshot {
            format: decoded.format,
            world,
        },
        contents.len(),
    ))
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
        !map.tile_entities.is_empty() && map.tile_entities.iter().all(Option::is_some)
    }) && !souls.is_empty()
        && !familiars.is_empty()
        && primary_windows.iter().count() == 1
}

fn request_operation(state: &mut SaveLoadState, request: SaveLoadState) -> bool {
    if *state != SaveLoadState::Idle {
        return false;
    }
    *state = request;
    true
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
    let escaped = json_escape(reason);
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
    write_new_atomic(path, format!("run_id={run_id}\n").as_bytes())
}

#[derive(Debug)]
struct CaptureEvidence {
    screenshot_bytes: u64,
    screenshot_width: u32,
    screenshot_height: u32,
    screenshot_sha256: String,
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
    if lines.len() != 6 {
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
    }))
}

fn write_success_result(
    path: &Path,
    run_id: &str,
    save_bytes: usize,
    world_epoch_before: u64,
    world_epoch_after: u64,
    evidence: &CaptureEvidence,
) -> io::Result<()> {
    let body = format!(
        concat!(
            "{{\n",
            "  \"status\": \"PASS\",\n",
            "  \"run_id\": \"{}\",\n",
            "  \"save_bytes\": {},\n",
            "  \"world_epoch_before_valid_load\": {},\n",
            "  \"world_epoch_after_valid_load\": {},\n",
            "  \"screenshot\": \"{}\",\n",
            "  \"screenshot_bytes\": {},\n",
            "  \"screenshot_width\": {},\n",
            "  \"screenshot_height\": {},\n",
            "  \"screenshot_sha256\": \"{}\",\n",
            "  \"checks\": [\n",
            "    \"actual-window application reached a complete persistent world\",\n",
            "    \"save and load succeeded through the production Last dispatcher\",\n",
            "    \"successful load advanced WorldEpoch exactly once\",\n",
            "    \"virtual time remained paused after world replacement\",\n",
            "    \"two consecutive post-convergence saves were semantically equal\",\n",
            "    \"invalid save was rejected without advancing WorldEpoch\",\n",
            "    \"post-rejection save was semantically equal to the paused baseline\",\n",
            "    \"fresh renderer screenshot was acknowledged for this run\"\n",
            "  ]\n",
            "}}\n"
        ),
        json_escape(run_id),
        save_bytes,
        world_epoch_before,
        world_epoch_after,
        SCREENSHOT_FILE,
        evidence.screenshot_bytes,
        evidence.screenshot_width,
        evidence.screenshot_height,
        evidence.screenshot_sha256,
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
        };
        write_success_result(&path, "test-run", 42, 3, 4, &evidence).unwrap();
        let result = fs::read_to_string(&path).unwrap();
        fs::remove_file(path).unwrap();

        assert!(result.contains("\"status\": \"PASS\""));
        assert!(result.contains("\"run_id\": \"test-run\""));
        assert!(result.contains("\"save_bytes\": 42"));
        assert!(result.contains("\"world_epoch_before_valid_load\": 3"));
        assert!(result.contains("\"world_epoch_after_valid_load\": 4"));
        assert!(result.contains(&format!("\"screenshot_sha256\": \"{}\"", "a".repeat(64))));
        assert!(result.len() < 2_048);
    }

    #[test]
    fn run_id_rejects_paths_and_multiline_values() {
        assert!(validate_run_id("c3-native_20260804").is_ok());
        assert!(validate_run_id("").is_err());
        assert!(validate_run_id("../other-run").is_err());
        assert!(validate_run_id("first\nsecond").is_err());
    }
}
