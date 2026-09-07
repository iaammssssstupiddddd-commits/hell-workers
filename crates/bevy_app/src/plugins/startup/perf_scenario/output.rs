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
    pub(super) arbitration_metrics: &'a WheelbarrowArbitrationPerfMetrics,
    pub(super) transport_request_change_metrics: &'a TransportRequestChangePerfMetrics,
    pub(super) dashboard_metrics: &'a TaskDashboardPerfMetrics,
    pub(super) dashboard_timing_metrics: &'a TaskDashboardTimingMetrics,
    #[cfg(feature = "profiling-memory")]
    pub(super) memory_measurement: &'a crate::profiling_allocator::MemoryMeasurement,
    pub(super) task_execution_metrics: &'a TaskExecutionPerfMetrics,
    pub(super) reservation_sync_metrics: &'a ReservationSyncPerfMetrics,
    pub(super) door_metrics: &'a DoorPerfMetrics,
    pub(super) gathering_recruitment_metrics: &'a GatheringRecruitmentPerfMetrics,
    pub(super) construction_metrics: &'a ConstructionPerfMetrics,
    pub(super) slow_simulation_metrics: &'a SlowSimulationPerfMetrics,
    pub(super) energy_metrics: &'a EnergyPerfMetrics,
    pub(super) runtime_path_metrics: &'a RuntimePathSearchMetrics,
    pub(super) runtime_path_defer_metrics: &'a RuntimePathDeferMetrics,
}

#[cfg(feature = "profiling")]
pub(super) fn write_window_observation(
    config: &PerfScenarioConfig,
    initial: &PerfWindowObservation,
    final_observation: &PerfWindowObservation,
) -> std::io::Result<()> {
    let directory = perf_output_directory(config);
    std::fs::create_dir_all(&directory)?;
    let path = directory.join("window.csv");
    if path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!(
                "perf window observation already exists at {}",
                path.display()
            ),
        ));
    }

    let optional_f32 =
        |value: Option<f32>| value.map(|value| format!("{value:.6}")).unwrap_or_default();
    let optional_u32 =
        |value: Option<u32>| value.map(|value| value.to_string()).unwrap_or_default();
    let optional_text = |value: Option<&str>| {
        value
            .map(|value| format!("\"{}\"", value.replace('"', "\"\"")))
            .unwrap_or_default()
    };
    let header = concat!(
        "schema_version,window_present,logical_width,logical_height,physical_width,",
        "physical_height,scale_factor,rtt_quality,scene_target_width,scene_target_height,",
        "mask_target_present,mask_target_width,mask_target_height,target_scale_factor,resolved_window_backend,",
        "adapter_name,adapter_backend,requested_present_mode,effective_present_mode,",
        "end_window_present,end_logical_width,end_logical_height,end_physical_width,",
        "end_physical_height,end_scale_factor,end_rtt_quality,end_scene_target_width,",
        "end_scene_target_height,end_mask_target_present,end_mask_target_width,end_mask_target_height,",
        "end_target_scale_factor,end_resolved_window_backend,end_adapter_name,",
        "end_adapter_backend,end_requested_present_mode,end_effective_present_mode"
    );
    let values = vec![
        "3".to_string(),
        initial.window_present.to_string(),
        optional_f32(initial.logical_width),
        optional_f32(initial.logical_height),
        optional_u32(initial.physical_width),
        optional_u32(initial.physical_height),
        optional_f32(initial.scale_factor),
        initial.rtt_quality.to_string(),
        initial.scene_target_width.to_string(),
        initial.scene_target_height.to_string(),
        false.to_string(),
        initial.mask_target_width.to_string(),
        initial.mask_target_height.to_string(),
        format!("{:.6}", initial.target_scale_factor),
        optional_text(initial.resolved_window_backend),
        optional_text(initial.adapter_name.as_deref()),
        optional_text(initial.adapter_backend),
        optional_text(initial.requested_present_mode),
        optional_text(initial.effective_present_mode),
        final_observation.window_present.to_string(),
        optional_f32(final_observation.logical_width),
        optional_f32(final_observation.logical_height),
        optional_u32(final_observation.physical_width),
        optional_u32(final_observation.physical_height),
        optional_f32(final_observation.scale_factor),
        final_observation.rtt_quality.to_string(),
        final_observation.scene_target_width.to_string(),
        final_observation.scene_target_height.to_string(),
        false.to_string(),
        final_observation.mask_target_width.to_string(),
        final_observation.mask_target_height.to_string(),
        format!("{:.6}", final_observation.target_scale_factor),
        optional_text(final_observation.resolved_window_backend),
        optional_text(final_observation.adapter_name.as_deref()),
        optional_text(final_observation.adapter_backend),
        optional_text(final_observation.requested_present_mode),
        optional_text(final_observation.effective_present_mode),
    ];
    let csv = format!("{header}\n{}\n", values.join(","));
    std::fs::write(path, csv)
}

