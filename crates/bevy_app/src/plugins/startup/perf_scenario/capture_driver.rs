use super::*;

/// シナリオの初期状態を、ゲーム更新より前に固定して記録する。
///
/// `Update` の末尾で初期値を採ると、初回フレームのAIや移動が入り込み、
/// 同じ seed の fixture ではなくなってしまう。そのため、シナリオ用の
/// deferred command を適用した直後にこの checkpoint を置く。
#[cfg(feature = "profiling")]
pub(crate) fn start_perf_capture_system(
    params: PerfCaptureStartParams,
    mut capture: ResMut<PerfCapture>,
    mut exit: MessageWriter<AppExit>,
) {
    if !params.config.enabled() || !matches!(capture.phase, PerfCapturePhase::WaitingForScenario) {
        return;
    }

    if !params.config.workload.has_automated_setup() {
        error!(
            "PERF_CAPTURE: workload '{}' has no automated setup yet; use gather",
            params.config.workload.as_str()
        );
        capture.phase = PerfCapturePhase::Finished;
        exit.write(AppExit::error());
        return;
    }
    if !params.applied.complete() {
        if params.config.uses_fixed_timesteps() && !capture.fixture_wait_reported {
            eprintln!(
                "PERF_DETERMINISM_AUDIT: waiting for fixture setup while virtual time remains paused"
            );
            capture.fixture_wait_reported = true;
        }
        return;
    }

    let initial_checksum = calculate_checksum(&params.checksum_queries);
    let expected_souls = params.config.soul_count as usize;
    let expected_familiars = params.config.familiar_count as usize;
    if initial_checksum.souls != expected_souls || initial_checksum.familiars != expected_familiars
    {
        if !capture.fixture_wait_reported {
            eprintln!(
                "{}: waiting for fixture expected_souls={expected_souls} expected_familiars={expected_familiars} observed_souls={} observed_familiars={}",
                if params.config.uses_fixed_timesteps() {
                    "PERF_DETERMINISM_AUDIT"
                } else {
                    "PERF_CAPTURE"
                },
                initial_checksum.souls,
                initial_checksum.familiars,
            );
            capture.fixture_wait_reported = true;
        }
        return;
    }

    capture.initial_checksum = Some(initial_checksum);
    capture.initial_scene_roots = Some(calculate_scene_root_counts(&params.checksum_queries));
    if params.config.uses_fixed_timesteps() {
        if let Err(error) = record_determinism_checkpoint(
            &mut capture,
            PerfCheckpointRequest {
                checkpoint: "fixture-pre-update",
                update_tick: 0,
                expects_paused_virtual_time: true,
            },
            &params.virtual_time,
            &params.fixed_time,
            &params.checksum_queries,
            PerfWorkSnapshot::from_resources(
                &params.familiar_metrics,
                &params.arbitration_metrics,
                params.runtime_path_budget.metrics(),
                &params.runtime_path_defer_metrics,
                &params.dashboard_metrics,
            ),
        ) {
            error!("PERF_DETERMINISM_AUDIT: invalid initial checkpoint: {error}");
            capture.phase = PerfCapturePhase::Finished;
            exit.write(AppExit::error());
            return;
        }
        capture.phase = PerfCapturePhase::ArmFixedAudit;
        eprintln!(
            "PERF_DETERMINISM_AUDIT: fixture checkpoint captured; arming fixed_hz={} warmup_ticks={} audit_ticks={}",
            params.config.fixed_step_hz(),
            params.config.fixed_warmup_ticks(),
            params.config.fixed_audit_ticks(),
        );
    } else {
        capture.phase = PerfCapturePhase::Warmup;
        capture.elapsed_secs = 0.0;
        eprintln!(
            "PERF_CAPTURE: phase=warmup virtual_speed=1.0 target_secs={}",
            params.config.warmup_secs
        );
    }
}

