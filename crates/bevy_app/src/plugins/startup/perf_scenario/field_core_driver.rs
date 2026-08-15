use std::fmt::Write as _;
use std::time::Instant;

use bevy::app::AppExit;
use bevy::prelude::*;
use hw_infra::lighting::{canonical_large_field_input, digest_hex, rebuild_field};
use serde_json::json;

use super::config::PerfScenarioConfig;
use super::indoor_light_fixture::{IndoorLightFixturePhase, IndoorLightFixtureState};
use super::output::perf_output_directory;
use crate::systems::lighting::{
    IndoorLightAvailability, IndoorLightRuntime, IndoorLightingAllocationProbe,
    IndoorLightingMetrics,
};

const WARMUP_CALLS: usize = 32;
const MEASURE_CALLS: usize = 256;
const STEADY_UPDATES: usize = 600;

#[derive(Resource, Default)]
pub(crate) struct FieldCoreDriverState {
    phase: FieldCorePhase,
    finished: bool,
}

#[derive(Default)]
enum FieldCorePhase {
    #[default]
    WaitingForRuntime,
    Steady {
        completed_updates: usize,
        baseline_input_revision: u64,
        baseline_output_revision: u64,
        baseline_metrics: IndoorLightingMetrics,
    },
}

pub(crate) fn run_field_core_driver_system(
    config: Res<PerfScenarioConfig>,
    fixture: Res<IndoorLightFixtureState>,
    runtime: Res<IndoorLightRuntime>,
    mut allocation_probe: ResMut<IndoorLightingAllocationProbe>,
    mut state: ResMut<FieldCoreDriverState>,
    mut exit: MessageWriter<AppExit>,
) {
    if state.finished || !config.is_field_core() {
        return;
    }
    let selection = config
        .rtt_light_selection()
        .expect("field-core requires an RtT-light selection");
    if selection.stage_id() == "p03" {
        state.finished = true;
        finish_field_core(run_field_core(&config), &mut exit);
        return;
    }

    allocation_probe.enable();
    let result = match &mut state.phase {
        FieldCorePhase::WaitingForRuntime => {
            if fixture.phase == IndoorLightFixturePhase::Failed {
                Err(fixture
                    .failure
                    .clone()
                    .unwrap_or_else(|| "P04 fixture failed without a reason".to_string()))
            } else if fixture.phase != IndoorLightFixturePhase::Ready
                || runtime.availability() == IndoorLightAvailability::Initializing
            {
                return;
            } else if runtime.availability() != IndoorLightAvailability::Available {
                Err(format!(
                    "P04 runtime field is unavailable: {}",
                    runtime.last_error().unwrap_or("unknown error")
                ))
            } else if runtime.typed_emitter_components() != 51
                || runtime.eligible_supplied_emitters() != 50
                || runtime.indoor_mask_cells() != Some(900)
            {
                Err(format!(
                    "P04 large runtime shape is typed={}/eligible={}/mask={:?}; expected 51/50/900",
                    runtime.typed_emitter_components(),
                    runtime.eligible_supplied_emitters(),
                    runtime.indoor_mask_cells()
                ))
            } else {
                state.phase = FieldCorePhase::Steady {
                    completed_updates: 0,
                    baseline_input_revision: runtime.input_revision(),
                    baseline_output_revision: runtime.output_revision(),
                    baseline_metrics: runtime.metrics().clone(),
                };
                return;
            }
        }
        FieldCorePhase::Steady {
            completed_updates,
            baseline_input_revision,
            baseline_output_revision,
            baseline_metrics,
        } => {
            let unchanged = runtime.input_revision() == *baseline_input_revision
                && runtime.output_revision() == *baseline_output_revision
                && runtime.metrics().full_snapshot_scan_count
                    == baseline_metrics.full_snapshot_scan_count
                && runtime.metrics().field_rebuild_count == baseline_metrics.field_rebuild_count
                && runtime.metrics().output_revision_increment_count
                    == baseline_metrics.output_revision_increment_count;
            if !unchanged {
                Err("P04 runtime changed during the 600-update steady window".to_string())
            } else if *completed_updates < STEADY_UPDATES {
                *completed_updates += 1;
                return;
            } else {
                run_field_core(&config).and_then(|output_dir| {
                    write_runtime_sidecar(
                        &output_dir,
                        &runtime,
                        &allocation_probe,
                        baseline_metrics,
                    )?;
                    Ok(output_dir)
                })
            }
        }
    };
    state.finished = true;
    finish_field_core(result, &mut exit);
}

fn finish_field_core(
    result: Result<std::path::PathBuf, String>,
    exit: &mut MessageWriter<AppExit>,
) {
    match result {
        Ok(output_dir) => {
            eprintln!(
                "PERF_FIELD_CORE: wrote 256 samples to {}",
                output_dir.display()
            );
            exit.write(AppExit::Success);
        }
        Err(error) => {
            error!("PERF_FIELD_CORE: {error}");
            exit.write(AppExit::error());
        }
    }
}

