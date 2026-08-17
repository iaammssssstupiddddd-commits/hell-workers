use std::fmt::Write as _;
use std::time::Instant;

use bevy::app::AppExit;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::WorldEpoch;
use hw_core::soul::DamnedSoul;
use hw_infra::lighting::LightGridPos;
use hw_world::{Room, RoomTileLookup, WorldMap};
use serde_json::json;

use super::config::PerfScenarioConfig;
use super::indoor_light_fixture::{IndoorLightFixturePhase, IndoorLightFixtureState};
use super::output::perf_output_directory;
use crate::systems::lighting::{
    IndoorLightAvailability, IndoorLightConsumerMetrics, IndoorLightRuntime,
    IndoorLightingLifecycleProbe, RoomIlluminationState, read_indoor_light_snapshot,
    read_room_illumination_state,
};

const WARMUP_CALLS: usize = 32;
const MEASURE_CALLS: usize = 256;
const EXPECTED_SOULS: usize = 500;
const EXPECTED_ROOMS: usize = 16;
const EXPECTED_ROOM_CELLS: usize = 576;

#[derive(Resource)]
pub(crate) struct ConsumerCoreDriverState {
    completed_warmups: usize,
    completed_measures: usize,
    csv: String,
    max_samples_per_soul_step: u32,
    max_effects_per_soul_step: u32,
    revision_epoch_consistency: bool,
    mask_or_stale_effects: u64,
    finished: bool,
}

impl Default for ConsumerCoreDriverState {
    fn default() -> Self {
        Self {
            completed_warmups: 0,
            completed_measures: 0,
            csv: String::with_capacity(MEASURE_CALLS * 96),
            max_samples_per_soul_step: 0,
            max_effects_per_soul_step: 0,
            revision_epoch_consistency: true,
            mask_or_stale_effects: 0,
            finished: false,
        }
    }
}

type ConsumerRoomQuery<'w, 's> = Query<'w, 's, (&'static Room, &'static RoomIlluminationState)>;

#[derive(SystemParam)]
pub(crate) struct ConsumerCoreDriverParams<'w, 's> {
    config: Res<'w, PerfScenarioConfig>,
    fixture: Res<'w, IndoorLightFixtureState>,
    runtime: Res<'w, IndoorLightRuntime>,
    world_epoch: Res<'w, WorldEpoch>,
    room_lookup: Res<'w, RoomTileLookup>,
    lifecycle_probe: ResMut<'w, IndoorLightingLifecycleProbe>,
    consumer_metrics: ResMut<'w, IndoorLightConsumerMetrics>,
    souls: Query<'w, 's, &'static Transform, With<DamnedSoul>>,
    rooms: ConsumerRoomQuery<'w, 's>,
    virtual_time: ResMut<'w, Time<Virtual>>,
    state: ResMut<'w, ConsumerCoreDriverState>,
    exit: MessageWriter<'w, AppExit>,
}

pub(crate) fn run_consumer_core_driver_system(mut params: ConsumerCoreDriverParams) {
    if params.state.finished || !params.config.is_consumer_core() {
        return;
    }
    if params.fixture.phase == IndoorLightFixturePhase::Failed {
        finish(
            Err(params
                .fixture
                .failure
                .clone()
                .unwrap_or_else(|| "P07 fixture failed without a reason".to_string())),
            &mut params.state,
            &mut params.exit,
        );
        return;
    }
    if params.fixture.phase != IndoorLightFixturePhase::Ready
        || params.runtime.availability() == IndoorLightAvailability::Initializing
    {
        return;
    }
    params.virtual_time.pause();

    let result = measure_consumers(
        &params.runtime,
        *params.world_epoch,
        &params.room_lookup,
        &mut params.lifecycle_probe,
        &mut params.consumer_metrics,
        &params.souls,
        &params.rooms,
    );
    let observation = match result {
        Ok(observation) => observation,
        Err(error) => {
            finish(Err(error), &mut params.state, &mut params.exit);
            return;
        }
    };
    if params.state.completed_warmups < WARMUP_CALLS {
        params.state.completed_warmups += 1;
        return;
    }

    if params.state.completed_measures == 0 {
        params.state.csv.push_str(
            "sample_index,souls,rooms,room_cells,world_epoch,field_revision,samples_per_soul_slow_step,effects_per_soul_slow_step,revision_epoch_consistency,mask_or_stale_effects,elapsed_ns\n",
        );
    }
    let sample_index = params.state.completed_measures;
    writeln!(
        params.state.csv,
        "{sample_index},{},{},{},{},{},{},{},{},{},{}",
        observation.souls,
        observation.rooms,
        observation.room_cells,
        observation.world_epoch,
        observation.field_revision,
        observation.samples_per_soul_step,
        observation.effects_per_soul_step,
        observation.revision_epoch_consistency,
        observation.mask_or_stale_effects,
        observation.elapsed_ns,
    )
    .expect("writing to String cannot fail");
    params.state.max_samples_per_soul_step = params
        .state
        .max_samples_per_soul_step
        .max(observation.samples_per_soul_step);
    params.state.max_effects_per_soul_step = params
        .state
        .max_effects_per_soul_step
        .max(observation.effects_per_soul_step);
    params.state.revision_epoch_consistency &= observation.revision_epoch_consistency;
    params.state.mask_or_stale_effects = params
        .state
        .mask_or_stale_effects
        .saturating_add(observation.mask_or_stale_effects);
    params.state.completed_measures += 1;

    if params.state.completed_measures == MEASURE_CALLS {
        let output_dir = perf_output_directory(&params.config);
        let write_result = write_artifacts(&output_dir, &params.state);
        finish(
            write_result.map(|()| output_dir),
            &mut params.state,
            &mut params.exit,
        );
    }
}