/// perf scenarioのwarm-up/計測/CSV出力を自動化する。
#[cfg(feature = "profiling")]
pub(crate) fn drive_perf_capture_system(
    mut params: PerfCaptureParams,
    mut capture: ResMut<PerfCapture>,
    mut exit: MessageWriter<AppExit>,
) {
    if !params.config.enabled() || matches!(capture.phase, PerfCapturePhase::Finished) {
        return;
    }

    match capture.phase {
        PerfCapturePhase::WaitingForScenario => {}
        PerfCapturePhase::ArmFixedAudit => {
            if !params.config.uses_fixed_timesteps() {
                error!("PERF_CAPTURE: fixed audit arm phase was entered with realtime clock");
                capture.phase = PerfCapturePhase::Finished;
                exit.write(AppExit::error());
                return;
            }
            params.time.unpause();
            capture.phase = PerfCapturePhase::Warmup;
            eprintln!("PERF_DETERMINISM_AUDIT: phase=warmup");
        }
        PerfCapturePhase::Warmup => {
            if params.config.uses_fixed_timesteps() {
                if let Err(error) = advance_fixed_audit_warmup(
                    &params.config,
                    &mut capture,
                    &params.time,
                    &params.fixed_time,
                    &params.checksum_queries,
                    PerfWorkSnapshot::from_resources(
                        &params.familiar_metrics,
                        &params.arbitration_metrics,
                        params.runtime_path_budget.metrics(),
                        &params.runtime_path_defer_metrics,
                        &params.dashboard_metrics,
                    ),
                ) {
                    error!("PERF_DETERMINISM_AUDIT: invalid warmup checkpoint: {error}");
                    capture.phase = PerfCapturePhase::Finished;
                    exit.write(AppExit::error());
                }
            } else {
                capture.elapsed_secs += params.time.delta_secs();
                capture.warmup_virtual_secs += params.time.delta_secs_f64();
                capture.warmup_real_secs += params.real_time.delta_secs_f64();
                if capture.elapsed_secs >= params.config.warmup_secs {
                    capture.warmup_checksum = Some(calculate_checksum(&params.checksum_queries));
                    capture.phase = PerfCapturePhase::Measure;
                    capture.elapsed_secs = 0.0;
                    capture.frame_times_ms.clear();
                    *params.familiar_metrics = FamiliarDelegationPerfMetrics::default();
                    *params.arbitration_metrics = WheelbarrowArbitrationPerfMetrics::default();
                    *params.dashboard_metrics = TaskDashboardPerfMetrics::default();
                    *params.dashboard_timing_metrics = TaskDashboardTimingMetrics {
                        active: true,
                        ..default()
                    };
                    *params.task_execution_metrics = TaskExecutionPerfMetrics::default();
                    *params.reservation_sync_metrics = ReservationSyncPerfMetrics::default();
                    *params.door_metrics = DoorPerfMetrics::default();
                    *params.construction_metrics = ConstructionPerfMetrics::default();
                    *params.slow_simulation_metrics = SlowSimulationPerfMetrics::default();
                    *params.energy_metrics = EnergyPerfMetrics::default();
                    params.runtime_path_budget.clear_metrics();
                    params.runtime_path_defer_metrics.clear();
                    eprintln!(
                        "PERF_CAPTURE: phase=measure target_secs={}",
                        params.config.measure_secs
                    );
                    #[cfg(feature = "profiling-memory")]
                    crate::profiling_allocator::begin_measurement();
                }
            }
        }
        PerfCapturePhase::Measure => {
            if params.config.uses_fixed_timesteps() {
                if let Err(error) = advance_fixed_audit_measure(
                    &params.config,
                    &mut capture,
                    &params.time,
                    &params.fixed_time,
                    &params.checksum_queries,
                    PerfWorkSnapshot::from_resources(
                        &params.familiar_metrics,
                        &params.arbitration_metrics,
                        params.runtime_path_budget.metrics(),
                        &params.runtime_path_defer_metrics,
                        &params.dashboard_metrics,
                    ),
                ) {
                    error!("PERF_DETERMINISM_AUDIT: invalid audit checkpoint: {error}");
                    capture.phase = PerfCapturePhase::Finished;
                    exit.write(AppExit::error());
                }
            } else {
                capture.elapsed_secs += params.time.delta_secs();
                capture.measure_virtual_secs += params.time.delta_secs_f64();
                capture.measure_real_secs += params.real_time.delta_secs_f64();
                if let Some(frame_time_ms) =
                    params.diagnostics.as_deref().and_then(latest_frame_time_ms)
                {
                    capture.frame_times_ms.push(frame_time_ms);
                }
                if capture.elapsed_secs >= params.config.measure_secs {
                    #[cfg(feature = "profiling-memory")]
                    {
                        capture.memory_measurement = crate::profiling_allocator::end_measurement();
                    }
                    capture.measure_end_checksum =
                        Some(calculate_checksum(&params.checksum_queries));
                    params.dashboard_timing_metrics.active = false;
                    capture.phase = PerfCapturePhase::Flush;
                }
            }
        }
        PerfCapturePhase::Flush => {
            let result = if params.config.uses_fixed_timesteps() {
                write_determinism_audit(
                    &params.config,
                    &capture.determinism_checkpoints,
                    &capture.determinism_actor_records,
                )
            } else {
                match (
                    capture.initial_checksum,
                    capture.initial_scene_roots,
                    capture.warmup_checksum,
                    capture.measure_end_checksum,
                ) {
                    (Some(initial), Some(initial_scene_roots), Some(warmup), Some(measure_end)) => {
                        write_perf_capture(PerfCaptureWriteInput {
                            config: &params.config,
                            initial_checksum: initial,
                            initial_scene_roots,
                            warmup_checksum: warmup,
                            measure_end_checksum: measure_end,
                            samples: &capture.frame_times_ms,
                            warmup_virtual_secs: capture.warmup_virtual_secs,
                            warmup_real_secs: capture.warmup_real_secs,
                            measure_virtual_secs: capture.measure_virtual_secs,
                            measure_real_secs: capture.measure_real_secs,
                            familiar_metrics: &params.familiar_metrics,
                            arbitration_metrics: &params.arbitration_metrics,
                            dashboard_metrics: &params.dashboard_metrics,
                            dashboard_timing_metrics: &params.dashboard_timing_metrics,
                            #[cfg(feature = "profiling-memory")]
                            memory_measurement: &capture.memory_measurement,
                            task_execution_metrics: &params.task_execution_metrics,
                            reservation_sync_metrics: &params.reservation_sync_metrics,
                            door_metrics: &params.door_metrics,
                            construction_metrics: &params.construction_metrics,
                            slow_simulation_metrics: &params.slow_simulation_metrics,
                            energy_metrics: &params.energy_metrics,
                            runtime_path_metrics: params.runtime_path_budget.metrics(),
                            runtime_path_defer_metrics: &params.runtime_path_defer_metrics,
                        })
                    }
                    _ => Err(std::io::Error::other(
                        "capture reached Flush without all scenario checkpoints",
                    )),
                }
            };

            capture.phase = PerfCapturePhase::Finished;
            if let Err(error) = result {
                error!("PERF_CAPTURE: failed to write CSV: {error}");
                exit.write(AppExit::error());
            } else {
                exit.write(AppExit::Success);
            }
        }
        PerfCapturePhase::Finished => {}
    }
}

