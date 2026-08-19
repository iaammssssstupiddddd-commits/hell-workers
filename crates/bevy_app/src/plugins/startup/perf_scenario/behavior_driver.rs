use std::collections::BTreeSet;
use std::fs::OpenOptions;
use std::io::Write;

use bevy::app::AppExit;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use hw_core::WorldEpoch;
use hw_energy::SoulSpaTile;
use hw_jobs::{Building, BuildingType};
use hw_ui::UiIntent;
use hw_world::{Room, Yard};
use serde_json::json;

use crate::systems::lighting::{
    IndoorLightConsumerMetrics, IndoorLightingLifecycleProbe, LightingFixtureMount,
    RoomIlluminationState, read_indoor_light_snapshot,
};
use crate::systems::save::{
    PerfLoadFault, PerfLoadFaultInjection, SaveLoadFailureKind, SaveLoadOperation, SaveLoadOutcome,
    SaveLoadResult, SaveLoadState, SavePath, SaveRecoveryMode, SaveStorageRoot,
    manual_save_request, normal_load_request, recovery_load_request,
};
use crate::systems::visual::indoor_light_texture::IndoorLightTexture;

use super::indoor_light_fixture::IndoorLightFixturePhase;
use super::output::{
    perf_output_directory, write_indoor_light_fixture_sidecars, write_window_observation,
};
use super::*;

const BEHAVIOR_TIMEOUT_UPDATES: u64 = 512;
const SMALL_DOOR_GRID: (i32, i32) = (19, 27);

#[derive(Resource, Default)]
pub(crate) struct PerfBehaviorCapture {
    phase: BehaviorPhase,
    update_count: u64,
    simulation_tick: u64,
    step_issued: bool,
    rows: Vec<TimelineRow>,
    subject_soul: Option<Entity>,
    subject_door: Option<Entity>,
    initial_epoch: WorldEpoch,
    initial_paused: bool,
    save_outcomes: u32,
    load_outcomes: u32,
    load_wait_updates: u32,
    fixture_runtime_revisions: Option<(u64, u64)>,
    fixture_runtime_steady_updates: u8,
    fixture_checksum: Option<&'static str>,
    initial_window: Option<PerfWindowObservation>,
    initial_field_checksum: Option<String>,
    initial_light_runtime: Option<crate::systems::lighting::IndoorLightRuntime>,
    initial_room_tiles: Vec<(i32, i32)>,
    invalid_mount_backup: Option<(Entity, LightingFixtureMount)>,
    initial_reset_count: u64,
    initial_wake_count: u64,
    old_epoch_read_attempted: bool,
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
    field_availability: &'static str,
    field_input_revision: Option<u64>,
    field_output_revision: Option<u64>,
    field_is_dark: Option<bool>,
    field_checksum: Option<String>,
    gpu_availability: &'static str,
    gpu_upload_epoch: Option<u64>,
    gpu_checksum: Option<String>,
    fixture_checksum: &'static str,
    terminal_outcome: &'static str,
    registry_phase: &'static str,
    registry_step_id: Option<&'static str>,
    wake_count: Option<u64>,
    field_read_count: Option<u64>,
    old_epoch_field_read_count: Option<u64>,
}