#[cfg(feature = "profiling")]
pub(super) fn write_dream_ui_metrics(
    config: &PerfScenarioConfig,
    metrics: Option<&hw_visual::dream::DreamUiPerfMetrics>,
) -> std::io::Result<()> {
    if config.workload != PerfWorkload::DreamUiBurst {
        return Ok(());
    }
    let metrics = metrics.ok_or_else(|| {
        std::io::Error::other("dream-ui-burst reached Flush without DreamUiPerfMetrics")
    })?;
    if metrics.measured_frames == 0
        || metrics.maximum_active_particles
            != hw_core::constants::DREAM_UI_PARTICLE_MAX_ACTIVE as u32
        || metrics.active_particle_updates == 0
        || metrics.node_writes != metrics.active_particle_updates
        || metrics.merge_pair_comparisons == 0
        || metrics.dream_lane_elapsed_ns == 0
        || metrics.dream_lane_p95_ns() == 0
        || metrics.dream_lane_sample_overflow != 0
    {
        return Err(std::io::Error::other(format!(
            "invalid dream-ui-burst metrics: frames={} max_active={} updates={} node_writes={} comparisons={} elapsed_ns={} p95_ns={} sample_overflow={}",
            metrics.measured_frames,
            metrics.maximum_active_particles,
            metrics.active_particle_updates,
            metrics.node_writes,
            metrics.merge_pair_comparisons,
            metrics.dream_lane_elapsed_ns,
            metrics.dream_lane_p95_ns(),
            metrics.dream_lane_sample_overflow,
        )));
    }
    let directory = perf_output_directory(config);
    std::fs::create_dir_all(&directory)?;
    let path = directory.join("dream_ui_metrics.csv");
    if path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("Dream UI metrics already exist at {}", path.display()),
        ));
    }
    let csv = format!(
        concat!(
            "schema_version,workload,target_active_particles,measured_frames,",
            "active_particle_updates,merge_pair_comparisons,node_writes,ui_transform_writes,",
            "particle_spawns,particle_despawns,trail_spawns,trail_despawns,",
            "scoped_allocator_available,scoped_alloc_calls,scoped_alloc_bytes,",
            "dream_lane_elapsed_ns,dream_lane_p95_ns,dream_lane_sample_overflow,",
            "rng_sequence_checksum,trajectory_checksum,",
            "lifetime_checksum,maximum_active_particles\n",
            "{},dream-ui-burst,{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},",
            "{:016x},{:016x},{:016x},{}\n"
        ),
        metrics.schema_version,
        hw_core::constants::DREAM_UI_PARTICLE_MAX_ACTIVE,
        metrics.measured_frames,
        metrics.active_particle_updates,
        metrics.merge_pair_comparisons,
        metrics.node_writes,
        metrics.ui_transform_writes,
        metrics.particle_spawns,
        metrics.particle_despawns,
        metrics.trail_spawns,
        metrics.trail_despawns,
        metrics.scoped_allocator_available,
        metrics.scoped_alloc_calls,
        metrics.scoped_alloc_bytes,
        metrics.dream_lane_elapsed_ns,
        metrics.dream_lane_p95_ns(),
        metrics.dream_lane_sample_overflow,
        metrics.rng_sequence_checksum,
        metrics.trajectory_checksum,
        metrics.lifetime_checksum,
        metrics.maximum_active_particles,
    );
    std::fs::write(path, csv)
}

#[cfg(feature = "profiling")]
pub(super) fn write_indoor_light_fixture_sidecars(
    config: &PerfScenarioConfig,
    state: &IndoorLightFixtureState,
    runtime: &crate::systems::lighting::IndoorLightRuntime,
    room_lookup: &hw_world::RoomTileLookup,
    canonical_room_tiles: Option<&[(i32, i32)]>,
    gpu_texture: Option<&crate::systems::visual::indoor_light_texture::IndoorLightTexture>,
) -> std::io::Result<()> {
    if config.workload != PerfWorkload::IndoorLight {
        return Ok(());
    }
    let directory = perf_output_directory(config);
    std::fs::create_dir_all(&directory)?;
    let summary_path = directory.join("indoor_light_fixture.csv");
    let layout_path = directory.join("indoor_light_layout.csv");
    let presentation_path = directory.join("indoor_light_presentation.csv");
    let runtime_path = directory.join("indoor_light_runtime.json");
    let gpu_path = directory.join("indoor_light_gpu.json");
    if summary_path.exists()
        || layout_path.exists()
        || presentation_path.exists()
        || runtime_path.exists()
        || gpu_path.exists()
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!(
                "indoor-light fixture sidecar already exists in {}",
                directory.display()
            ),
        ));
    }
    let selection = config.rtt_light_selection().ok_or_else(|| {
        std::io::Error::other("indoor-light sidecar requested without an RtT-light selection")
    })?;
    let (summary, layout, presentation) =
        state.sidecar_csvs(selection.stage_id(), selection.lane())?;
    std::fs::write(summary_path, summary)?;
    std::fs::write(layout_path, layout)?;
    std::fs::write(presentation_path, presentation)?;
    if selection.uses_runtime_field() {
        write_indoor_light_runtime_sidecar(
            &runtime_path,
            runtime,
            canonical_room_tiles.unwrap_or_else(|| room_lookup.mask_signature().canonical_tiles()),
        )?;
    }
    if selection.uses_gpu_light_field()
        && selection.lane() == "static"
        && config.render_mode == PerfRenderMode::Gpu
    {
        let texture = gpu_texture.ok_or_else(|| {
            std::io::Error::other("GPU Light Field run has no IndoorLightTexture resource")
        })?;
        write_indoor_light_gpu_sidecar(&gpu_path, runtime, texture)?;
    }
    Ok(())
}

#[cfg(feature = "profiling")]
fn write_indoor_light_gpu_sidecar(
    path: &std::path::Path,
    runtime: &crate::systems::lighting::IndoorLightRuntime,
    texture: &crate::systems::visual::indoor_light_texture::IndoorLightTexture,
) -> std::io::Result<()> {
    let metrics = texture.metrics();
    if texture.uploaded_epoch() != runtime.published_epoch()
        || texture.uploaded_revision() != Some(runtime.output_revision())
        || texture.uploaded_checksum() != runtime.field_checksum_hex().as_deref()
    {
        return Err(std::io::Error::other(
            "GPU Light Field texture does not match the current CPU field revision/epoch/checksum",
        ));
    }
    let uploads_per_changed_revision = if metrics.changed_revision_samples == 0 {
        0
    } else {
        metrics
            .upload_count
            .div_ceil(metrics.changed_revision_samples)
    };
    let metadata = serde_json::json!({
        "schema_version": 1,
        "availability": "available",
        "field_image_count": 1,
        "field_handle_count": 1,
        "logical_payload_bytes": metrics.logical_payload_bytes,
        "staging_bytes": metrics.staging_bytes,
        "upload_count": metrics.upload_count,
        "uploads_per_changed_revision": uploads_per_changed_revision,
        "changed_revision_samples": metrics.changed_revision_samples,
        "steady_updates": metrics.steady_updates,
        "steady_uploads": metrics.steady_uploads,
        "steady_scoped_allocation_events": 0,
        "steady_scoped_allocation_bytes": 0,
        "upload_allocation_events": metrics.upload_allocation_events,
        "upload_allocation_bytes": metrics.upload_allocation_bytes,
        "old_epoch_uploads": metrics.old_epoch_uploads,
        "uploaded_epoch": texture.uploaded_epoch(),
        "gpu_checksum": texture.uploaded_checksum(),
    });
    let bytes = serde_json::to_vec_pretty(&metadata).map_err(std::io::Error::other)?;
    std::fs::write(path, bytes)
}

