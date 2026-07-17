use super::*;

#[cfg(feature = "profiling")]
pub(super) struct PerfCaptureWriteInput<'a> {
    pub(super) config: &'a PerfScenarioConfig,
    pub(super) initial_checksum: PerfScenarioChecksum,
    pub(super) initial_scene_roots: PerfSceneRootCounts,
    pub(super) warmup_checksum: PerfScenarioChecksum,
    pub(super) measure_end_checksum: PerfScenarioChecksum,
    pub(super) samples: &'a [f64],
    pub(super) warmup_virtual_secs: f64,
    pub(super) warmup_real_secs: f64,
    pub(super) measure_virtual_secs: f64,
    pub(super) measure_real_secs: f64,
    pub(super) familiar_metrics: &'a FamiliarDelegationPerfMetrics,
    pub(super) task_execution_metrics: &'a TaskExecutionPerfMetrics,
    pub(super) reservation_sync_metrics: &'a ReservationSyncPerfMetrics,
    pub(super) door_metrics: &'a DoorPerfMetrics,
    pub(super) construction_metrics: &'a ConstructionPerfMetrics,
    pub(super) slow_simulation_metrics: &'a SlowSimulationPerfMetrics,
    pub(super) energy_metrics: &'a EnergyPerfMetrics,
    pub(super) runtime_path_metrics: &'a RuntimePathSearchMetrics,
    pub(super) runtime_path_defer_metrics: &'a RuntimePathDeferMetrics,
}