impl TimelineRow {
    fn json(&self) -> String {
        let optional = |value: Option<&str>| {
            value
                .map(|value| format!("\"{}\"", json_escape(value)))
                .unwrap_or_else(|| "null".to_string())
        };
        let optional_u64 = |value: Option<u64>| {
            value
                .map(|value| value.to_string())
                .unwrap_or_else(|| "null".to_string())
        };
        let optional_bool = |value: Option<bool>| {
            value
                .map(|value| value.to_string())
                .unwrap_or_else(|| "null".to_string())
        };
        format!(
            concat!(
                "{{\"case_id\":\"{}\",\"step_index\":{},\"script_update\":{},",
                "\"simulation_tick\":{},\"pause_state\":\"{}\",\"world_epoch\":{},",
                "\"intent\":\"{}\",\"attempted\":{},\"applied\":{},",
                "\"semantic_state\":{},\"active_presentation_state\":{},",
                "\"registry_phase\":\"{}\",",
                "\"registry_step_id\":{},\"wake_count\":{},",
                "\"field_availability\":\"{}\",",
                "\"field_input_revision\":{},\"field_output_revision\":{},",
                "\"field_read_count\":{},\"old_epoch_field_read_count\":{},",
                "\"field_is_dark\":{},\"field_checksum\":{},",
                "\"gpu_availability\":\"{}\",",
                "\"gpu_upload_epoch\":{},\"gpu_checksum\":{},",
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
            self.registry_phase,
            optional(self.registry_step_id),
            optional_u64(self.wake_count),
            self.field_availability,
            optional_u64(self.field_input_revision),
            optional_u64(self.field_output_revision),
            optional_u64(self.field_read_count),
            optional_u64(self.old_epoch_field_read_count),
            optional_bool(self.field_is_dark),
            optional(self.field_checksum.as_deref()),
            self.gpu_availability,
            optional_u64(self.gpu_upload_epoch),
            optional(self.gpu_checksum.as_deref()),
            self.fixture_checksum,
            self.terminal_outcome,
        )
    }
}

struct TimelineGpuObservation {
    availability: &'static str,
    upload_epoch: Option<u64>,
    checksum: Option<String>,
}

fn timeline_gpu_observation(
    config: &PerfScenarioConfig,
    texture: &IndoorLightTexture,
) -> TimelineGpuObservation {
    let p06 = config
        .rtt_light_selection()
        .is_some_and(|selection| selection.uses_gpu_light_field());
    if !p06 {
        return TimelineGpuObservation {
            availability: "stage_before_gpu_owner",
            upload_epoch: None,
            checksum: None,
        };
    }
    TimelineGpuObservation {
        availability: if texture.uploaded_epoch().is_some() {
            "available"
        } else {
            "unavailable"
        },
        upload_epoch: texture.uploaded_epoch(),
        checksum: texture.uploaded_checksum().map(str::to_owned),
    }
}

#[derive(SystemParam)]
pub(crate) struct BehaviorDriveParams<'w, 's> {
    config: Res<'w, PerfScenarioConfig>,
    applied: Res<'w, PerfScenarioApplied>,
    fixture: Res<'w, IndoorLightFixtureState>,
    indoor_light_runtime: Res<'w, crate::systems::lighting::IndoorLightRuntime>,
    room_lookup: Res<'w, hw_world::RoomTileLookup>,
    capture: ResMut<'w, PerfBehaviorCapture>,
    virtual_time: ResMut<'w, Time<Virtual>>,
    world_epoch: Res<'w, WorldEpoch>,
    world_map: Res<'w, WorldMap>,
    save_path: ResMut<'w, SavePath>,
    save_root: ResMut<'w, SaveStorageRoot>,
    save_state: ResMut<'w, SaveLoadState>,
    recovery_mode: ResMut<'w, SaveRecoveryMode>,
    perf_load_fault: ResMut<'w, PerfLoadFaultInjection>,
    lifecycle_probe: ResMut<'w, IndoorLightingLifecycleProbe>,
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
    lamp_mounts: Query<
        'w,
        's,
        (
            Entity,
            &'static Building,
            &'static Transform,
            &'static mut LightingFixtureMount,
        ),
        Without<DamnedSoul>,
    >,
    primary_window: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    rtt_runtime: Res<'w, RttRuntime>,
    quality: Res<'w, QualitySettings>,
    exit: MessageWriter<'w, AppExit>,
}

struct TimelineFieldObservation {
    availability: &'static str,
    input_revision: Option<u64>,
    output_revision: Option<u64>,
    is_dark: Option<bool>,
    checksum: Option<String>,
}

fn timeline_field_observation(
    config: &PerfScenarioConfig,
    runtime: &crate::systems::lighting::IndoorLightRuntime,
) -> Result<TimelineFieldObservation, String> {
    let uses_runtime = config
        .rtt_light_selection()
        .is_some_and(|selection| selection.uses_runtime_field());
    if !uses_runtime {
        return Ok(TimelineFieldObservation {
            availability: "stage_before_field_owner",
            input_revision: None,
            output_revision: None,
            is_dark: None,
            checksum: None,
        });
    }
    if runtime.availability() != crate::systems::lighting::IndoorLightAvailability::Available {
        if config
            .rtt_light_selection()
            .is_some_and(|selection| matches!(selection.stage_id(), "p05" | "p06" | "p07" | "p08"))
        {
            return Ok(TimelineFieldObservation {
                availability: "unavailable",
                input_revision: None,
                output_revision: None,
                is_dark: Some(runtime.is_fail_dark()),
                checksum: None,
            });
        }
        return Err(format!(
            "P04 behavior observed an unavailable Light Field: {}",
            runtime.last_error().unwrap_or("initializing")
        ));
    }
    Ok(TimelineFieldObservation {
        availability: "available",
        input_revision: Some(runtime.input_revision()),
        output_revision: Some(runtime.output_revision()),
        is_dark: runtime.is_dark(),
        checksum: runtime.field_checksum_hex(),
    })
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
        if params
            .config
            .rtt_light_selection()
            .is_some_and(|selection| selection.uses_runtime_field())
        {
            if params.indoor_light_runtime.availability()
                != crate::systems::lighting::IndoorLightAvailability::Available
            {
                return;
            }
            let revisions = (
                params.indoor_light_runtime.input_revision(),
                params.indoor_light_runtime.output_revision(),
            );
            if params.capture.fixture_runtime_revisions != Some(revisions) {
                params.capture.fixture_runtime_revisions = Some(revisions);
                params.capture.fixture_runtime_steady_updates = 0;
                return;
            }
            params.capture.fixture_runtime_steady_updates = params
                .capture
                .fixture_runtime_steady_updates
                .saturating_add(1);
            if params.capture.fixture_runtime_steady_updates < 2 {
                return;
            }
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
        params.capture.initial_epoch = *params.world_epoch;
        params.capture.subject_soul = Some(subject_soul);
        params.capture.subject_door = Some(subject_door);
        params.capture.fixture_checksum = Some(fixture_checksum);
        params.capture.initial_field_checksum = params.indoor_light_runtime.field_checksum_hex();
        params.capture.initial_light_runtime = Some(params.indoor_light_runtime.clone());
        params.capture.initial_room_tiles = params
            .room_lookup
            .mask_signature()
            .canonical_tiles()
            .to_vec();
        params.lifecycle_probe.enable();
        params.capture.initial_reset_count = params.lifecycle_probe.reset_count();
        params.capture.initial_wake_count = params.lifecycle_probe.wake_count();
        params.capture.initial_window = Some(PerfWindowObservation::capture(
            params.primary_window.single().ok(),
            &params.rtt_runtime,
            &params.quality,
            None,
        ));
        let output_directory = perf_output_directory(&params.config);
        let Some(run_directory) = output_directory.parent() else {
            fail_behavior(
                &mut params.capture,
                "behavior output directory has no run-owned parent",
                &mut params.exit,
            );
            return;
        };
        let private_root = run_directory.join("behavior-runtime");
        if private_root.exists() {
            fail_behavior(
                &mut params.capture,
                "job-owned behavior save root already exists",
                &mut params.exit,
            );
            return;
        }
        if let Err(error) = std::fs::create_dir_all(&private_root) {
            fail_behavior(
                &mut params.capture,
                &format!("failed to create behavior save root: {error}"),
                &mut params.exit,
            );
            return;
        }
        *params.save_root = SaveStorageRoot::new(private_root.clone());
        *params.save_path =
            SavePath::new(private_root.join(hw_core::SaveSlotId::Manual1.canonical_file_name()));
        params.capture.phase = match params.config.behavior_case() {
            Some(PerfBehaviorCase::DoorStateV1) => BehaviorPhase::DoorStep(0),
            Some(
                PerfBehaviorCase::LoadNormalV1
                | PerfBehaviorCase::LoadPreflightRejectV1
                | PerfBehaviorCase::LoadRollbackV1
                | PerfBehaviorCase::LoadRecoveryOnlyV1
                | PerfBehaviorCase::LoadRecoveryFailedV1
                | PerfBehaviorCase::LoadDuplicateResetV1,
            ) => BehaviorPhase::LoadStep(0),
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
            let behavior_case = params.config.behavior_case();
            match step {
                0 | 2 | 4 | 5 => {}
                1 => {
                    if !params.save_state.is_idle() {
                        fail_behavior(
                            &mut params.capture,
                            "save/load dispatcher was busy before behavior save",
                            &mut params.exit,
                        );
                        return;
                    }
                    if behavior_case == Some(PerfBehaviorCase::LoadPreflightRejectV1) {
                        let Some((entity, _, transform, mut mount)) = params
                            .lamp_mounts
                            .iter_mut()
                            .find(|(_, building, _, _)| building.kind == BuildingType::OutdoorLamp)
                        else {
                            fail_behavior(
                                &mut params.capture,
                                "P05 preflight case has no persisted lamp mount",
                                &mut params.exit,
                            );
                            return;
                        };
                        params.capture.invalid_mount_backup = Some((entity, *mount));
                        let origin = WorldMap::world_to_grid(transform.translation.truncate());
                        *mount =
                            LightingFixtureMount(hw_infra::lighting::FixtureMount::WallMounted {
                                anchor: hw_infra::lighting::LightGridPos::new(
                                    origin.0 + 1,
                                    origin.1,
                                ),
                                inward: hw_infra::lighting::CardinalDirection::West,
                            });
                    }
                    if !params
                        .save_state
                        .try_set(manual_save_request(hw_core::SaveSlotId::Manual1, 1))
                    {
                        fail_behavior(
                            &mut params.capture,
                            "P05 behavior save request was rejected",
                            &mut params.exit,
                        );
                        return;
                    }
                }
                3 => {
                    if !params.save_state.is_idle() {
                        fail_behavior(
                            &mut params.capture,
                            "save/load dispatcher was busy before behavior load",
                            &mut params.exit,
                        );
                        return;
                    }
                    let request = match behavior_case {
                        Some(PerfBehaviorCase::LoadRollbackV1) => {
                            params.perf_load_fault.arm(PerfLoadFault::ApplyRecovered);
                            normal_load_request(hw_core::SaveSlotId::Manual1, 1)
                        }
                        Some(PerfBehaviorCase::LoadRecoveryOnlyV1) => {
                            *params.recovery_mode = SaveRecoveryMode::RecoveryFailed;
                            recovery_load_request(hw_core::SaveSlotId::Manual1, 1)
                        }
                        Some(PerfBehaviorCase::LoadRecoveryFailedV1) => {
                            params
                                .perf_load_fault
                                .arm(PerfLoadFault::RecoveryFailedNormalApply);
                            normal_load_request(hw_core::SaveSlotId::Manual1, 1)
                        }
                        Some(PerfBehaviorCase::LoadDuplicateResetV1) => {
                            params.perf_load_fault.arm(PerfLoadFault::DuplicateReset);
                            normal_load_request(hw_core::SaveSlotId::Manual1, 1)
                        }
                        _ => normal_load_request(hw_core::SaveSlotId::Manual1, 1),
                    };
                    if !params.save_state.try_set(request) {
                        fail_behavior(
                            &mut params.capture,
                            "P05 behavior load request was rejected",
                            &mut params.exit,
                        );
                        return;
                    }
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
    indoor_light_runtime: Res<'w, crate::systems::lighting::IndoorLightRuntime>,
    indoor_light_texture: Res<'w, IndoorLightTexture>,
    lifecycle_probe: ResMut<'w, IndoorLightingLifecycleProbe>,
    consumer_metrics: Res<'w, IndoorLightConsumerMetrics>,
    room_lookup: Res<'w, hw_world::RoomTileLookup>,
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
    building_3d_visuals: Query<
        'w,
        's,
        (
            &'static Building3dVisual,
            Option<&'static DoorPresentationState>,
        ),
    >,
    door_components: Query<'w, 's, &'static Door>,
    door_handles: Res<'w, DoorVisualHandles>,
    buildings: Query<'w, 's, (Entity, &'static Building, &'static Transform)>,
    lamp_mounts: Query<'w, 's, &'static mut LightingFixtureMount>,
    soul_spa_tiles: Query<'w, 's, &'static SoulSpaTile>,
    souls: Query<'w, 's, (), With<DamnedSoul>>,
    familiars: Query<'w, 's, (), With<Familiar>>,
    yards: Query<'w, 's, &'static Yard>,
    rooms: Query<'w, 's, Option<&'static RoomIlluminationState>, With<Room>>,
    world_map: Res<'w, WorldMap>,
    save_path: Res<'w, SavePath>,
    save_root: Res<'w, SaveStorageRoot>,
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
            let owner_3d_visuals = params
                .building_3d_visuals
                .iter()
                .filter(|(visual, _)| visual.owner == door_entity)
                .collect::<Vec<_>>();
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
                || owner_3d_visuals.len() != 1
            {
                return fail_behavior(
                    &mut params.capture,
                    "Door behavior subject differs from its building/grid/WorldMap/3D owner relation",
                    &mut params.exit,
                );
            }
            let semantic_state = door_state_name(door.state);
            let presentation_state = match owner_3d_visuals[0].1 {
                Some(DoorPresentationState::Closed) => "closed",
                Some(DoorPresentationState::Open) => "open",
                Some(DoorPresentationState::Locked) => "locked",
                None => "unknown",
            };
            let child_sprite_matches = child_sprites[0].image
                == if door.state == DoorState::Open {
                    params.door_handles.door_open.clone()
                } else {
                    params.door_handles.door_closed.clone()
                };
            let p02 = params
                .config
                .rtt_light_selection()
                .is_some_and(|selection| selection.uses_p02_presentation());
            let expected_semantic = if p02 {
                ["closed", "open", "open", "locked", "locked"][step as usize]
            } else {
                "closed"
            };
            let expected_applied = p02 && matches!(step, 1 | 3);
            let expected_paused = matches!(step, 2 | 3);
            let uses_runtime_field = params
                .config
                .rtt_light_selection()
                .is_some_and(|selection| selection.uses_runtime_field());
            if uses_runtime_field && step == 3 && semantic_state != "locked" {
                // P04 owns the N -> N+1 Interface request handoff. Keep the
                // script step pending until the next PreActor consumer pass.
                return;
            }
            if semantic_state != expected_semantic
                || presentation_state != expected_semantic
                || !child_sprite_matches
                || params.virtual_time.is_paused() != expected_paused
            {
                fail_behavior(
                    &mut params.capture,
                    &format!(
                        "Door behavior step {step} observed semantic={semantic_state}, presentation={presentation_state}, paused={}; expected {expected_semantic}/{expected_semantic}/{expected_paused}",
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
            let owns_lifecycle = params
                .config
                .rtt_light_selection()
                .is_some_and(|selection| {
                    matches!(selection.stage_id(), "p05" | "p06" | "p07" | "p08")
                });
            let Ok(field) =
                timeline_field_observation(&params.config, &params.indoor_light_runtime)
            else {
                fail_behavior(
                    &mut params.capture,
                    "behavior timeline could not observe the P04 Light Field",
                    &mut params.exit,
                );
                return;
            };
            let gpu = timeline_gpu_observation(&params.config, &params.indoor_light_texture);
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
                    applied: expected_applied,
                    semantic_state: Some(semantic_state),
                    active_presentation_state: Some(presentation_state),
                    field_availability: field.availability,
                    field_input_revision: field.input_revision,
                    field_output_revision: field.output_revision,
                    field_is_dark: field.is_dark,
                    field_checksum: field.checksum,
                    gpu_availability: gpu.availability,
                    gpu_upload_epoch: gpu.upload_epoch,
                    gpu_checksum: gpu.checksum,
                    fixture_checksum,
                    terminal_outcome: if step == 4 {
                        "succeeded"
                    } else {
                        "in_progress"
                    },
                    registry_phase: if owns_lifecycle {
                        "candidate_preflight"
                    } else {
                        "stage_before_registry_owner"
                    },
                    registry_step_id: None,
                    wake_count: owns_lifecycle.then_some(0),
                    field_read_count: owns_lifecycle
                        .then_some(params.lifecycle_probe.field_read_count()),
                    old_epoch_field_read_count: owns_lifecycle
                        .then_some(params.lifecycle_probe.old_epoch_field_reads()),
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
            let behavior_case = params.config.behavior_case();
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
                        if let Some((entity, original)) = params.capture.invalid_mount_backup.take()
                        {
                            let Ok(mut mount) = params.lamp_mounts.get_mut(entity) else {
                                fail_behavior(
                                    &mut params.capture,
                                    "P05 preflight case could not restore the live lamp mount",
                                    &mut params.exit,
                                );
                                return;
                            };
                            *mount = original;
                        }
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
                4 => {
                    let expected_result = match behavior_case {
                        Some(PerfBehaviorCase::LoadPreflightRejectV1) => {
                            SaveLoadResult::Failed(SaveLoadFailureKind::InvalidData)
                        }
                        Some(PerfBehaviorCase::LoadRollbackV1) => {
                            SaveLoadResult::Failed(SaveLoadFailureKind::ApplyRecovered)
                        }
                        Some(PerfBehaviorCase::LoadRecoveryFailedV1) => {
                            SaveLoadResult::Failed(SaveLoadFailureKind::RecoveryFailed)
                        }
                        _ => SaveLoadResult::Succeeded,
                    };
                    match outcome {
                        Some(outcome)
                            if outcome.operation == SaveLoadOperation::Load
                                && outcome.result == expected_result =>
                        {
                            params.capture.load_outcomes += 1;
                            (
                                if behavior_case == Some(PerfBehaviorCase::LoadNormalV1) {
                                    "observe-load-succeeded"
                                } else {
                                    "observe-load-terminal"
                                },
                                false,
                                true,
                                true,
                            )
                        }
                        Some(outcome) => {
                            fail_behavior(
                                &mut params.capture,
                                &format!(
                                    "P05 behavior received the wrong load outcome: got={:?}, expected={expected_result:?}",
                                    outcome.result
                                ),
                                &mut params.exit,
                            );
                            return;
                        }
                        None => ("observe-load-terminal", false, false, false),
                    }
                }
                5 => {
                    if behavior_case == Some(PerfBehaviorCase::LoadRecoveryFailedV1) {
                        if params.indoor_light_runtime.is_fail_dark() {
                            ("verify-fail-dark", false, true, true)
                        } else {
                            ("verify-fail-dark", false, false, false)
                        }
                    } else {
                        match params.validate_loaded_small_fixture() {
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
            let expected_epoch_delta =
                u64::from(behavior_case != Some(PerfBehaviorCase::LoadPreflightRejectV1));
            if step == 4
                && params.world_epoch.get()
                    != params
                        .capture
                        .initial_epoch
                        .get()
                        .wrapping_add(expected_epoch_delta)
            {
                fail_behavior(
                    &mut params.capture,
                    "P05 load case observed the wrong WorldEpoch delta",
                    &mut params.exit,
                );
                return;
            }
            let recovery_failed = behavior_case == Some(PerfBehaviorCase::LoadRecoveryFailedV1);
            if (!recovery_failed
                && params.virtual_time.is_paused() != params.capture.initial_paused)
                || (recovery_failed && step >= 4 && !params.virtual_time.is_paused())
            {
                fail_behavior(
                    &mut params.capture,
                    "normal load changed the pause state",
                    &mut params.exit,
                );
                return;
            }
            let simulation_tick = params.capture.simulation_tick;
            let fixture_checksum = params.capture.fixture_checksum.unwrap_or("");
            if step == 5 && expected_epoch_delta == 1 && !params.capture.old_epoch_read_attempted {
                let requested = params.capture.initial_epoch;
                let current = *params.world_epoch;
                let _ = read_indoor_light_snapshot(
                    &params.indoor_light_runtime,
                    requested,
                    current,
                    &mut params.lifecycle_probe,
                );
                params.capture.old_epoch_read_attempted = true;
            }
            let Ok(mut field) =
                timeline_field_observation(&params.config, &params.indoor_light_runtime)
            else {
                fail_behavior(
                    &mut params.capture,
                    "behavior timeline could not observe the P04 Light Field",
                    &mut params.exit,
                );
                return;
            };
            let mut gpu = timeline_gpu_observation(&params.config, &params.indoor_light_texture);
            let reset_count = params
                .lifecycle_probe
                .reset_count()
                .saturating_sub(params.capture.initial_reset_count);
            let wake_count = params
                .lifecycle_probe
                .wake_count()
                .saturating_sub(params.capture.initial_wake_count);
            if step == 4 && expected_epoch_delta == 1 {
                field = TimelineFieldObservation {
                    availability: "unavailable",
                    input_revision: None,
                    output_revision: None,
                    is_dark: Some(params.lifecycle_probe.all_resets_fail_dark()),
                    checksum: None,
                };
                if params
                    .config
                    .rtt_light_selection()
                    .is_some_and(|selection| selection.uses_gpu_light_field())
                {
                    gpu = TimelineGpuObservation {
                        availability: "unavailable",
                        upload_epoch: None,
                        checksum: None,
                    };
                }
            }
            if step == 5 {
                let expected_counts = match behavior_case {
                    Some(PerfBehaviorCase::LoadPreflightRejectV1) => (0, 0),
                    Some(PerfBehaviorCase::LoadRollbackV1) => (2, 1),
                    Some(PerfBehaviorCase::LoadRecoveryOnlyV1) => (1, 1),
                    Some(PerfBehaviorCase::LoadRecoveryFailedV1) => (2, 0),
                    Some(PerfBehaviorCase::LoadDuplicateResetV1) => (2, 1),
                    _ => (1, 1),
                };
                let checksum_matches = behavior_case
                    != Some(PerfBehaviorCase::LoadPreflightRejectV1)
                    || field.checksum == params.capture.initial_field_checksum;
                if !checksum_matches && params.capture.load_wait_updates < 128 {
                    params.capture.load_wait_updates += 1;
                    return;
                }
                if (reset_count, wake_count) != expected_counts
                    || params.lifecycle_probe.old_epoch_field_reads() != 0
                    || !params.lifecycle_probe.all_resets_fail_dark()
                    || !checksum_matches
                {
                    let reason = format!(
                        "P05 lifecycle proof failed: reset/wake={reset_count}/{wake_count}, expected={}/{}, old_epoch_reads={}, reset_dark={}, checksum_match={checksum_matches}, initial_checksum={:?}, terminal_checksum={:?}",
                        expected_counts.0,
                        expected_counts.1,
                        params.lifecycle_probe.old_epoch_field_reads(),
                        params.lifecycle_probe.all_resets_fail_dark(),
                        params.capture.initial_field_checksum,
                        field.checksum,
                    );
                    fail_behavior(&mut params.capture, &reason, &mut params.exit);
                    return;
                }
            }
            append_row(
                &mut params.capture,
                TimelineRow {
                    case_id: behavior_case.map_or("load-normal-v1", PerfBehaviorCase::as_str),
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
                    field_availability: field.availability,
                    field_input_revision: field.input_revision,
                    field_output_revision: field.output_revision,
                    field_is_dark: field.is_dark,
                    field_checksum: field.checksum,
                    gpu_availability: gpu.availability,
                    gpu_upload_epoch: gpu.upload_epoch,
                    gpu_checksum: gpu.checksum,
                    fixture_checksum,
                    terminal_outcome: if step == 5 {
                        match behavior_case {
                            Some(PerfBehaviorCase::LoadPreflightRejectV1) => "rejected",
                            Some(PerfBehaviorCase::LoadRecoveryFailedV1) => "failed_dark",
                            _ => "succeeded",
                        }
                    } else {
                        "in_progress"
                    },
                    registry_phase: if step == 4 && expected_epoch_delta == 1 {
                        "load_reset"
                    } else if step >= 4 {
                        "wake_domains"
                    } else {
                        "candidate_preflight"
                    },
                    registry_step_id: (step >= 4).then_some("lighting.wake"),
                    wake_count: Some(wake_count),
                    field_read_count: Some(params.lifecycle_probe.field_read_count()),
                    old_epoch_field_read_count: Some(
                        params.lifecycle_probe.old_epoch_field_reads(),
                    ),
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
    let mut room_states = params.rooms.iter();
    let room_state_available =
        room_states.next().is_some_and(|state| state.is_some()) && room_states.next().is_none();
    let result = finalize_behavior_runtime(
        &params.config,
        params.config.behavior_case(),
        params.save_path.as_path(),
        params.save_root.as_path(),
    )
    .and_then(|()| write_behavior_timeline(&params.config, &params.capture.rows))
    .and_then(|()| {
        write_consumer_lifecycle_sidecar(
            &params.config,
            &params.indoor_light_runtime,
            *params.world_epoch,
            &params.consumer_metrics,
            room_state_available,
        )
    })
    .and_then(|()| {
        let initial = params.capture.initial_window.as_ref().ok_or_else(|| {
            std::io::Error::other("behavior flush has no initial window observation")
        })?;
        write_window_observation(&params.config, initial, &final_window)
    })
    .and_then(|()| {
        let runtime =
            if params.config.behavior_case() == Some(PerfBehaviorCase::LoadRecoveryFailedV1) {
                params
                    .capture
                    .initial_light_runtime
                    .as_ref()
                    .ok_or_else(|| {
                        std::io::Error::other("behavior flush has no trusted initial Light Field")
                    })?
            } else {
                &params.indoor_light_runtime
            };
        write_indoor_light_fixture_sidecars(
            &params.config,
            &params.fixture,
            runtime,
            &params.room_lookup,
            (params.config.behavior_case() == Some(PerfBehaviorCase::LoadRecoveryFailedV1))
                .then_some(params.capture.initial_room_tiles.as_slice()),
            None,
        )
    });
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

fn finalize_behavior_runtime(
    config: &PerfScenarioConfig,
    behavior_case: Option<PerfBehaviorCase>,
    save_path: &std::path::Path,
    save_root: &std::path::Path,
) -> std::io::Result<()> {
    if behavior_case.is_some_and(|case| case.as_str().starts_with("load-")) {
        let output_directory = perf_output_directory(config);
        std::fs::create_dir_all(&output_directory)?;
        let artifact_path = output_directory.join("behavior-save.scn.ron");
        if artifact_path.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "behavior-save.scn.ron already exists",
            ));
        }
        std::fs::copy(save_path, artifact_path)?;
    }
    std::fs::remove_dir_all(save_root)
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

fn write_consumer_lifecycle_sidecar(
    config: &PerfScenarioConfig,
    runtime: &crate::systems::lighting::IndoorLightRuntime,
    world_epoch: WorldEpoch,
    metrics: &IndoorLightConsumerMetrics,
    room_state_available: bool,
) -> std::io::Result<()> {
    if config
        .rtt_light_selection()
        .is_none_or(|selection| !selection.uses_cpu_consumers())
    {
        return Ok(());
    }
    let case_id = config
        .behavior_case_as_str()
        .ok_or_else(|| std::io::Error::other("consumer lifecycle sidecar has no behavior case"))?;
    let current_field_available = runtime.published_epoch() == Some(world_epoch.get())
        && runtime.availability() == crate::systems::lighting::IndoorLightAvailability::Available;
    let body = json!({
        "schema": "consumer-lifecycle-v1",
        "schema_version": 1,
        "case_id": case_id,
        "world_epoch": world_epoch.get(),
        "field_revision": current_field_available.then(|| runtime.output_revision()),
        "old_epoch_recovery_effects": metrics.old_epoch_recovery_effects,
        "old_epoch_room_summary_reads": metrics.old_epoch_room_reads,
        "room_state_available": room_state_available,
    });
    let directory = perf_output_directory(config);
    std::fs::create_dir_all(&directory)?;
    let path = directory.join("indoor_light_consumer_lifecycle.json");
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    serde_json::to_writer_pretty(&mut file, &body)?;
    file.write_all(b"\n")?;
    file.sync_all()
}

impl BehaviorObserveParams<'_, '_> {
    fn validate_loaded_small_fixture(&self) -> Result<(), String> {
        let buildings = &self.buildings;
        let door_components = &self.door_components;
        let soul_spa_tiles = &self.soul_spa_tiles;
        let souls = &self.souls;
        let familiars = &self.familiars;
        let yards = &self.yards;
        let rooms = &self.rooms;
        let world_map = &self.world_map;
        let counts = (
            souls.iter().count(),
            familiars.iter().count(),
            yards.iter().count(),
        );
        // The fixed small fixture adds two contract-owned Yards while preserving
        // the seed-owned world-generation Yard after proving that it does not
        // overlap the fixture. A normal load must restore the complete durable
        // world, not only the two fixture-owned Yards.
        if counts != (50, 4, 3) {
            return Err(format!(
                "Soul/Familiar/Yard counts are {}/{}/{}, expected 50/4/3",
                counts.0, counts.1, counts.2
            ));
        }
        if rooms.iter().count() != 1 {
            return Err("Room count has not converged".to_string());
        }
        let mut floors = BTreeSet::new();
        let mut walls = BTreeSet::new();
        let mut doors = Vec::new();
        let mut lamps = BTreeSet::new();
        let mut spa_sites = Vec::new();
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
                    spa_sites.push(entity);
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
        let map_owner = world_map.door_entity(SMALL_DOOR_GRID.0, SMALL_DOOR_GRID.1);
        let map_state = world_map.door_state(SMALL_DOOR_GRID.0, SMALL_DOOR_GRID.1);
        let component_state = door_components
            .get(doors[0].0)
            .map(|door| door.state)
            .map_err(|_| "loaded Door is missing its semantic Door component".to_string())?;
        if map_owner != Some(doors[0].0) || map_state != Some(component_state) {
            return Err(format!(
                "Door WorldMap owner/state relation differs after load: entity={:?}, component_state={component_state:?}, map_owner={map_owner:?}, map_state={map_state:?}",
                doors[0].0,
            ));
        }
        if lamps != BTreeSet::from([(17, 21), (80, 80)]) {
            return Err("Lamp semantic grids differ after load".to_string());
        }
        // SoulSpa is a 2x2 building whose site Transform is the footprint center,
        // not its placement anchor. Converting that center back to one grid loses
        // the anchor semantics. Validate the durable site-to-tile relationship and
        // exact footprint instead; this is also what WorldMap rehydration owns.
        if spa_sites.len() != 1 {
            return Err(format!(
                "SoulSpa site count is {}, expected 1",
                spa_sites.len()
            ));
        }
        let spa_site = spa_sites[0];
        let spa_grids = soul_spa_tiles
            .iter()
            .filter(|tile| tile.parent_site == spa_site)
            .map(|tile| tile.grid_pos)
            .collect::<BTreeSet<_>>();
        let expected_spa_grids = BTreeSet::from([(21, 25), (22, 25), (21, 26), (22, 26)]);
        if spa_grids != expected_spa_grids
            || expected_spa_grids
                .iter()
                .any(|grid| world_map.building_entity(*grid) != Some(spa_site))
        {
            return Err("SoulSpa semantic footprint differs after load".to_string());
        }
        Ok(())
    }
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

#[cfg(test)]
mod system_param_tests {
    use super::*;
    use bevy::ecs::system::{IntoSystem, System};

    #[test]
    fn behavior_driver_queries_are_disjoint() {
        let mut world = World::new();
        let mut system = IntoSystem::into_system(drive_perf_behavior_system);
        system.initialize(&mut world);
    }

    #[test]
    fn behavior_observer_queries_are_disjoint() {
        let mut world = World::new();
        let mut system = IntoSystem::into_system(observe_perf_behavior_system);
        system.initialize(&mut world);
    }
}