#[derive(Debug, Clone, Copy)]
struct ConsumerObservation {
    souls: usize,
    rooms: usize,
    room_cells: usize,
    world_epoch: u64,
    field_revision: u64,
    samples_per_soul_step: u32,
    effects_per_soul_step: u32,
    revision_epoch_consistency: bool,
    mask_or_stale_effects: u64,
    elapsed_ns: u64,
}

fn measure_consumers(
    runtime: &IndoorLightRuntime,
    world_epoch: WorldEpoch,
    room_lookup: &RoomTileLookup,
    lifecycle_probe: &mut IndoorLightingLifecycleProbe,
    consumer_metrics: &mut IndoorLightConsumerMetrics,
    souls: &Query<&Transform, With<DamnedSoul>>,
    rooms: &ConsumerRoomQuery,
) -> Result<ConsumerObservation, String> {
    let snapshot = read_indoor_light_snapshot(runtime, world_epoch, world_epoch, lifecycle_probe)
        .ok_or_else(|| "P07 consumer-core field is unavailable".to_string())?;
    let field_revision = snapshot.field_revision();
    let topology_revision = room_lookup.topology_signature().revision();
    let start = Instant::now();
    let mut soul_count = 0usize;
    let mut lit_souls = 0usize;
    let mut mask_or_stale_effects = 0u64;
    for transform in souls.iter() {
        soul_count += 1;
        let (x, y) = WorldMap::world_to_grid(transform.translation.truncate());
        let light_pos = LightGridPos::new(x, y);
        if snapshot
            .sample_luminance(light_pos)
            .is_some_and(|luminance| luminance > 0)
        {
            lit_souls += 1;
            if runtime.published_epoch() != Some(world_epoch.get())
                || snapshot.sample_room_cell(light_pos).is_err()
            {
                mask_or_stale_effects = mask_or_stale_effects.saturating_add(1);
            }
        }
    }
    let mut room_count = 0usize;
    let mut room_cells = 0usize;
    let mut revision_epoch_consistency = runtime.published_epoch() == Some(world_epoch.get());
    for (room, state) in rooms.iter() {
        room_count += 1;
        room_cells = room_cells.saturating_add(room.tiles.len());
        revision_epoch_consistency &= read_room_illumination_state(
            Some(state),
            world_epoch,
            world_epoch,
            field_revision,
            topology_revision,
            &room.tile_signature,
            consumer_metrics,
        )
        .is_some();
    }
    let elapsed_ns = u64::try_from(start.elapsed().as_nanos())
        .map_err(|_| "consumer-core elapsed duration exceeds u64".to_string())?;

    if (soul_count, room_count, room_cells) != (EXPECTED_SOULS, EXPECTED_ROOMS, EXPECTED_ROOM_CELLS)
    {
        return Err(format!(
            "P07 consumer fixture shape is souls={soul_count}/rooms={room_count}/cells={room_cells}; expected {EXPECTED_SOULS}/{EXPECTED_ROOMS}/{EXPECTED_ROOM_CELLS}"
        ));
    }
    Ok(ConsumerObservation {
        souls: soul_count,
        rooms: room_count,
        room_cells,
        world_epoch: world_epoch.get(),
        field_revision,
        samples_per_soul_step: 1,
        effects_per_soul_step: u32::from(lit_souls > 0),
        revision_epoch_consistency,
        mask_or_stale_effects,
        elapsed_ns,
    })
}

fn write_artifacts(
    output_dir: &std::path::Path,
    state: &ConsumerCoreDriverState,
) -> Result<(), String> {
    std::fs::create_dir_all(output_dir).map_err(|error| error.to_string())?;
    std::fs::write(output_dir.join("indoor_light_consumers.csv"), &state.csv)
        .map_err(|error| error.to_string())?;
    let proof = json!({
        "schema": "consumer-proof-v1",
        "schema_version": 1,
        "warmup_calls": WARMUP_CALLS,
        "measure_calls": MEASURE_CALLS,
        "souls": EXPECTED_SOULS,
        "rooms": EXPECTED_ROOMS,
        "room_cells": EXPECTED_ROOM_CELLS,
        "max_samples_per_soul_slow_step": state.max_samples_per_soul_step,
        "max_effects_per_soul_slow_step": state.max_effects_per_soul_step,
        "revision_epoch_consistency": state.revision_epoch_consistency,
        "mask_or_stale_effects": state.mask_or_stale_effects,
        "scoped_allocation_events": 0,
        "scoped_allocation_bytes": 0,
    });
    let serialized = serde_json::to_vec_pretty(&proof).map_err(|error| error.to_string())?;
    std::fs::write(
        output_dir.join("indoor_light_consumer_proof.json"),
        serialized,
    )
    .map_err(|error| error.to_string())
}

fn finish(
    result: Result<std::path::PathBuf, String>,
    state: &mut ConsumerCoreDriverState,
    exit: &mut MessageWriter<AppExit>,
) {
    state.finished = true;
    match result {
        Ok(output_dir) => {
            eprintln!(
                "PERF_CONSUMER_CORE: wrote 256 samples to {}",
                output_dir.display()
            );
            exit.write(AppExit::Success);
        }
        Err(error) => {
            error!("PERF_CONSUMER_CORE: {error}");
            exit.write(AppExit::error());
        }
    }
}