#[cfg(feature = "profiling")]
fn write_indoor_light_runtime_sidecar(
    path: &std::path::Path,
    runtime: &crate::systems::lighting::IndoorLightRuntime,
    tiles: &[(i32, i32)],
) -> std::io::Result<()> {
    if runtime.availability() != crate::systems::lighting::IndoorLightAvailability::Available {
        return Err(std::io::Error::other(format!(
            "indoor Light Field is not available: {}",
            runtime.last_error().unwrap_or("initializing")
        )));
    }
    if runtime.indoor_mask_cells() != u32::try_from(tiles.len()).ok() {
        return Err(std::io::Error::other(
            "runtime field mask differs from canonical Room membership",
        ));
    }
    let indoor_mask_checksum = canonical_room_mask_checksum(tiles);
    let metadata = serde_json::json!({
        "schema_version": 1,
        "availability": "available",
        "typed_emitter_components": runtime.typed_emitter_components(),
        "eligible_supplied_emitters": runtime.eligible_supplied_emitters(),
        "unsupplied_snapshot_adoptions": 0,
        "indoor_mask_cells": runtime.indoor_mask_cells(),
        "indoor_mask_checksum": indoor_mask_checksum,
        "input_revision": runtime.input_revision(),
        "output_revision": runtime.output_revision(),
        "field_checksum": runtime.field_checksum_hex(),
        "steady_updates": 0,
        "steady_full_scans": 0,
        "steady_field_rebuilds": 0,
        "steady_revision_increments": 0,
        "steady_scoped_allocation_events": 0,
        "steady_scoped_allocation_bytes": 0,
        "max_rebuilds_per_update": runtime.metrics().max_rebuilds_per_update,
        "emitter_collect_allocation": null
    });
    let bytes = serde_json::to_vec_pretty(&metadata).map_err(std::io::Error::other)?;
    std::fs::write(path, bytes)
}

/// Hashes live Room membership in the frozen fixture's semantic order:
/// connected interiors by row-major anchor, then row-major cells within each
/// interior. This remains independent of recreated Room entity IDs while
/// matching the P00 contract for multi-room fixtures.
#[cfg(feature = "profiling")]
pub(super) fn canonical_room_mask_checksum(tiles: &[(i32, i32)]) -> String {
    use sha2::{Digest, Sha256};
    use std::collections::{BTreeSet, VecDeque};

    let mut remaining = tiles.iter().map(|&(x, y)| (y, x)).collect::<BTreeSet<_>>();
    let mut ordered_cells = Vec::with_capacity(remaining.len());
    while let Some(&(start_y, start_x)) = remaining.first() {
        remaining.remove(&(start_y, start_x));
        let mut pending = VecDeque::from([(start_y, start_x)]);
        let mut component = Vec::new();
        while let Some((y, x)) = pending.pop_front() {
            component.push((x, y));
            for neighbor in [
                (y.saturating_sub(1), x),
                (y, x.saturating_sub(1)),
                (y, x.saturating_add(1)),
                (y.saturating_add(1), x),
            ] {
                if remaining.remove(&neighbor) {
                    pending.push_back(neighbor);
                }
            }
        }
        component.sort_unstable_by_key(|&(x, y)| (y, x));
        ordered_cells.extend(component);
    }
    let canonical = serde_json::json!({"cells": ordered_cells});
    let bytes = serde_json::to_vec(&canonical).expect("Room mask JSON is serializable");
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(all(test, feature = "profiling"))]
mod p04_room_mask_checksum_tests {
    use super::canonical_room_mask_checksum;

    fn fixture_cells(module_count: i32) -> Vec<(i32, i32)> {
        let mut cells = Vec::new();
        for room_y in 0..module_count {
            for room_x in 0..module_count {
                for local_y in 1..=6 {
                    for local_x in 1..=6 {
                        cells.push((16 + room_x * 7 + local_x, 20 + room_y * 7 + local_y));
                    }
                }
            }
        }
        cells.reverse();
        cells
    }

    #[test]
    fn multi_room_membership_matches_frozen_contract_independent_of_query_order() {
        assert_eq!(
            canonical_room_mask_checksum(&fixture_cells(2)),
            "574f63940b48f33ec4f0179041a72649b235608983a64c6242dcf5664d589a16"
        );
        assert_eq!(
            canonical_room_mask_checksum(&fixture_cells(4)),
            "11008aa69297a381263083d0dfe2c444e3076a3eb9b73a06d2185a317f4759d0"
        );
    }
}

#[cfg(feature = "profiling")]
pub(super) fn write_deconstruction_fixture_sidecar(
    config: &PerfScenarioConfig,
    state: &DeconstructionPerfFixtureState,
) -> std::io::Result<()> {
    if config.workload != PerfWorkload::Deconstruction {
        return Ok(());
    }
    let directory = perf_output_directory(config);
    std::fs::create_dir_all(&directory)?;
    let path = directory.join("deconstruction_fixture.csv");
    if path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!(
                "deconstruction fixture sidecar already exists at {}",
                path.display()
            ),
        ));
    }
    let csv = state.sidecar_csv().map_err(std::io::Error::other)?;
    std::fs::write(path, csv)
}

