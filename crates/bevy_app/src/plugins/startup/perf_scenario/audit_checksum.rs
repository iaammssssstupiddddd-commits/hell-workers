use super::*;

#[cfg(feature = "profiling")]
pub(super) fn latest_frame_time_ms(
    diagnostics: &bevy::diagnostic::DiagnosticsStore,
) -> Option<f64> {
    diagnostics
        .get_measurement(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FRAME_TIME)
        .map(|measurement| measurement.value)
}

#[cfg(feature = "profiling")]
pub(super) fn calculate_checksum(
    checksum_queries: &PerfChecksumQueries<'_, '_>,
) -> PerfScenarioChecksum {
    let souls = checksum_queries
        .souls
        .iter()
        .map(|(_, transform)| checksum_position(transform))
        .collect::<Vec<_>>();
    let familiars = checksum_queries
        .familiars
        .iter()
        .map(|(_, transform)| checksum_position(transform))
        .collect::<Vec<_>>();
    let designations = checksum_queries.designations.iter().count();
    let mut positions = souls.clone();
    positions.extend(familiars.iter().copied());
    positions.sort_unstable();

    let mut checksum = 0xcbf2_9ce4_8422_2325u64;
    for value in [
        souls.len() as u64,
        familiars.len() as u64,
        designations as u64,
    ] {
        checksum = fnv1a(checksum, value);
    }
    for (x, y) in positions {
        checksum = fnv1a(checksum, x as u64);
        checksum = fnv1a(checksum, y as u64);
    }

    PerfScenarioChecksum {
        souls: souls.len(),
        familiars: familiars.len(),
        designations,
        value: checksum,
    }
}

#[cfg(feature = "profiling")]
pub(super) fn calculate_scene_root_counts(
    checksum_queries: &PerfChecksumQueries<'_, '_>,
) -> PerfSceneRootCounts {
    PerfSceneRootCounts {
        soul_proxy_3d: checksum_queries.soul_proxy_3d.iter().count(),
        soul_mask_proxy_3d: checksum_queries.soul_mask_proxy_3d.iter().count(),
        soul_shadow_proxy_3d: checksum_queries.soul_shadow_proxy_3d.iter().count(),
        familiar_proxy_3d: checksum_queries.familiar_proxy_3d.iter().count(),
        building_3d_visual: checksum_queries.building_3d_visual.iter().count(),
    }
}

