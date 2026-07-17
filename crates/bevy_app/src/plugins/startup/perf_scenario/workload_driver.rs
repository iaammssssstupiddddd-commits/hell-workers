use super::*;

/// Applies the deterministic path-door interaction sequence after the initial
/// fixture checkpoint. The slot is derived from virtual time, so render-frame
/// timing and user input cannot alter the workload.
#[cfg(feature = "profiling")]
pub(crate) fn drive_perf_workload_system(
    config: Res<PerfScenarioConfig>,
    applied: Res<PerfScenarioApplied>,
    virtual_time: Res<Time<Virtual>>,
    mut state: ResMut<PerfScenarioDriverState>,
    handles: Res<DoorVisualHandles>,
    mut world_map: WorldMapWrite,
    mut q_doors: Query<(&PerfFixtureMarker, &Transform, &mut Door, &mut Sprite)>,
) {
    if !applied.0 || !config.enabled() || config.workload != PerfWorkload::PathDoor {
        return;
    }

    let toggle_slot = (virtual_time.elapsed_secs_f64() / 0.5).floor() as u64;
    if toggle_slot == 0 || state.last_path_door_toggle_slot == Some(toggle_slot) {
        return;
    }
    state.last_path_door_toggle_slot = Some(toggle_slot);
    let next_state = if toggle_slot.is_multiple_of(2) {
        DoorState::Closed
    } else {
        DoorState::Open
    };
    for (marker, transform, mut door, mut sprite) in q_doors.iter_mut() {
        if marker.kind != PerfFixtureKind::Door || door.state == next_state {
            continue;
        }
        let grid = WorldMap::world_to_grid(transform.translation.truncate());
        hw_world::apply_door_state(
            &mut door,
            &mut sprite,
            &mut world_map,
            &handles,
            grid,
            next_state,
        );
    }
}