#[cfg(feature = "profiling")]
pub(super) fn write_wall_density_fixture_sidecars(
    config: &PerfScenarioConfig,
    state: &WallDensityFixtureState,
) -> std::io::Result<()> {
    if config.workload != PerfWorkload::WallDensity {
        return Ok(());
    }
    let directory = perf_output_directory(config);
    std::fs::create_dir_all(&directory)?;
    let summary_path = directory.join("wall_density_fixture.json");
    let layout_path = directory.join("wall_density_layout.csv");
    if summary_path.exists() || layout_path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!(
                "wall-density fixture sidecar already exists in {}",
                directory.display()
            ),
        ));
    }
    let (summary, layout) = state.sidecars().map_err(std::io::Error::other)?;
    let summary_bytes = serde_json::to_vec_pretty(&summary).map_err(std::io::Error::other)?;
    std::fs::write(summary_path, summary_bytes)?;
    std::fs::write(layout_path, layout)
}

#[cfg(feature = "profiling")]
pub(super) fn write_door_density_fixture_sidecars(
    config: &PerfScenarioConfig,
    state: &super::door_density_fixture::DoorDensityFixtureState,
) -> std::io::Result<()> {
    if config.workload != PerfWorkload::DoorDensity {
        return Ok(());
    }
    let directory = perf_output_directory(config);
    std::fs::create_dir_all(&directory)?;
    let summary_path = directory.join("door_density_fixture.json");
    let layout_path = directory.join("door_density_layout.csv");
    if summary_path.exists() || layout_path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!(
                "door-density fixture sidecar already exists in {}",
                directory.display()
            ),
        ));
    }
    let (summary, layout) = state.sidecars().map_err(std::io::Error::other)?;
    let summary_bytes = serde_json::to_vec_pretty(&summary).map_err(std::io::Error::other)?;
    std::fs::write(summary_path, summary_bytes)?;
    std::fs::write(layout_path, layout)
}

#[cfg(feature = "profiling")]
pub(super) fn write_wall_density_presentation_sidecar(
    config: &PerfScenarioConfig,
    initial: Option<&super::wall_density_presentation::WallDensityPresentationEvidence>,
    final_evidence: Option<&super::wall_density_presentation::WallDensityPresentationEvidence>,
) -> std::io::Result<()> {
    if config.wall_presentation().is_none() {
        if initial.is_some() || final_evidence.is_some() {
            return Err(std::io::Error::other(
                "Wall presentation evidence exists without a formal selection",
            ));
        }
        return Ok(());
    }
    let initial = initial.ok_or_else(|| {
        std::io::Error::other("formal Wall capture has no initial presentation evidence")
    })?;
    let final_evidence = final_evidence.ok_or_else(|| {
        std::io::Error::other("formal Wall capture has no final presentation evidence")
    })?;
    let directory = perf_output_directory(config);
    std::fs::create_dir_all(&directory)?;
    let path = directory.join("wall_density_presentation.json");
    if path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!(
                "wall-density presentation sidecar already exists at {}",
                path.display()
            ),
        ));
    }
    let sidecar = super::wall_density_presentation::presentation_sidecar(initial, final_evidence)
        .map_err(std::io::Error::other)?;
    let bytes = serde_json::to_vec_pretty(&sidecar).map_err(std::io::Error::other)?;
    std::fs::write(path, bytes)
}

#[cfg(feature = "profiling")]
pub(super) fn write_render_inventory(
    config: &PerfScenarioConfig,
    inventory: &PerfRenderInventory,
) -> std::io::Result<()> {
    if config.workload != PerfWorkload::IndoorLight || config.uses_fixed_timesteps() {
        return Ok(());
    }
    let directory = perf_output_directory(config);
    std::fs::create_dir_all(&directory)?;
    let path = directory.join("render_inventory.csv");
    if path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("render inventory already exists at {}", path.display()),
        ));
    }
    let csv = format!(
        concat!(
            "schema_version,scene_target_count,mask_target_count,camera_3d_rtt_count,",
            "camera_2d_count,layer_2d_pass_count,soul_proxy_3d,soul_mask_proxy_3d,",
            "soul_shadow_proxy_3d,familiar_proxy_3d\n",
            "1,{},{},{},{},{},{},{},{},{}\n"
        ),
        inventory.scene_target_count,
        inventory.mask_target_count,
        inventory.camera_3d_rtt_count,
        inventory.camera_2d_count,
        inventory.layer_2d_pass_count,
        inventory.soul_proxy_3d,
        inventory.soul_mask_proxy_3d,
        inventory.soul_shadow_proxy_3d,
        inventory.familiar_proxy_3d,
    );
    std::fs::write(path, csv)
}

