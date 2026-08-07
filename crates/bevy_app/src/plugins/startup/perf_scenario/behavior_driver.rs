use std::collections::BTreeSet;
use std::fs::OpenOptions;
use std::io::Write;

use bevy::app::AppExit;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use hw_core::WorldEpoch;
use hw_jobs::{Building, BuildingType};
use hw_ui::UiIntent;
use hw_world::{Room, Yard};

use crate::systems::save::{
    SaveLoadOperation, SaveLoadOutcome, SaveLoadResult, SaveLoadState, SavePath,
};

use super::indoor_light_fixture::IndoorLightFixturePhase;
use super::output::{
    perf_output_directory, write_indoor_light_fixture_sidecars, write_window_observation,
};
use super::*;

const BEHAVIOR_TIMEOUT_UPDATES: u64 = 512;
const SMALL_DOOR_GRID: (i32, i32) = (19, 27);
// The normal startup world contains one Yard; the indoor-light fixture adds a
// main and a control Yard around it.
const SMALL_GLOBAL_YARD_COUNT: usize = 3;

#[derive(Resource, Default)]
pub(crate) struct PerfBehaviorCapture {
    phase: BehaviorPhase,
    update_count: u64,
    simulation_tick: u64,
    step_issued: bool,
    rows: Vec<TimelineRow>,
    subject_soul: Option<Entity>,
    subject_door: Option<Entity>,
    initial_epoch: u64,
    initial_paused: bool,
    save_outcomes: u32,
    load_outcomes: u32,
    load_wait_updates: u32,
    fixture_checksum: Option<&'static str>,
    initial_window: Option<PerfWindowObservation>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum BehaviorPhase {
    #[default]
    WaitingForFixture,
    DoorStep(u32),
    LoadStep(u32),
    Flush,
    Finished,
}

struct TimelineRow {
    case_id: &'static str,
    step_index: u32,
    script_update: u64,
    simulation_tick: u64,
    pause_state: &'static str,
    world_epoch: u64,
    intent: &'static str,
    attempted: bool,
    applied: bool,
    semantic_state: Option<&'static str>,
    active_presentation_state: Option<&'static str>,
    fixture_checksum: &'static str,
    terminal_outcome: &'static str,
}

impl TimelineRow {
    fn json(&self) -> String {
        let optional = |value: Option<&str>| {
            value
                .map(|value| format!("\"{}\"", json_escape(value)))
                .unwrap_or_else(|| "null".to_string())
        };
        format!(
            concat!(
                "{{\"case_id\":\"{}\",\"step_index\":{},\"script_update\":{},",
                "\"simulation_tick\":{},\"pause_state\":\"{}\",\"world_epoch\":{},",
                "\"intent\":\"{}\",\"attempted\":{},\"applied\":{},",
                "\"semantic_state\":{},\"active_presentation_state\":{},",
                "\"registry_phase\":\"stage_before_registry_owner\",",
                "\"registry_step_id\":null,\"wake_count\":null,",
                "\"field_availability\":\"stage_before_field_owner\",",
                "\"field_input_revision\":null,\"field_output_revision\":null,",
                "\"field_read_count\":null,\"old_epoch_field_read_count\":null,",
                "\"field_is_dark\":null,\"field_checksum\":null,",
                "\"gpu_availability\":\"stage_before_gpu_owner\",",
                "\"gpu_upload_epoch\":null,\"gpu_checksum\":null,",
                "\"fixture_checksum\":\"{}\",\"terminal_outcome\":\"{}\"}}"
            ),
            json_escape(self.case_id),
            self.step_index,
            self.script_update,
            self.simulation_tick,
            self.pause_state,
            self.world_epoch,
            json_escape(self.intent),
            self.attempted,
            self.applied,
            optional(self.semantic_state),
            optional(self.active_presentation_state),
            self.fixture_checksum,
            self.terminal_outcome,
        )
    }
}

#[derive(SystemParam)]
pub(crate) struct BehaviorDriveParams<'w, 's> {
    config: Res<'w, PerfScenarioConfig>,
    applied: Res<'w, PerfScenarioApplied>,
    fixture: Res<'w, IndoorLightFixtureState>,
    capture: ResMut<'w, PerfBehaviorCapture>,
    virtual_time: ResMut<'w, Time<Virtual>>,
    world_epoch: Res<'w, WorldEpoch>,
    world_map: Res<'w, WorldMap>,
    save_path: ResMut<'w, SavePath>,
    save_state: ResMut<'w, SaveLoadState>,
    ui_intents: MessageWriter<'w, UiIntent>,
    souls: Query<
        'w,
        's,
        (
            &'static mut Transform,
            &'static mut Destination,
            &'static mut Path,
        ),
        With<DamnedSoul>,
    >,
    primary_window: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    rtt_runtime: Res<'w, RttRuntime>,
    quality: Res<'w, QualitySettings>,
    exit: MessageWriter<'w, AppExit>,
}

pub(crate) fn drive_perf_behavior_system(mut params: BehaviorDriveParams) {
    if params.capture.phase == BehaviorPhase::Finished {
        return;
    }
    if params.capture.update_count >= BEHAVIOR_TIMEOUT_UPDATES {
        fail_behavior(
            &mut params.capture,
            "behavior script exceeded its update budget",
            &mut params.exit,
        );
        return;
    }
    params.capture.update_count += 1;

    if params.capture.phase == BehaviorPhase::WaitingForFixture {
        if !params.applied.complete() || params.fixture.phase != IndoorLightFixturePhase::Ready {
            return;
        }
        let Some((subject_soul, subject_door, door_grid)) = params.fixture.behavior_subjects()
        else {
            fail_behavior(
                &mut params.capture,
                "ready fixture has no behavior subjects",
                &mut params.exit,
            );
            return;
        };
        if door_grid != SMALL_DOOR_GRID {
            fail_behavior(
                &mut params.capture,
                "behavior Door is not the canonical small-fixture Door",
                &mut params.exit,
            );
            return;
        }
        let Some(fixture_checksum) = params.fixture.behavior_fixture_checksum() else {
            fail_behavior(
                &mut params.capture,
                "ready fixture has no semantic checksum",
                &mut params.exit,
            );
            return;
        };
        params.virtual_time.unpause();
        params.capture.initial_paused = params.virtual_time.is_paused();
        params.capture.initial_epoch = params.world_epoch.get();
        params.capture.subject_soul = Some(subject_soul);
        params.capture.subject_door = Some(subject_door);
        params.capture.fixture_checksum = Some(fixture_checksum);
        params.capture.initial_window = Some(PerfWindowObservation::capture(
            params.primary_window.single().ok(),
            &params.rtt_runtime,
            &params.quality,
            None,
        ));
        let private_save = perf_output_directory(&params.config).join("behavior-save.scn.ron");
        if private_save.exists() {
            fail_behavior(
                &mut params.capture,
                "job-owned behavior save path already exists",
                &mut params.exit,
            );
            return;
        }
        *params.save_path = SavePath::new(private_save);
        params.capture.phase = match params.config.behavior_case() {
            Some(PerfBehaviorCase::DoorStateV1) => BehaviorPhase::DoorStep(0),
            Some(PerfBehaviorCase::LoadNormalV1) => BehaviorPhase::LoadStep(0),
            None => {
                fail_behavior(
                    &mut params.capture,
                    "behavior lane has no selected case",
                    &mut params.exit,
                );
                return;
            }
        };
        params.capture.step_issued = false;
        info!(
            "PERF_BEHAVIOR: case={} fixture={} started",
            params
                .config
                .behavior_case()
                .map_or("<missing>", PerfBehaviorCase::as_str),
            fixture_checksum,
        );
    }

    if params.capture.step_issued {
        return;
    }
    match params.capture.phase {
        BehaviorPhase::DoorStep(step) => {
            let stimulus = match step {
                0 => Ok(()),
                1 => {
                    let Some(soul) = params.capture.subject_soul else {
                        return fail_behavior(
                            &mut params.capture,
                            "Door behavior lost its Soul subject",
                            &mut params.exit,
                        );
                    };
                    let Ok((mut transform, mut destination, mut path)) = params.souls.get_mut(soul)
                    else {
                        return fail_behavior(
                            &mut params.capture,
                            "Door behavior Soul subject vanished",
                            &mut params.exit,
                        );
                    };
                    let approach_position =
                        WorldMap::grid_to_world(SMALL_DOOR_GRID.0, SMALL_DOOR_GRID.1 - 1);
                    let door_position =
                        WorldMap::grid_to_world(SMALL_DOOR_GRID.0, SMALL_DOOR_GRID.1);
                    transform.translation.x = approach_position.x;
                    transform.translation.y = approach_position.y;
                    destination.0 = door_position;
                    path.waypoints.clear();
                    path.waypoints.push(door_position);
                    path.current_index = 0;
                    path.planned_destination = Some(door_position);
                    path.validated_obstacle_version = params.world_map.obstacle_version;
                    Ok(())
                }
                2 | 4 => {
                    params.ui_intents.write(UiIntent::TogglePause);
                    Ok(())
                }
                3 => {
                    let Some(door) = params.capture.subject_door else {
                        return fail_behavior(
                            &mut params.capture,
                            "Door behavior lost its Door subject",
                            &mut params.exit,
                        );
                    };
                    params.ui_intents.write(UiIntent::ToggleDoorLock(door));
                    Ok(())
                }
                _ => Err("Door behavior requested an unknown step"),
            };
            if let Err(reason) = stimulus {
                fail_behavior(&mut params.capture, reason, &mut params.exit);
                return;
            }
            params.capture.step_issued = true;
        }
        BehaviorPhase::LoadStep(step) => {
            match step {
                0 | 2 | 4 | 5 => {}
                1 => {
                    if *params.save_state != SaveLoadState::Idle {
                        fail_behavior(
                            &mut params.capture,
                            "save/load dispatcher was busy before behavior save",
                            &mut params.exit,
                        );
                        return;
                    }
                    *params.save_state = SaveLoadState::SaveRequested;
                }
                3 => {
                    if *params.save_state != SaveLoadState::Idle {
                        fail_behavior(
                            &mut params.capture,
                            "save/load dispatcher was busy before behavior load",
                            &mut params.exit,
                        );
                        return;
                    }
                    *params.save_state = SaveLoadState::LoadRequested;
                }
                _ => {
                    fail_behavior(
                        &mut params.capture,
                        "normal-load behavior requested an unknown step",
                        &mut params.exit,
                    );
                    return;
                }
            }
            params.capture.step_issued = true;
        }
        _ => {}
    }
}

pub(crate) fn count_perf_behavior_fixed_tick_system(mut capture: ResMut<PerfBehaviorCapture>) {
    if !matches!(
        capture.phase,
        BehaviorPhase::WaitingForFixture | BehaviorPhase::Finished
    ) {
        capture.simulation_tick = capture.simulation_tick.saturating_add(1);
    }
}

#[derive(SystemParam)]
pub(crate) struct BehaviorObserveParams<'w, 's> {
    config: Res<'w, PerfScenarioConfig>,
    fixture: Res<'w, IndoorLightFixtureState>,
    capture: ResMut<'w, PerfBehaviorCapture>,
    virtual_time: Res<'w, Time<Virtual>>,
    world_epoch: Res<'w, WorldEpoch>,
    outcomes: MessageReader<'w, 's, SaveLoadOutcome>,
    doors: Query<
        'w,
        's,
        (
            &'static Door,
            &'static Building,
            &'static Transform,
            &'static Children,
        ),
    >,
    sprites: Query<'w, 's, &'static Sprite>,
    building_3d_visuals: Query<'w, 's, &'static Building3dVisual>,
    door_handles: Res<'w, DoorVisualHandles>,
    buildings: Query<'w, 's, (Entity, &'static Building, &'static Transform)>,
    souls: Query<'w, 's, (), With<DamnedSoul>>,
    familiars: Query<'w, 's, (), With<Familiar>>,
    yards: Query<'w, 's, &'static Yard>,
    rooms: Query<'w, 's, (), With<Room>>,
    world_map: Res<'w, WorldMap>,
    primary_window: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    rtt_runtime: Res<'w, RttRuntime>,
    quality: Res<'w, QualitySettings>,
    exit: MessageWriter<'w, AppExit>,
}

pub(crate) fn observe_perf_behavior_system(mut params: BehaviorObserveParams) {
    if !params.capture.step_issued || params.capture.phase == BehaviorPhase::Finished {
        return;
    }
    let terminal_outcomes = params.outcomes.read().cloned().collect::<Vec<_>>();
    if terminal_outcomes.len() > 1 {
        fail_behavior(
            &mut params.capture,
            "behavior received multiple save/load outcomes in one update",
            &mut params.exit,
        );
        return;
    }
    match params.capture.phase {
        BehaviorPhase::DoorStep(step) => {
            if !terminal_outcomes.is_empty() {
                fail_behavior(
                    &mut params.capture,
                    "Door behavior received an unexpected save/load outcome",
                    &mut params.exit,
                );
                return;
            }
            let Some(door_entity) = params.capture.subject_door else {
                return fail_behavior(
                    &mut params.capture,
                    "Door behavior has no subject",
                    &mut params.exit,
                );
            };
            let Ok((door, building, transform, children)) = params.doors.get(door_entity) else {
                return fail_behavior(
                    &mut params.capture,
                    "Door behavior subject no longer has the production Door topology",
                    &mut params.exit,
                );
            };
            let child_sprites = children
                .iter()
                .filter_map(|child| params.sprites.get(child).ok())
                .collect::<Vec<_>>();
            if child_sprites.len() != 1 || params.sprites.contains(door_entity) {
                return fail_behavior(
                    &mut params.capture,
                    "Door behavior subject differs from root-Door/child-Sprite topology",
                    &mut params.exit,
                );
            }
            let owner_3d_count = params
                .building_3d_visuals
                .iter()
                .filter(|visual| visual.owner == door_entity)
                .count();
            if building.kind != BuildingType::Door
                || WorldMap::world_to_grid(transform.translation.truncate()) != SMALL_DOOR_GRID
                || params
                    .world_map
                    .door_entity(SMALL_DOOR_GRID.0, SMALL_DOOR_GRID.1)
                    != Some(door_entity)
                || params
                    .world_map
                    .door_state(SMALL_DOOR_GRID.0, SMALL_DOOR_GRID.1)
                    != Some(door.state)
                || owner_3d_count != 1
            {
                return fail_behavior(
                    &mut params.capture,
                    "Door behavior subject differs from its building/grid/WorldMap/3D owner relation",
                    &mut params.exit,
                );
            }
            let semantic_state = door_state_name(door.state);
            let presentation_state = if child_sprites[0].image == params.door_handles.door_open {
                "open"
            } else if child_sprites[0].image == params.door_handles.door_closed {
                "closed"
            } else {
                "unknown"
            };
            let expected_paused = matches!(step, 2 | 3);
            if semantic_state != "closed"
                || presentation_state != "closed"
                || params.virtual_time.is_paused() != expected_paused
            {
                fail_behavior(
                    &mut params.capture,
                    &format!(
                        "Door behavior step {step} observed semantic={semantic_state}, presentation={presentation_state}, paused={}; expected closed/closed/{expected_paused}",
                        params.virtual_time.is_paused()
                    ),
                    &mut params.exit,
                );
                return;
            }
            let intents = [
                "observe-initial",
                "auto-open-nearby-soul",
                "pause",
                "manual-lock-while-paused",
                "resume",
            ];
            let simulation_tick = params.capture.simulation_tick;
            let fixture_checksum = params.capture.fixture_checksum.unwrap_or("");
            append_row(
                &mut params.capture,
                TimelineRow {
                    case_id: "door-state-v1",
                    step_index: step,
                    script_update: u64::from(step),
                    simulation_tick,
                    pause_state: if expected_paused { "paused" } else { "running" },
                    world_epoch: params.world_epoch.get(),
                    intent: intents[step as usize],
                    attempted: matches!(step, 1 | 3),
                    applied: false,
                    semantic_state: Some(semantic_state),
                    active_presentation_state: Some(presentation_state),
                    fixture_checksum,
                    terminal_outcome: if step == 4 {
                        "succeeded"
                    } else {
                        "in_progress"
                    },
                },
            );
            params.capture.step_issued = false;
            params.capture.phase = if step == 4 {
                BehaviorPhase::Flush
            } else {
                BehaviorPhase::DoorStep(step + 1)
            };
        }
        BehaviorPhase::LoadStep(step) => {
            let outcome = terminal_outcomes.first();
            let (intent, attempted, applied, ready) = match step {
                0 => ("observe-initial", false, false, true),
                1 => ("request-save", true, false, outcome.is_none()),
                2 => match outcome {
                    Some(outcome)
                        if outcome.operation == SaveLoadOperation::Save
                            && outcome.result == SaveLoadResult::Succeeded =>
                    {
                        params.capture.save_outcomes += 1;
                        ("observe-save-succeeded", false, true, true)
                    }
                    Some(_) => {
                        fail_behavior(
                            &mut params.capture,
                            "normal-load behavior received a failed or wrong save outcome",
                            &mut params.exit,
                        );
                        return;
                    }
                    None => ("observe-save-succeeded", false, false, false),
                },
                3 => ("request-load", true, false, outcome.is_none()),
                4 => match outcome {
                    Some(outcome)
                        if outcome.operation == SaveLoadOperation::Load
                            && outcome.result == SaveLoadResult::Succeeded =>
                    {
                        params.capture.load_outcomes += 1;
                        ("observe-load-succeeded", false, true, true)
                    }
                    Some(_) => {
                        fail_behavior(
                            &mut params.capture,
                            "normal-load behavior received a failed or wrong load outcome",
                            &mut params.exit,
                        );
                        return;
                    }
                    None => ("observe-load-succeeded", false, false, false),
                },
                5 => {
                    match validate_loaded_small_fixture(
                        &params.buildings,
                        &params.souls,
                        &params.familiars,
                        &params.yards,
                        &params.rooms,
                        &params.world_map,
                    ) {
                        Ok(()) => ("verify-semantic-rebind", false, true, true),
                        Err(reason) if params.capture.load_wait_updates < 128 => {
                            params.capture.load_wait_updates += 1;
                            debug!("PERF_BEHAVIOR: waiting for load convergence: {reason}");
                            ("verify-semantic-rebind", false, false, false)
                        }
                        Err(reason) => {
                            fail_behavior(
                                &mut params.capture,
                                &format!("normal-load semantic rebind failed: {reason}"),
                                &mut params.exit,
                            );
                            return;
                        }
                    }
                }
                _ => {
                    fail_behavior(
                        &mut params.capture,
                        "normal-load observer reached an unknown step",
                        &mut params.exit,
                    );
                    return;
                }
            };
            if !ready {
                return;
            }
            if step == 4 && params.world_epoch.get() != params.capture.initial_epoch.wrapping_add(1)
            {
                fail_behavior(
                    &mut params.capture,
                    "normal load did not advance WorldEpoch exactly once",
                    &mut params.exit,
                );
                return;
            }
            if params.virtual_time.is_paused() != params.capture.initial_paused {
                fail_behavior(
                    &mut params.capture,
                    "normal load changed the pause state",
                    &mut params.exit,
                );
                return;
            }
            let simulation_tick = params.capture.simulation_tick;
            let fixture_checksum = params.capture.fixture_checksum.unwrap_or("");
            append_row(
                &mut params.capture,
                TimelineRow {
                    case_id: "load-normal-v1",
                    step_index: step,
                    script_update: u64::from(step),
                    simulation_tick,
                    pause_state: if params.virtual_time.is_paused() {
                        "paused"
                    } else {
                        "running"
                    },
                    world_epoch: params.world_epoch.get(),
                    intent,
                    attempted,
                    applied,
                    semantic_state: None,
                    active_presentation_state: None,
                    fixture_checksum,
                    terminal_outcome: if step == 5 {
                        "succeeded"
                    } else {
                        "in_progress"
                    },
                },
            );
            params.capture.step_issued = false;
            params.capture.phase = if step == 5 {
                BehaviorPhase::Flush
            } else {
                BehaviorPhase::LoadStep(step + 1)
            };
        }
        BehaviorPhase::Flush => {}
        _ => return,
    }

    if params.capture.phase != BehaviorPhase::Flush {
        return;
    }
    if params.config.behavior_case() == Some(PerfBehaviorCase::LoadNormalV1)
        && (params.capture.save_outcomes, params.capture.load_outcomes) != (1, 1)
    {
        fail_behavior(
            &mut params.capture,
            "normal-load behavior did not observe exactly one save and one load outcome",
            &mut params.exit,
        );
        return;
    }
    let final_window = PerfWindowObservation::capture(
        params.primary_window.single().ok(),
        &params.rtt_runtime,
        &params.quality,
        None,
    );
    let result = write_behavior_timeline(&params.config, &params.capture.rows)
        .and_then(|()| {
            let initial = params.capture.initial_window.as_ref().ok_or_else(|| {
                std::io::Error::other("behavior flush has no initial window observation")
            })?;
            write_window_observation(&params.config, initial, &final_window)
        })
        .and_then(|()| write_indoor_light_fixture_sidecars(&params.config, &params.fixture));
    params.capture.phase = BehaviorPhase::Finished;
    match result {
        Ok(()) => {
            eprintln!(
                "PERF_BEHAVIOR: wrote {} timeline rows",
                params.capture.rows.len()
            );
            params.exit.write(AppExit::Success);
        }
        Err(error) => {
            error!("PERF_BEHAVIOR: artifact write failed: {error}");
            params.exit.write(AppExit::error());
        }
    }
}

fn append_row(capture: &mut PerfBehaviorCapture, row: TimelineRow) {
    capture.rows.push(row);
}

fn write_behavior_timeline(
    config: &PerfScenarioConfig,
    rows: &[TimelineRow],
) -> std::io::Result<()> {
    let directory = perf_output_directory(config);
    std::fs::create_dir_all(&directory)?;
    let path = directory.join("timeline.json");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    let row_json = rows
        .iter()
        .map(TimelineRow::json)
        .collect::<Vec<_>>()
        .join(",\n    ");
    let body = format!(
        "{{\n  \"schema_version\": 1,\n  \"complete\": true,\n  \"rows\": [\n    {row_json}\n  ]\n}}\n"
    );
    file.write_all(body.as_bytes())?;
    file.sync_all()
}

fn validate_loaded_small_fixture(
    buildings: &Query<'_, '_, (Entity, &Building, &Transform)>,
    souls: &Query<'_, '_, (), With<DamnedSoul>>,
    familiars: &Query<'_, '_, (), With<Familiar>>,
    yards: &Query<'_, '_, &Yard>,
    rooms: &Query<'_, '_, (), With<Room>>,
    world_map: &WorldMap,
) -> Result<(), String> {
    if (
        souls.iter().count(),
        familiars.iter().count(),
        yards.iter().count(),
    ) != (50, 4, SMALL_GLOBAL_YARD_COUNT)
    {
        return Err("Soul/Familiar/Yard counts have not converged".to_string());
    }
    if rooms.iter().count() != 1 {
        return Err("Room count has not converged".to_string());
    }
    let mut floors = BTreeSet::new();
    let mut walls = BTreeSet::new();
    let mut doors = Vec::new();
    let mut lamps = BTreeSet::new();
    let mut spas = BTreeSet::new();
    for (entity, building, transform) in buildings.iter() {
        let grid = WorldMap::world_to_grid(transform.translation.truncate());
        match building.kind {
            BuildingType::Floor => {
                floors.insert(grid);
            }
            BuildingType::Wall => {
                walls.insert(grid);
            }
            BuildingType::Door => doors.push((entity, grid)),
            BuildingType::OutdoorLamp => {
                lamps.insert(grid);
            }
            BuildingType::SoulSpa => {
                spas.insert(grid);
            }
            _ => {}
        }
    }
    let expected_floors = (21..=26)
        .flat_map(|y| (17..=22).map(move |x| (x, y)))
        .collect::<BTreeSet<_>>();
    let expected_walls = (20..=27)
        .flat_map(|y| [(16, y), (23, y)])
        .chain((17..=22).flat_map(|x| [(x, 20), (x, 27)]))
        .filter(|grid| *grid != SMALL_DOOR_GRID)
        .collect::<BTreeSet<_>>();
    if floors != expected_floors || walls != expected_walls {
        return Err("Floor or Wall semantic grid set differs after load".to_string());
    }
    if doors.len() != 1 || doors[0].1 != SMALL_DOOR_GRID {
        return Err("Door semantic identity differs after load".to_string());
    }
    if world_map.door_entity(SMALL_DOOR_GRID.0, SMALL_DOOR_GRID.1) != Some(doors[0].0)
        || world_map.door_state(SMALL_DOOR_GRID.0, SMALL_DOOR_GRID.1)
            != Some(hw_core::world::DoorState::Closed)
    {
        return Err("Door WorldMap owner/state relation differs after load".to_string());
    }
    // SoulSpa's root is centered on its two-tile-wide footprint; its placement
    // anchor is `(21, 26)`, while the root `Transform` maps to `(22, 26)`.
    if lamps != BTreeSet::from([(17, 21), (80, 80)]) || spas != BTreeSet::from([(22, 26)]) {
        return Err("Lamp or SoulSpa semantic grids differ after load".to_string());
    }
    Ok(())
}

const fn door_state_name(state: hw_core::world::DoorState) -> &'static str {
    match state {
        hw_core::world::DoorState::Open => "open",
        hw_core::world::DoorState::Closed => "closed",
        hw_core::world::DoorState::Locked => "locked",
    }
}

fn json_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn fail_behavior(
    capture: &mut PerfBehaviorCapture,
    reason: &str,
    exit: &mut MessageWriter<AppExit>,
) {
    if capture.phase == BehaviorPhase::Finished {
        return;
    }
    capture.phase = BehaviorPhase::Finished;
    error!("PERF_BEHAVIOR: {reason}");
    exit.write(AppExit::error());
}