#[cfg(feature = "profiling")]
fn advance_fixed_audit_warmup(
    config: &PerfScenarioConfig,
    capture: &mut PerfCapture,
    virtual_time: &Time<Virtual>,
    fixed_time: &Time<Fixed>,
    checksum_queries: &PerfChecksumQueries<'_, '_>,
    work: PerfWorkSnapshot,
) -> Result<(), String> {
    capture.fixed_update_tick += 1;
    let tick = capture.fixed_update_tick;
    if let Some(checkpoint) = early_checkpoint_name(tick) {
        record_determinism_checkpoint(
            capture,
            PerfCheckpointRequest {
                checkpoint,
                update_tick: tick,
                expects_paused_virtual_time: false,
            },
            virtual_time,
            fixed_time,
            checksum_queries,
            work,
        )?;
    }
    if tick == config.fixed_warmup_ticks() {
        record_determinism_checkpoint(
            capture,
            PerfCheckpointRequest {
                checkpoint: "post-warmup",
                update_tick: tick,
                expects_paused_virtual_time: false,
            },
            virtual_time,
            fixed_time,
            checksum_queries,
            work,
        )?;
        capture.phase = PerfCapturePhase::Measure;
        eprintln!("PERF_DETERMINISM_AUDIT: phase=audit");
    }
    Ok(())
}