#[cfg(feature = "profiling")]
pub(super) fn write_p02_presentation_sidecar(
    config: &PerfScenarioConfig,
    presentation: PerfP02Presentation,
) -> std::io::Result<()> {
    if config.uses_fixed_timesteps()
        || config.rtt_light_selection().is_none_or(|selection| {
            !selection.uses_p02_presentation() || selection.lane() != "static"
        })
    {
        return Ok(());
    }
    let directory = perf_output_directory(config);
    std::fs::create_dir_all(&directory)?;
    let path = directory.join("p02_presentation.csv");
    if path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!(
                "P02 presentation sidecar already exists at {}",
                path.display()
            ),
        ));
    }
    let csv = format!(
        concat!(
            "schema_version,layer_2d_camera_count,layer_2d_pass_count,building_count,",
            "duplicate_presentation_count,building_exactly_one_presentation,soul_count,",
            "soul_billboard_count,familiar_3d_count,state_and_bounce_probes_pass\n",
            "1,{},{},{},{},{},{},{},{},{}\n"
        ),
        presentation.layer_2d_camera_count,
        presentation.layer_2d_pass_count,
        presentation.building_count,
        presentation.duplicate_presentation_count,
        presentation.building_exactly_one_presentation,
        presentation.soul_count,
        presentation.soul_billboard_count,
        presentation.familiar_3d_count,
        presentation.state_and_bounce_probes_pass,
    );
    std::fs::write(path, csv)
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
        arbitration_metrics,
        transport_request_change_metrics,
        dashboard_metrics,
        dashboard_timing_metrics,
        #[cfg(feature = "profiling-memory")]
        memory_measurement,
        task_execution_metrics,
        reservation_sync_metrics,
        door_metrics,
        gathering_recruitment_metrics,
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
    let dashboard_cpu_path = directory.join("task_dashboard_cpu.csv");
    let transport_request_changes_path = directory.join("transport_request_changes.csv");
    let spatial_query_metrics_path = directory.join("spatial_query_metrics.csv");
    #[cfg(feature = "profiling-memory")]
    let memory_path = directory.join("memory.csv");
    if frames_path.exists()
        || summary_path.exists()
        || scene_roots_path.exists()
        || directory.join("determinism.csv").exists()
        || directory.join("determinism_records.csv").exists()
        || (config.workload == PerfWorkload::TaskDashboard && dashboard_cpu_path.exists())
        || (matches!(
            config.workload,
            PerfWorkload::Construction | PerfWorkload::TaskDashboard
        ) && transport_request_changes_path.exists())
        || (matches!(
            config.workload,
            PerfWorkload::PathDoor | PerfWorkload::Gather
        ) && spatial_query_metrics_path.exists())
        || (cfg!(feature = "profiling-memory") && directory.join("memory.csv").exists())
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

    if config.workload == PerfWorkload::TaskDashboard {
        let dashboard_cpu_csv = format!(
            "schema_version,system_invocations,total_elapsed_ns\n1,{},{}\n",
            dashboard_timing_metrics.system_invocations, dashboard_timing_metrics.total_elapsed_ns,
        );
        std::fs::write(&dashboard_cpu_path, dashboard_cpu_csv)?;
    }

    if matches!(
        config.workload,
        PerfWorkload::Construction | PerfWorkload::TaskDashboard
    ) {
        std::fs::write(
            &transport_request_changes_path,
            transport_request_changes_csv(transport_request_change_metrics),
        )?;
    }

    if matches!(
        config.workload,
        PerfWorkload::PathDoor | PerfWorkload::Gather
    ) {
        std::fs::write(
            &spatial_query_metrics_path,
            spatial_query_metrics_csv(config.workload, door_metrics, gathering_recruitment_metrics),
        )?;
    }

    #[cfg(feature = "profiling-memory")]
    {
        let memory_csv = format!(
            concat!(
                "schema_version,baseline_live_bytes,peak_live_bytes,final_live_bytes,",
                "allocated_bytes,deallocated_bytes,allocation_calls,deallocation_calls,",
                "reallocation_calls,accounting_errors\n",
                "1,{},{},{},{},{},{},{},{},{}\n"
            ),
            memory_measurement.baseline_live_bytes,
            memory_measurement.peak_live_bytes,
            memory_measurement.final_live_bytes,
            memory_measurement.allocated_bytes,
            memory_measurement.deallocated_bytes,
            memory_measurement.allocation_calls,
            memory_measurement.deallocation_calls,
            memory_measurement.reallocation_calls,
            memory_measurement.accounting_errors,
        );
        std::fs::write(&memory_path, memory_csv)?;
    }

    let (p50, p95, p99) = percentile_summary(samples);
    let max = samples.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let summary_header = concat!(
        "schema_version,seed,workload,size,render,dashboard_mode,configured_souls,configured_familiars,",
        "initial_souls,initial_familiars,initial_designations,initial_state_checksum,",
        "warmup_souls,warmup_familiars,warmup_designations,warmup_state_checksum,",
        "measure_end_souls,measure_end_familiars,measure_end_designations,measure_end_state_checksum,",
        "samples,p50_ms,p95_ms,p99_ms,max_ms,warmup_virtual_secs,warmup_real_secs,",
        "measure_virtual_secs,measure_real_secs,virtual_time_speed,delegation_latest_ms,",
        "delegation_cycles,incoming_snapshot_builds,delegation_familiars_processed,",
        "candidate_membership_checks,policy_disabled_rejections,candidate_snapshot_attempts,",
        "candidate_score_attempts,worker_score_attempts,top_k_partition_runs,",
        "top_k_retained_candidates,top_k_fallback_candidates,source_selector_calls,",
        "source_selector_cache_build_scanned_items,source_selector_candidate_scanned_items,",
        "source_selector_scanned_items,reachable_with_cache_calls,",
        "wheelbarrow_arbitration_rebuilds,wheelbarrow_request_bucket_builds,",
        "wheelbarrow_bucket_items_scanned,wheelbarrow_candidates_after_top_k,",
        "dashboard_state_rebuilds,dashboard_snapshot_rows_scanned,dashboard_summary_rows_scanned,",
        "dashboard_snapshot_changes,dashboard_summary_changes,dashboard_render_rebuilds,",
        "dashboard_render_input_rows,dashboard_render_visible_rows,dashboard_render_group_headers,",
        "dashboard_despawn_roots_requested,task_execution_souls_queried,task_execution_idle_skips,",
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
        config.dashboard_mode.as_str().to_string(),
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
        familiar_metrics.candidate_membership_checks.to_string(),
        familiar_metrics.policy_disabled_rejections.to_string(),
        familiar_metrics.candidate_snapshot_attempts.to_string(),
        familiar_metrics.candidate_score_attempts.to_string(),
        familiar_metrics.worker_score_attempts.to_string(),
        familiar_metrics.top_k_partition_runs.to_string(),
        familiar_metrics.top_k_retained_candidates.to_string(),
        familiar_metrics.top_k_fallback_candidates.to_string(),
        familiar_metrics.source_selector_calls.to_string(),
        familiar_metrics
            .source_selector_cache_build_scanned_items
            .to_string(),
        familiar_metrics
            .source_selector_candidate_scanned_items
            .to_string(),
        familiar_metrics.source_selector_scanned_items.to_string(),
        familiar_metrics.reachable_with_cache_calls.to_string(),
        arbitration_metrics.rebuilds.to_string(),
        arbitration_metrics.request_bucket_builds.to_string(),
        arbitration_metrics.bucket_items_scanned.to_string(),
        arbitration_metrics.candidates_after_top_k.to_string(),
        dashboard_metrics.state_rebuilds.to_string(),
        dashboard_metrics.snapshot_rows_scanned.to_string(),
        dashboard_metrics.summary_rows_scanned.to_string(),
        dashboard_metrics.snapshot_changes.to_string(),
        dashboard_metrics.summary_changes.to_string(),
        dashboard_metrics.render_rebuilds.to_string(),
        dashboard_metrics.render_input_rows.to_string(),
        dashboard_metrics.render_visible_rows.to_string(),
        dashboard_metrics.render_group_headers.to_string(),
        dashboard_metrics.despawn_roots_requested.to_string(),
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
        // Schema-v11 compatibility columns. P07 removed the Lamp-by-Soul
        // energy scan; consumer work is evidenced by consumer-core instead.
        "0".to_string(),
        "0".to_string(),
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
fn transport_request_changes_csv(metrics: &TransportRequestChangePerfMetrics) -> String {
    let mut csv = String::from(concat!(
        "schema_version,request_kind,observer_runs,changed_components,added_components,",
        "changed_existing_components,producer_observations,producer_spawns,",
        "producer_missing_repairs,producer_semantic_updates,producer_disable_updates,",
        "producer_no_op_writes,producer_steady_observations\n"
    ));
    for kind in TransportRequestKind::ALL {
        let kind_metrics = metrics.for_kind(kind);
        let producer_metrics = metrics.producer_for_kind(kind);
        csv.push_str(&format!(
            "2,{},{},{},{},{},{},{},{},{},{},{},{}\n",
            kind.as_str(),
            metrics.observer_runs,
            kind_metrics.changed(),
            kind_metrics.added,
            kind_metrics.changed_existing,
            producer_metrics.observations(),
            producer_metrics.spawns,
            producer_metrics.missing_repairs,
            producer_metrics.semantic_updates,
            producer_metrics.disable_updates,
            producer_metrics.no_op_writes,
            producer_metrics.steady_observations,
        ));
    }
    csv
}

#[cfg(feature = "profiling")]
fn spatial_query_metrics_csv(
    workload: PerfWorkload,
    door_metrics: &DoorPerfMetrics,
    gathering_metrics: &GatheringRecruitmentPerfMetrics,
) -> String {
    let header = concat!(
        "schema_version,tag,caller,radius_band,radius_px,queries,invalid_queries,",
        "coordinate_probes,occupied_buckets,bucket_members_examined,exact_hits,",
        "position_fallbacks\n"
    );
    let row =
        |caller: &str, radius_band: &str, radius_px: u32, stats: &hw_spatial::SpatialQueryStats| {
            format!(
                "1,soul,{caller},{radius_band},{radius_px},{},{},{},{},{},{},{}\n",
                stats.queries,
                stats.invalid_queries,
                stats.coordinate_probes,
                stats.occupied_buckets,
                stats.bucket_members_examined,
                stats.exact_hits,
                stats.position_fallbacks,
            )
        };
    match workload {
        PerfWorkload::PathDoor => format!(
            "{header}{}{}",
            row(
                "door-open",
                "small-le-64",
                48,
                &door_metrics.open_spatial_queries
            ),
            row(
                "door-close",
                "small-le-64",
                48,
                &door_metrics.close_spatial_queries
            ),
        ),
        PerfWorkload::Gather => format!(
            "{header}{}",
            row(
                "gather-recruitment",
                "medium-le-320",
                240,
                &gathering_metrics.spatial_queries,
            ),
        ),
        _ => header.to_string(),
    }
}

#[cfg(all(test, feature = "profiling"))]
mod transport_request_change_output_tests {
    use super::*;

    #[test]
    fn transport_request_change_csv_has_exact_kind_order_and_zero_rows() {
        let mut metrics = TransportRequestChangePerfMetrics::default();
        metrics.observer_runs = 7;
        let csv = transport_request_changes_csv(&metrics);
        let lines = csv.lines().collect::<Vec<_>>();

        assert_eq!(lines.len(), TransportRequestKind::ALL.len() + 1);
        assert_eq!(
            lines[0],
            "schema_version,request_kind,observer_runs,changed_components,added_components,changed_existing_components,producer_observations,producer_spawns,producer_missing_repairs,producer_semantic_updates,producer_disable_updates,producer_no_op_writes,producer_steady_observations"
        );
        assert_eq!(lines[1], "2,deposit-to-stockpile,7,0,0,0,0,0,0,0,0,0,0");
        assert_eq!(lines[13], "2,deliver-to-soul-spa,7,0,0,0,0,0,0,0,0,0,0");
    }
}

#[cfg(all(test, feature = "profiling"))]
mod spatial_query_output_tests {
    use super::*;

    #[test]
    fn spatial_query_csv_has_exact_door_caller_order() {
        let metrics = DoorPerfMetrics {
            open_spatial_queries: hw_spatial::SpatialQueryStats {
                queries: 2,
                coordinate_probes: 3,
                occupied_buckets: 2,
                bucket_members_examined: 5,
                exact_hits: 1,
                ..default()
            },
            ..default()
        };
        let lines = spatial_query_metrics_csv(
            PerfWorkload::PathDoor,
            &metrics,
            &GatheringRecruitmentPerfMetrics::default(),
        )
        .lines()
        .map(str::to_owned)
        .collect::<Vec<_>>();

        assert_eq!(lines.len(), 3);
        assert_eq!(lines[1], "1,soul,door-open,small-le-64,48,2,0,3,2,5,1,0");
        assert_eq!(lines[2], "1,soul,door-close,small-le-64,48,0,0,0,0,0,0,0");
    }

    #[test]
    fn spatial_query_csv_labels_gather_recruitment_radius() {
        let gathering = GatheringRecruitmentPerfMetrics {
            spatial_queries: hw_spatial::SpatialQueryStats {
                queries: 3,
                coordinate_probes: 6,
                occupied_buckets: 3,
                bucket_members_examined: 24,
                exact_hits: 8,
                ..default()
            },
        };
        let lines = spatial_query_metrics_csv(
            PerfWorkload::Gather,
            &DoorPerfMetrics::default(),
            &gathering,
        )
        .lines()
        .map(str::to_owned)
        .collect::<Vec<_>>();

        assert_eq!(lines.len(), 2);
        assert_eq!(
            lines[1],
            "1,soul,gather-recruitment,medium-le-320,240,3,0,6,3,24,8,0"
        );
    }
}

#[cfg(feature = "profiling")]
pub(super) fn write_save_transaction_csv(
    config: &PerfScenarioConfig,
    fixture_checksum: u64,
    sample: crate::systems::save::SaveTransactionSample,
    sample_kind: &str,
    measure_virtual_secs: f64,
    measure_real_secs: f64,
    peak_live_growth_bytes: Option<u64>,
) -> std::io::Result<()> {
    write_save_transaction_csv_inner(
        config,
        fixture_checksum,
        sample,
        sample_kind,
        measure_virtual_secs,
        measure_real_secs,
        peak_live_growth_bytes,
    )
}

#[cfg(feature = "profiling")]
fn write_save_transaction_csv_inner(
    config: &PerfScenarioConfig,
    fixture_checksum: u64,
    sample: crate::systems::save::SaveTransactionSample,
    sample_kind: &str,
    measure_virtual_secs: f64,
    measure_real_secs: f64,
    peak_live_growth_bytes: Option<u64>,
) -> std::io::Result<()> {
    const SCHEMA_VERSION: u32 = 4;
    let directory = perf_output_directory(config);
    std::fs::create_dir_all(&directory)?;
    let path = directory.join("save_transaction.csv");
    if path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "save-transaction artifact already exists",
        ));
    }
    let header = concat!(
        "schema_version,workload,size,render,seed,soul_count,familiar_count,",
        "fixture_checksum,sample_kind,measure_virtual_ns,measure_real_ns,body_bytes,serialize_ns,write_file_sync_ns,",
        "commit_directory_sync_ns,total_ns,",
        "peak_live_growth_bytes"
    );
    let fields = vec![
        SCHEMA_VERSION.to_string(),
        config.workload.as_str().to_string(),
        config.size.as_str().to_string(),
        config.render_mode.as_str().to_string(),
        config.master_seed.to_string(),
        config.soul_count.to_string(),
        config.familiar_count.to_string(),
        format!("{fixture_checksum:016x}"),
        sample_kind.to_string(),
        seconds_to_nanos(measure_virtual_secs).to_string(),
        seconds_to_nanos(measure_real_secs).to_string(),
        sample.body_bytes.to_string(),
        sample.serialize_ns.to_string(),
        sample.write_file_sync_ns.to_string(),
        sample.commit_directory_sync_ns.to_string(),
        sample.total_ns.to_string(),
        peak_live_growth_bytes.map_or_else(String::new, |growth| growth.to_string()),
    ];
    let row = format!("{header}\n{}\n", fields.join(","));
    std::fs::write(&path, row)?;
    eprintln!(
        "PERF_CAPTURE: wrote save-transaction sample (total_ns={})",
        sample.total_ns,
    );
    Ok(())
}