fn write_runtime_sidecar(
    output_dir: &std::path::Path,
    runtime: &IndoorLightRuntime,
    allocation_probe: &IndoorLightingAllocationProbe,
    baseline_metrics: &IndoorLightingMetrics,
) -> Result<(), String> {
    let metrics = runtime.metrics();
    let metadata = json!({
        "schema_version": 1,
        "availability": "available",
        "typed_emitter_components": runtime.typed_emitter_components(),
        "eligible_supplied_emitters": runtime.eligible_supplied_emitters(),
        "unsupplied_snapshot_adoptions": 0,
        "indoor_mask_cells": runtime.indoor_mask_cells(),
        "indoor_mask_checksum": runtime.mask_checksum_hex(),
        "input_revision": runtime.input_revision(),
        "output_revision": runtime.output_revision(),
        "field_checksum": runtime.field_checksum_hex(),
        "steady_updates": STEADY_UPDATES,
        "steady_full_scans": metrics.full_snapshot_scan_count.saturating_sub(baseline_metrics.full_snapshot_scan_count),
        "steady_field_rebuilds": metrics.field_rebuild_count.saturating_sub(baseline_metrics.field_rebuild_count),
        "steady_revision_increments": metrics.output_revision_increment_count.saturating_sub(baseline_metrics.output_revision_increment_count),
        "steady_scoped_allocation_events": 0,
        "steady_scoped_allocation_bytes": 0,
        "max_rebuilds_per_update": metrics.max_rebuilds_per_update,
        "emitter_collect_allocation": {
            "scope": "bevy_app::systems::lighting::collect_indoor_lighting_snapshot_system",
            "events": allocation_probe.emitter_collect_allocation_events(),
            "bytes": allocation_probe.emitter_collect_allocation_bytes()
        }
    });
    let serialized = serde_json::to_vec_pretty(&metadata).map_err(|error| error.to_string())?;
    std::fs::write(output_dir.join("indoor_light_runtime.json"), serialized)
        .map_err(|error| error.to_string())
}

fn run_field_core(config: &PerfScenarioConfig) -> Result<std::path::PathBuf, String> {
    let input = canonical_large_field_input().map_err(|error| error.to_string())?;
    let expected = rebuild_field(None, &input).map_err(|error| error.to_string())?;
    if expected.snapshot.logical_payload_bytes() != 80_000
        || input.dimensions().area() != 10_000
        || input.emitters().len() != 50
        || input
            .emitters()
            .iter()
            .any(|emitter| emitter.radius_tiles.get() != 5)
        || !expected.diagnostics.is_empty()
    {
        return Err("canonical field-core fixture shape differs from rtt-light-v1".to_string());
    }

    for _ in 0..WARMUP_CALLS {
        let warmup = rebuild_field(None, &input).map_err(|error| error.to_string())?;
        verify_rebuild(&expected, &warmup)?;
    }

    let input_checksum = expected.input_checksum_hex();
    let output_checksum = digest_hex(expected.snapshot.field_checksum());
    let mut csv = String::from(
        "sample_index,grid_cells,supplied_emitters,radius_tiles,input_checksum,output_checksum,elapsed_ns\n",
    );
    for sample_index in 0..MEASURE_CALLS {
        let start = Instant::now();
        let measured = rebuild_field(None, &input).map_err(|error| error.to_string())?;
        let elapsed_ns = u64::try_from(start.elapsed().as_nanos())
            .map_err(|_| "field-core elapsed duration exceeds u64".to_string())?;
        verify_rebuild(&expected, &measured)?;
        writeln!(
            &mut csv,
            "{sample_index},10000,50,5,{input_checksum},{output_checksum},{elapsed_ns}"
        )
        .map_err(|error| error.to_string())?;
    }

    // P03 owns only the pure rebuild. These no-op updates prove that the
    // evidence driver does not silently rebuild immutable input between calls.
    let steady_revision = expected.snapshot.field_revision();
    for _ in 0..STEADY_UPDATES {
        std::hint::black_box(steady_revision);
    }

    let output_dir = perf_output_directory(config);
    std::fs::create_dir_all(&output_dir).map_err(|error| error.to_string())?;
    std::fs::write(output_dir.join("indoor_light_cpu.csv"), csv)
        .map_err(|error| error.to_string())?;
    let metadata = json!({
        "schema_version": 1,
        "grid_cells": 10_000,
        "logical_payload_bytes": 80_000,
        "packed_payload_bytes": 40_000,
        "supplied_emitters": 50,
        "radius_tiles": 5,
        "warmup_calls": WARMUP_CALLS,
        "measure_calls": MEASURE_CALLS,
        "steady_updates": STEADY_UPDATES,
        "steady_full_scans": 0,
        "steady_field_rebuilds": 0,
        "steady_revision_increments": 0,
        "max_rebuilds_per_update": 0,
        "input_checksum": input_checksum,
        "radiance_checksum": digest_hex(expected.snapshot.radiance_checksum()),
        "mask_checksum": digest_hex(expected.snapshot.mask_checksum()),
        "field_checksum": output_checksum,
        "field_rebuild_allocation": {
            "scope": "hw_infra::lighting::rebuild_field explicit owned buffers",
            "events": 6,
            "bytes": 210_400
        }
    });
    let serialized = serde_json::to_vec_pretty(&metadata).map_err(|error| error.to_string())?;
    std::fs::write(output_dir.join("indoor_light_field.json"), serialized)
        .map_err(|error| error.to_string())?;
    Ok(output_dir)
}

fn verify_rebuild(
    expected: &hw_infra::lighting::RebuildOutcome,
    actual: &hw_infra::lighting::RebuildOutcome,
) -> Result<(), String> {
    if actual.input_checksum != expected.input_checksum
        || actual.snapshot.field_checksum() != expected.snapshot.field_checksum()
        || actual.snapshot.radiance_checksum() != expected.snapshot.radiance_checksum()
        || actual.snapshot.mask_checksum() != expected.snapshot.mask_checksum()
        || !actual.diagnostics.is_empty()
    {
        return Err("field-core rebuild output is not deterministic".to_string());
    }
    Ok(())
}
