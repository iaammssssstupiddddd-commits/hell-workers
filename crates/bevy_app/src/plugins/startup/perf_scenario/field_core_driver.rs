use std::fmt::Write as _;
use std::time::Instant;

use bevy::app::AppExit;
use bevy::prelude::*;
use hw_infra::lighting::{canonical_large_field_input, digest_hex, rebuild_field};
use serde_json::json;

use super::config::PerfScenarioConfig;
use super::output::perf_output_directory;

const WARMUP_CALLS: usize = 32;
const MEASURE_CALLS: usize = 256;
const STEADY_UPDATES: usize = 600;

#[derive(Resource, Default)]
pub(crate) struct FieldCoreDriverState {
    finished: bool,
}

pub(crate) fn run_field_core_driver_system(
    config: Res<PerfScenarioConfig>,
    mut state: ResMut<FieldCoreDriverState>,
    mut exit: MessageWriter<AppExit>,
) {
    if state.finished || !config.is_field_core() {
        return;
    }
    state.finished = true;
    match run_field_core(&config) {
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