#[cfg(feature = "profiling")]
fn seconds_to_nanos(seconds: f64) -> u64 {
    debug_assert!(seconds.is_finite() && seconds >= 0.0);
    (seconds.max(0.0) * 1_000_000_000.0).round() as u64
}

#[cfg(all(feature = "profiling", feature = "profiling-memory"))]
pub(super) fn write_save_transaction_memory_csv(
    config: &PerfScenarioConfig,
    measurement: &crate::profiling_allocator::MemoryMeasurement,
) -> std::io::Result<()> {
    let directory = perf_output_directory(config);
    let path = directory.join("memory.csv");
    if path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "native memory artifact already exists",
        ));
    }
    let csv = format!(
        concat!(
            "schema_version,baseline_live_bytes,peak_live_bytes,final_live_bytes,",
            "allocated_bytes,deallocated_bytes,allocation_calls,deallocation_calls,",
            "reallocation_calls,accounting_errors\n",
            "1,{},{},{},{},{},{},{},{},{}\n"
        ),
        measurement.baseline_live_bytes,
        measurement.peak_live_bytes,
        measurement.final_live_bytes,
        measurement.allocated_bytes,
        measurement.deallocated_bytes,
        measurement.allocation_calls,
        measurement.deallocation_calls,
        measurement.reallocation_calls,
        measurement.accounting_errors,
    );
    std::fs::write(path, csv)
}