#[cfg(feature = "profiling")]
fn advance_fixed_audit_measure(
    config: &PerfScenarioConfig,
    capture: &mut PerfCapture,
    virtual_time: &Time<Virtual>,
    fixed_time: &Time<Fixed>,
    checksum_queries: &PerfChecksumQueries<'_, '_>,
    work: PerfWorkSnapshot,
) -> Result<(), String> {
    capture.fixed_update_tick += 1;
    let tick = capture.fixed_update_tick;
    if tick == config.fixed_audit_end_tick() {
        record_determinism_checkpoint(
            capture,
            PerfCheckpointRequest {
                checkpoint: "post-audit-end",
                update_tick: tick,
                expects_paused_virtual_time: false,
            },
            virtual_time,
            fixed_time,
            checksum_queries,
            work,
        )?;
        capture.phase = PerfCapturePhase::Flush;
    }
    Ok(())
}

#[cfg(feature = "profiling")]
fn early_checkpoint_name(tick: u64) -> Option<&'static str> {
    match tick {
        1 => Some("post-update-1"),
        8 => Some("post-update-8"),
        32 => Some("post-update-32"),
        128 => Some("post-update-128"),
        _ => None,
    }
}

#[cfg(feature = "profiling")]
struct PerfCheckpointRequest {
    checkpoint: &'static str,
    update_tick: u64,
    expects_paused_virtual_time: bool,
}

#[cfg(feature = "profiling")]
fn record_determinism_checkpoint(
    capture: &mut PerfCapture,
    request: PerfCheckpointRequest,
    virtual_time: &Time<Virtual>,
    fixed_time: &Time<Fixed>,
    checksum_queries: &PerfChecksumQueries<'_, '_>,
    work: PerfWorkSnapshot,
) -> Result<(), String> {
    let PerfCheckpointRequest {
        checkpoint,
        update_tick,
        expects_paused_virtual_time,
    } = request;
    if virtual_time.is_paused() != expects_paused_virtual_time {
        return Err(format!(
            "{checkpoint}: virtual pause state is {}, expected {}",
            virtual_time.is_paused(),
            expects_paused_virtual_time
        ));
    }
    if virtual_time.relative_speed_f64() != 1.0 {
        return Err(format!(
            "{checkpoint}: virtual relative speed is {}, expected 1.0",
            virtual_time.relative_speed_f64()
        ));
    }
    if !expects_paused_virtual_time {
        let timestep = fixed_time.timestep();
        if virtual_time.delta() != timestep {
            return Err(format!(
                "{checkpoint}: virtual delta {:?} differs from fixed timestep {:?}",
                virtual_time.delta(),
                timestep
            ));
        }
        if fixed_time.delta() != timestep {
            return Err(format!(
                "{checkpoint}: fixed delta {:?} differs from fixed timestep {:?}",
                fixed_time.delta(),
                timestep
            ));
        }
        if fixed_time.overstep() != std::time::Duration::ZERO {
            return Err(format!(
                "{checkpoint}: fixed overstep is {:?}, expected zero",
                fixed_time.overstep()
            ));
        }
    }

    let structural_checksum = calculate_checksum(checksum_queries);
    let audit_records = collect_audit_actor_records(checksum_queries)?;
    let checksum = checksum_from_audit_records(&audit_records);
    capture
        .determinism_checkpoints
        .push(PerfDeterminismCheckpoint {
            checkpoint,
            update_tick,
            fixed_timestep_ns: fixed_time.timestep().as_nanos(),
            virtual_delta_ns: virtual_time.delta().as_nanos(),
            virtual_elapsed_ns: virtual_time.elapsed().as_nanos(),
            fixed_delta_ns: fixed_time.delta().as_nanos(),
            fixed_elapsed_ns: fixed_time.elapsed().as_nanos(),
            fixed_overstep_ns: fixed_time.overstep().as_nanos(),
            virtual_paused: virtual_time.is_paused(),
            virtual_relative_speed_bits: virtual_time.relative_speed_f64().to_bits(),
            virtual_effective_speed_bits: virtual_time.effective_speed_f64().to_bits(),
            structural_checksum,
            checksum,
            work,
        });
    capture
        .determinism_actor_records
        .extend(
            audit_records
                .into_iter()
                .map(|record| PerfDeterminismActorRecord {
                    checkpoint,
                    update_tick,
                    actor_kind: record.actor_kind,
                    actor_key: record.actor_key,
                    record: record.record,
                }),
        );
    Ok(())
}