#[cfg(feature = "profiling")]
pub(super) fn write_perf_capture(input: PerfCaptureWriteInput<'_>) -> std::io::Result<()> {
    let PerfCaptureWriteInput {
        config,
        initial_checksum,
        initial_scene_roots,
        warmup_checksum,
        measure_end_checksum,
        samples,
        warmup_virtual_secs,
        warmup_real_secs,
        measure_virtual_secs,
        measure_real_secs,
        familiar_metrics,
        task_execution_metrics,
        reservation_sync_metrics,
        door_metrics,
        construction_metrics,
        slow_simulation_metrics,
        energy_metrics,
        runtime_path_metrics,
        runtime_path_defer_metrics,
    } = input;

    if config.uses_fixed_timesteps() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "frame-time capture must not write fixed-step audit artifacts",
        ));
    }
    if samples.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "frame-time capture produced no samples",
        ));
    }

    let directory = perf_output_directory(config);
    std::fs::create_dir_all(&directory)?;

    let frames_path = directory.join("frames.csv");
    let summary_path = directory.join("summary.csv");
    let scene_roots_path = directory.join("scene_roots.csv");
    if frames_path.exists()
        || summary_path.exists()
        || scene_roots_path.exists()
        || directory.join("determinism.csv").exists()
        || directory.join("determinism_records.csv").exists()
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("perf output already exists in {}", directory.display()),
        ));
    }
    let mut frame_csv = String::from("frame_index,frame_time_ms\n");
    for (index, frame_time_ms) in samples.iter().enumerate() {
        frame_csv.push_str(&format!("{index},{frame_time_ms:.6}\n"));
    }
    std::fs::write(&frames_path, frame_csv)?;

    let scene_roots_csv = format!(
        concat!(
            "soul_proxy_3d,soul_mask_proxy_3d,soul_shadow_proxy_3d,",
            "familiar_proxy_3d,building_3d_visual\n",
            "{},{},{},{},{}\n"
        ),
        initial_scene_roots.soul_proxy_3d,
        initial_scene_roots.soul_mask_proxy_3d,
        initial_scene_roots.soul_shadow_proxy_3d,
        initial_scene_roots.familiar_proxy_3d,
        initial_scene_roots.building_3d_visual,
    );
    std::fs::write(&scene_roots_path, scene_roots_csv)?;

    let (p50, p95, p99) = percentile_summary(samples);
    let max = samples.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let summary_header = concat!(
        "schema_version,seed,workload,size,render,configured_souls,configured_familiars,",
        "initial_souls,initial_familiars,initial_designations,initial_state_checksum,",
        "warmup_souls,warmup_familiars,warmup_designations,warmup_state_checksum,",
        "measure_end_souls,measure_end_familiars,measure_end_designations,measure_end_state_checksum,",
        "samples,p50_ms,p95_ms,p99_ms,max_ms,warmup_virtual_secs,warmup_real_secs,",
        "measure_virtual_secs,measure_real_secs,virtual_time_speed,delegation_latest_ms,",
        "delegation_cycles,incoming_snapshot_builds,delegation_familiars_processed,",
        "source_selector_calls,source_selector_scanned_items,",
        "reachable_with_cache_calls,task_execution_souls_queried,task_execution_idle_skips,",
        "task_execution_handler_runs,reservation_sync_full_rebuilds,",
        "reservation_sync_pending_tasks_scanned,reservation_sync_assigned_tasks_scanned,",
        "runtime_path_actor_new_core_searches,runtime_path_actor_new_deferred,",
        "runtime_path_actor_reuse_core_searches,runtime_path_actor_reuse_deferred,",
        "runtime_path_actor_rest_fallback_core_searches,runtime_path_actor_rest_fallback_deferred,",
        "runtime_path_escape_core_searches,runtime_path_escape_deferred,",
        "runtime_path_task_execution_core_searches,runtime_path_task_execution_deferred,",
        "runtime_path_bucket_transport_core_searches,runtime_path_bucket_transport_deferred,",
        "runtime_path_total_core_searches,runtime_path_expanded_nodes,",
        "runtime_path_max_expanded_nodes_per_search,runtime_path_active_task_max_defer_frames,",
        "runtime_path_idle_or_rest_max_defer_frames,runtime_path_deferred_actor_retries,",
        "door_open_souls_scanned,door_open_waypoints_scanned,door_close_souls_scanned,",
        "construction_floor_sites_considered,construction_wall_sites_considered,",
        "construction_floor_tiles_inspected,construction_wall_tiles_inspected,",
        "construction_evacuation_candidates_scanned,",
        "construction_floor_phase_elapsed_micros,construction_floor_completion_elapsed_micros,",
        "construction_wall_phase_elapsed_micros,construction_wall_completion_elapsed_micros,",
        "slow_simulation_steps,slow_simulation_souls_updated,slow_simulation_idle_decisions,",
        "slow_simulation_idle_spatial_target_lookups,slow_simulation_state_sanity_audits,",
        "energy_power_output_runs,energy_grid_recalc_runs,energy_lamp_steps,",
        "energy_lamp_candidates_scanned\n"
    );
    let summary_fields = vec![
        PERF_SUMMARY_SCHEMA_VERSION.to_string(),
        config.master_seed.to_string(),
        config.workload.as_str().to_string(),
        config.size.as_str().to_string(),
        config.render_mode.as_str().to_string(),
        config.soul_count.to_string(),
        config.familiar_count.to_string(),
        initial_checksum.souls.to_string(),
        initial_checksum.familiars.to_string(),
        initial_checksum.designations.to_string(),
        format!("{:016x}", initial_checksum.value),
        warmup_checksum.souls.to_string(),
        warmup_checksum.familiars.to_string(),
        warmup_checksum.designations.to_string(),
        format!("{:016x}", warmup_checksum.value),
        measure_end_checksum.souls.to_string(),
        measure_end_checksum.familiars.to_string(),
        measure_end_checksum.designations.to_string(),
        format!("{:016x}", measure_end_checksum.value),
        samples.len().to_string(),
        format!("{p50:.6}"),
        format!("{p95:.6}"),
        format!("{p99:.6}"),
        format!("{max:.6}"),
        format!("{warmup_virtual_secs:.6}"),
        format!("{warmup_real_secs:.6}"),
        format!("{measure_virtual_secs:.6}"),
        format!("{measure_real_secs:.6}"),
        "1.0".to_string(),
        format!("{:.6}", familiar_metrics.latest_elapsed_ms),
        familiar_metrics.delegation_cycles.to_string(),
        familiar_metrics.incoming_snapshot_builds.to_string(),
        familiar_metrics.familiars_processed.to_string(),
        familiar_metrics.source_selector_calls.to_string(),
        familiar_metrics.source_selector_scanned_items.to_string(),
        familiar_metrics.reachable_with_cache_calls.to_string(),
        task_execution_metrics.souls_queried.to_string(),
        task_execution_metrics.idle_skips.to_string(),
        task_execution_metrics.handler_runs.to_string(),
        reservation_sync_metrics.full_rebuilds.to_string(),
        reservation_sync_metrics.pending_tasks_scanned.to_string(),
        reservation_sync_metrics.assigned_tasks_scanned.to_string(),
        runtime_path_metrics.actor_new_core_searches.to_string(),
        runtime_path_metrics.actor_new_deferred.to_string(),
        runtime_path_metrics.actor_reuse_core_searches.to_string(),
        runtime_path_metrics.actor_reuse_deferred.to_string(),
        runtime_path_metrics
            .actor_rest_fallback_core_searches
            .to_string(),
        runtime_path_metrics
            .actor_rest_fallback_deferred
            .to_string(),
        runtime_path_metrics.escape_core_searches.to_string(),
        runtime_path_metrics.escape_deferred.to_string(),
        runtime_path_metrics
            .task_execution_core_searches
            .to_string(),
        runtime_path_metrics.task_execution_deferred.to_string(),
        runtime_path_metrics
            .bucket_transport_core_searches
            .to_string(),
        runtime_path_metrics.bucket_transport_deferred.to_string(),
        runtime_path_metrics.total_core_searches().to_string(),
        runtime_path_metrics.expanded_nodes.to_string(),
        runtime_path_metrics
            .max_expanded_nodes_per_search
            .to_string(),
        runtime_path_defer_metrics
            .active_task_max_defer_frames
            .to_string(),
        runtime_path_defer_metrics
            .idle_or_rest_max_defer_frames
            .to_string(),
        runtime_path_defer_metrics
            .deferred_actor_retries
            .to_string(),
        door_metrics.open_souls_scanned.to_string(),
        door_metrics.open_waypoints_scanned.to_string(),
        door_metrics.close_souls_scanned.to_string(),
        construction_metrics.floor_sites_considered.to_string(),
        construction_metrics.wall_sites_considered.to_string(),
        construction_metrics.floor_tiles_inspected.to_string(),
        construction_metrics.wall_tiles_inspected.to_string(),
        construction_metrics
            .evacuation_candidates_scanned
            .to_string(),
        construction_metrics.floor_phase_elapsed_micros.to_string(),
        construction_metrics
            .floor_completion_elapsed_micros
            .to_string(),
        construction_metrics.wall_phase_elapsed_micros.to_string(),
        construction_metrics
            .wall_completion_elapsed_micros
            .to_string(),
        slow_simulation_metrics.steps.to_string(),
        slow_simulation_metrics.souls_updated.to_string(),
        slow_simulation_metrics.idle_decisions.to_string(),
        slow_simulation_metrics
            .idle_spatial_target_lookups
            .to_string(),
        slow_simulation_metrics.state_sanity_audits.to_string(),
        energy_metrics.power_output_runs.to_string(),
        energy_metrics.grid_recalc_runs.to_string(),
        energy_metrics.lamp_steps.to_string(),
        energy_metrics.lamp_candidates_scanned.to_string(),
    ];
    let summary = format!("{summary_header}{}\n", summary_fields.join(","));
    std::fs::write(&summary_path, summary)?;
    eprintln!(
        "PERF_CAPTURE: wrote {} samples to {} (p50={p50:.3}ms p95={p95:.3}ms p99={p99:.3}ms initial_checksum={:016x} warmup_checksum={:016x})",
        samples.len(),
        directory.display(),
        initial_checksum.value,
        warmup_checksum.value,
    );
    Ok(())
}