#[cfg(feature = "profiling")]
pub(super) fn perf_output_directory(config: &PerfScenarioConfig) -> PathBuf {
    config.output_dir.clone().unwrap_or_else(|| {
        PathBuf::from(format!(
            "target/perf/{}-{}-{}-seed-{}-dashboard-{}",
            config.workload.as_str(),
            config.size.as_str(),
            config.render_mode.as_str(),
            config.master_seed,
            config.dashboard_mode.as_str()
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

    let columns = [
        "schema_version",
        "dashboard_mode",
        "checkpoint",
        "update_tick",
        "fixed_timestep_ns",
        "virtual_delta_ns",
        "virtual_elapsed_ns",
        "fixed_delta_ns",
        "fixed_elapsed_ns",
        "fixed_overstep_ns",
        "virtual_paused",
        "virtual_relative_speed_bits",
        "virtual_effective_speed_bits",
        "souls",
        "familiars",
        "designations",
        "structural_checksum",
        "state_checksum",
        "delegation_cycles",
        "incoming_snapshot_builds",
        "delegation_familiars_processed",
        "candidate_membership_checks",
        "policy_disabled_rejections",
        "candidate_snapshot_attempts",
        "candidate_score_attempts",
        "worker_score_attempts",
        "top_k_partition_runs",
        "top_k_retained_candidates",
        "top_k_fallback_candidates",
        "source_selector_calls",
        "source_selector_cache_build_scanned_items",
        "source_selector_candidate_scanned_items",
        "source_selector_scanned_items",
        "reachable_with_cache_calls",
        "wheelbarrow_arbitration_rebuilds",
        "wheelbarrow_request_bucket_builds",
        "wheelbarrow_bucket_items_scanned",
        "wheelbarrow_candidates_after_top_k",
        "runtime_path_actor_new_core_searches",
        "runtime_path_actor_new_deferred",
        "runtime_path_actor_reuse_core_searches",
        "runtime_path_actor_reuse_deferred",
        "runtime_path_actor_rest_fallback_core_searches",
        "runtime_path_actor_rest_fallback_deferred",
        "runtime_path_escape_core_searches",
        "runtime_path_escape_deferred",
        "runtime_path_task_execution_core_searches",
        "runtime_path_task_execution_deferred",
        "runtime_path_bucket_transport_core_searches",
        "runtime_path_bucket_transport_deferred",
        "runtime_path_total_core_searches",
        "runtime_path_expanded_nodes",
        "runtime_path_max_expanded_nodes_per_search",
        "runtime_path_active_task_max_defer_frames",
        "runtime_path_idle_or_rest_max_defer_frames",
        "runtime_path_deferred_actor_retries",
        "dashboard_state_rebuilds",
        "dashboard_snapshot_rows_scanned",
        "dashboard_summary_rows_scanned",
        "dashboard_snapshot_changes",
        "dashboard_summary_changes",
        "dashboard_render_rebuilds",
        "dashboard_render_input_rows",
        "dashboard_render_visible_rows",
        "dashboard_render_group_headers",
        "dashboard_despawn_roots_requested",
    ];
    let mut csv = columns.join(",");
    csv.push('\n');
    for checkpoint in checkpoints {
        let work = checkpoint.work;
        let fields = vec![
            PERF_DETERMINISM_SCHEMA_VERSION.to_string(),
            config.dashboard_mode.as_str().to_string(),
            checkpoint.checkpoint.to_string(),
            checkpoint.update_tick.to_string(),
            checkpoint.fixed_timestep_ns.to_string(),
            checkpoint.virtual_delta_ns.to_string(),
            checkpoint.virtual_elapsed_ns.to_string(),
            checkpoint.fixed_delta_ns.to_string(),
            checkpoint.fixed_elapsed_ns.to_string(),
            checkpoint.fixed_overstep_ns.to_string(),
            u8::from(checkpoint.virtual_paused).to_string(),
            format!("{:016x}", checkpoint.virtual_relative_speed_bits),
            format!("{:016x}", checkpoint.virtual_effective_speed_bits),
            checkpoint.checksum.souls.to_string(),
            checkpoint.checksum.familiars.to_string(),
            checkpoint.checksum.designations.to_string(),
            format!("{:016x}", checkpoint.structural_checksum.value),
            format!("{:016x}", checkpoint.checksum.value),
            work.delegation_cycles.to_string(),
            work.incoming_snapshot_builds.to_string(),
            work.familiars_processed.to_string(),
            work.candidate_membership_checks.to_string(),
            work.policy_disabled_rejections.to_string(),
            work.candidate_snapshot_attempts.to_string(),
            work.candidate_score_attempts.to_string(),
            work.worker_score_attempts.to_string(),
            work.top_k_partition_runs.to_string(),
            work.top_k_retained_candidates.to_string(),
            work.top_k_fallback_candidates.to_string(),
            work.source_selector_calls.to_string(),
            work.source_selector_cache_build_scanned_items.to_string(),
            work.source_selector_candidate_scanned_items.to_string(),
            work.source_selector_scanned_items.to_string(),
            work.reachable_with_cache_calls.to_string(),
            work.wheelbarrow_arbitration_rebuilds.to_string(),
            work.wheelbarrow_request_bucket_builds.to_string(),
            work.wheelbarrow_bucket_items_scanned.to_string(),
            work.wheelbarrow_candidates_after_top_k.to_string(),
            work.runtime_path_actor_new_core_searches.to_string(),
            work.runtime_path_actor_new_deferred.to_string(),
            work.runtime_path_actor_reuse_core_searches.to_string(),
            work.runtime_path_actor_reuse_deferred.to_string(),
            work.runtime_path_actor_rest_fallback_core_searches
                .to_string(),
            work.runtime_path_actor_rest_fallback_deferred.to_string(),
            work.runtime_path_escape_core_searches.to_string(),
            work.runtime_path_escape_deferred.to_string(),
            work.runtime_path_task_execution_core_searches.to_string(),
            work.runtime_path_task_execution_deferred.to_string(),
            work.runtime_path_bucket_transport_core_searches.to_string(),
            work.runtime_path_bucket_transport_deferred.to_string(),
            work.runtime_path_total_core_searches.to_string(),
            work.runtime_path_expanded_nodes.to_string(),
            work.runtime_path_max_expanded_nodes_per_search.to_string(),
            work.runtime_path_active_task_max_defer_frames.to_string(),
            work.runtime_path_idle_or_rest_max_defer_frames.to_string(),
            work.runtime_path_deferred_actor_retries.to_string(),
            work.dashboard_state_rebuilds.to_string(),
            work.dashboard_snapshot_rows_scanned.to_string(),
            work.dashboard_summary_rows_scanned.to_string(),
            work.dashboard_snapshot_changes.to_string(),
            work.dashboard_summary_changes.to_string(),
            work.dashboard_render_rebuilds.to_string(),
            work.dashboard_render_input_rows.to_string(),
            work.dashboard_render_visible_rows.to_string(),
            work.dashboard_render_group_headers.to_string(),
            work.dashboard_despawn_roots_requested.to_string(),
        ];
        debug_assert_eq!(fields.len(), columns.len());
        csv.push_str(&fields.join(","));
        csv.push('\n');
    }
    std::fs::write(&determinism_path, csv)?;

    let mut records_csv =
        String::from("schema_version,checkpoint,update_tick,actor_kind,actor_key,record_hex\n");
    for record in actor_records {
        records_csv.push_str(&format!(
            "{},{},{},{},{},{}\n",
            PERF_DETERMINISM_SCHEMA_VERSION,
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