#[cfg(feature = "profiling")]
pub(super) fn collect_audit_actor_records(
    checksum_queries: &PerfChecksumQueries<'_, '_>,
) -> Result<Vec<PerfAuditActorRecord>, String> {
    let mut records = Vec::new();

    for (entity, transform, soul, idle, destination, path, task, random_state) in
        checksum_queries.audit_souls.iter()
    {
        let mut record = vec![b'S'];
        write_transform(&mut record, transform, "soul transform")?;
        write_f32(&mut record, soul.laziness, "soul laziness")?;
        write_f32(&mut record, soul.motivation, "soul motivation")?;
        write_f32(&mut record, soul.fatigue, "soul fatigue")?;
        write_f32(&mut record, soul.stress, "soul stress")?;
        write_f32(&mut record, soul.dream, "soul dream")?;
        write_idle_state(&mut record, idle)?;
        write_vec2(&mut record, destination.0, "soul destination")?;
        write_path(&mut record, path, "soul path")?;
        write_assigned_task(&mut record, task, &checksum_queries.target_transforms)?;
        write_option_u64(
            &mut record,
            random_state.map(SimulationRandomState::audit_cursor),
        );
        records.push(PerfAuditActorRecord {
            actor_kind: "soul",
            actor_key: random_state
                .map(SimulationRandomState::stable_key)
                .unwrap_or_else(|| entity.to_bits()),
            record,
        });
    }

    for (
        entity,
        transform,
        familiar,
        destination,
        path,
        command,
        operation,
        ai_state,
        random_state,
    ) in checksum_queries.audit_familiars.iter()
    {
        let mut record = vec![b'F'];
        write_transform(&mut record, transform, "familiar transform")?;
        write_familiar_state(&mut record, familiar, command, operation, ai_state)?;
        write_vec2(&mut record, destination.0, "familiar destination")?;
        write_path(&mut record, path, "familiar path")?;
        write_option_u64(
            &mut record,
            random_state.map(SimulationRandomState::audit_cursor),
        );
        records.push(PerfAuditActorRecord {
            actor_kind: "familiar",
            actor_key: random_state
                .map(SimulationRandomState::stable_key)
                .unwrap_or_else(|| entity.to_bits()),
            record,
        });
    }

    for (entity, transform, designation, priority, slots) in
        checksum_queries.audit_designations.iter()
    {
        let mut record = vec![b'D'];
        write_transform(&mut record, transform, "designation transform")?;
        write_work_type(&mut record, designation.work_type);
        write_option_u32(&mut record, priority.map(|priority| priority.0));
        write_option_u32(&mut record, slots.map(|slots| slots.max));
        records.push(PerfAuditActorRecord {
            actor_kind: "designation",
            actor_key: entity.to_bits(),
            record,
        });
    }

    for (_entity, marker, transform, door, floor_site, floor_tile, blueprint) in
        checksum_queries.audit_fixtures.iter()
    {
        let mut record = vec![b'X', marker.kind.audit_tag()];
        write_u64(&mut record, marker.ordinal as u64);
        write_transform(&mut record, transform, "fixture transform")?;
        match marker.kind {
            PerfFixtureKind::Door => {
                let Some(door) = door else {
                    return Err("door fixture is missing Door".to_string());
                };
                write_door_state(&mut record, door.state);
            }
            PerfFixtureKind::ConstructionSite => {
                let Some(site) = floor_site else {
                    return Err(
                        "construction site fixture is missing FloorConstructionSite".to_string()
                    );
                };
                write_floor_phase(&mut record, site.phase);
                write_u64(&mut record, site.tiles_total as u64);
                write_u64(&mut record, site.tiles_reinforced as u64);
                write_u64(&mut record, site.tiles_poured as u64);
                write_f32(
                    &mut record,
                    site.curing_remaining_secs,
                    "fixture curing remaining secs",
                )?;
            }
            PerfFixtureKind::ConstructionTile => {
                let Some(tile) = floor_tile else {
                    return Err(
                        "construction tile fixture is missing FloorTileBlueprint".to_string()
                    );
                };
                write_grid_pos(&mut record, tile.grid_pos);
                write_floor_tile_state(&mut record, tile.state);
                write_u64(&mut record, tile.bones_delivered as u64);
                write_u64(&mut record, tile.mud_delivered as u64);
            }
            PerfFixtureKind::UiBlueprint => {
                let Some(blueprint) = blueprint else {
                    return Err("ui-gpu fixture is missing Blueprint".to_string());
                };
                write_building_type(&mut record, blueprint.kind);
                write_f32(
                    &mut record,
                    blueprint.progress,
                    "fixture blueprint progress",
                )?;
                write_u64(&mut record, blueprint.occupied_grids.len() as u64);
                for grid in &blueprint.occupied_grids {
                    write_grid_pos(&mut record, *grid);
                }
            }
        }
        records.push(PerfAuditActorRecord {
            actor_kind: "fixture",
            actor_key: ((marker.kind.audit_tag() as u64) << 32) | u64::from(marker.ordinal),
            record,
        });
    }

    records.sort_unstable_by(|left, right| {
        left.actor_kind
            .cmp(right.actor_kind)
            .then(left.actor_key.cmp(&right.actor_key))
    });
    Ok(records)
}

#[cfg(feature = "profiling")]
pub(super) fn checksum_from_audit_records(
    records: &[PerfAuditActorRecord],
) -> PerfScenarioChecksum {
    let mut payloads = records
        .iter()
        .map(|record| record.record.as_slice())
        .collect::<Vec<_>>();
    payloads.sort_unstable();
    let mut checksum = fnv1a(0xcbf2_9ce4_8422_2325u64, payloads.len() as u64);
    for record in payloads {
        checksum = fnv1a_bytes(checksum, record);
    }

    PerfScenarioChecksum {
        souls: records
            .iter()
            .filter(|record| record.actor_kind == "soul")
            .count(),
        familiars: records
            .iter()
            .filter(|record| record.actor_kind == "familiar")
            .count(),
        designations: records
            .iter()
            .filter(|record| record.actor_kind == "designation")
            .count(),
        value: checksum,
    }
}

#[cfg(feature = "profiling")]
fn checksum_position(transform: &Transform) -> (i64, i64) {
    (
        (transform.translation.x * 100.0).round() as i64,
        (transform.translation.y * 100.0).round() as i64,
    )
}