#[cfg(feature = "profiling")]
fn perf_output_directory(config: &PerfScenarioConfig) -> PathBuf {
    config.output_dir.clone().unwrap_or_else(|| {
        PathBuf::from(format!(
            "target/perf/{}-{}-{}-seed-{}",
            config.workload.as_str(),
            config.size.as_str(),
            config.render_mode.as_str(),
            config.master_seed
        ))
    })
}

#[cfg(feature = "profiling")]
fn expected_determinism_checkpoints(config: &PerfScenarioConfig) -> [(&'static str, u64); 7] {
    [
        ("fixture-pre-update", 0),
        ("post-update-1", FIXED_STEP_AUDIT_EARLY_UPDATE_TICKS[0]),
        ("post-update-8", FIXED_STEP_AUDIT_EARLY_UPDATE_TICKS[1]),
        ("post-update-32", FIXED_STEP_AUDIT_EARLY_UPDATE_TICKS[2]),
        ("post-update-128", FIXED_STEP_AUDIT_EARLY_UPDATE_TICKS[3]),
        ("post-warmup", config.fixed_warmup_ticks()),
        ("post-audit-end", config.fixed_audit_end_tick()),
    ]
}

#[cfg(feature = "profiling")]
pub(super) fn write_determinism_audit(
    config: &PerfScenarioConfig,
    checkpoints: &[PerfDeterminismCheckpoint],
    actor_records: &[PerfDeterminismActorRecord],
) -> std::io::Result<()> {
    if !config.uses_fixed_timesteps() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "determinism audit requires --perf-clock fixed",
        ));
    }
    let expected = expected_determinism_checkpoints(config);
    let observed = checkpoints
        .iter()
        .map(|checkpoint| (checkpoint.checkpoint, checkpoint.update_tick))
        .collect::<Vec<_>>();
    if observed.as_slice() != expected {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("determinism checkpoints are {observed:?}; expected {expected:?}"),
        ));
    }

    let directory = perf_output_directory(config);
    std::fs::create_dir_all(&directory)?;
    let determinism_path = directory.join("determinism.csv");
    let actor_records_path = directory.join("determinism_records.csv");
    if determinism_path.exists()
        || actor_records_path.exists()
        || directory.join("frames.csv").exists()
        || directory.join("summary.csv").exists()
        || directory.join("scene_roots.csv").exists()
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("perf output already exists in {}", directory.display()),
        ));
    }

    let mut csv = String::from(concat!(
        "schema_version,checkpoint,update_tick,fixed_timestep_ns,virtual_delta_ns,",
        "virtual_elapsed_ns,fixed_delta_ns,fixed_elapsed_ns,fixed_overstep_ns,virtual_paused,",
        "virtual_relative_speed_bits,virtual_effective_speed_bits,souls,familiars,designations,",
        "state_checksum\n"
    ));
    for checkpoint in checkpoints {
        csv.push_str(&format!(
            "1,{},{},{},{},{},{},{},{},{},{:016x},{:016x},{},{},{},{:016x}\n",
            checkpoint.checkpoint,
            checkpoint.update_tick,
            checkpoint.fixed_timestep_ns,
            checkpoint.virtual_delta_ns,
            checkpoint.virtual_elapsed_ns,
            checkpoint.fixed_delta_ns,
            checkpoint.fixed_elapsed_ns,
            checkpoint.fixed_overstep_ns,
            u8::from(checkpoint.virtual_paused),
            checkpoint.virtual_relative_speed_bits,
            checkpoint.virtual_effective_speed_bits,
            checkpoint.checksum.souls,
            checkpoint.checksum.familiars,
            checkpoint.checksum.designations,
            checkpoint.checksum.value,
        ));
    }
    std::fs::write(&determinism_path, csv)?;

    let mut records_csv =
        String::from("schema_version,checkpoint,update_tick,actor_kind,actor_key,record_hex\n");
    for record in actor_records {
        records_csv.push_str(&format!(
            "1,{},{},{},{},{}\n",
            record.checkpoint,
            record.update_tick,
            record.actor_kind,
            record.actor_key,
            encode_hex(&record.record),
        ));
    }
    std::fs::write(&actor_records_path, records_csv)?;
    eprintln!(
        "PERF_DETERMINISM_AUDIT: wrote {} checkpoints and {} actor records to {}",
        checkpoints.len(),
        actor_records.len(),
        directory.display(),
    );
    Ok(())
}

#[cfg(feature = "profiling")]
fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(feature = "profiling")]
fn percentile_summary(samples: &[f64]) -> (f64, f64, f64) {
    if samples.is_empty() {
        return (0.0, 0.0, 0.0);
    }

    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    let percentile = |ratio: f64| {
        let index = ((sorted.len() - 1) as f64 * ratio).round() as usize;
        sorted[index]
    };
    (percentile(0.50), percentile(0.95), percentile(0.99))
}

#[cfg(feature = "profiling")]
pub(super) const fn fnv1a(current: u64, value: u64) -> u64 {
    let mut hash = current;
    let bytes = value.to_le_bytes();
    let mut index = 0;
    while index < bytes.len() {
        hash ^= bytes[index] as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        index += 1;
    }
    hash
}

#[cfg(feature = "profiling")]
pub(super) fn fnv1a_bytes(mut current: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        current ^= *byte as u64;
        current = current.wrapping_mul(0x0000_0100_0000_01b3);
    }
    current
}
