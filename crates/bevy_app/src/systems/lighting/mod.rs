mod components;
mod lifecycle;
mod room_summary;
mod runtime;

pub use components::{LightingFixtureMount, RadialLightEmitter};
pub(crate) use lifecycle::{
    normalize_lighting_mounts, rebuild_lighting_emitters, reset_indoor_lighting_for_world_replace,
    validate_fixture_mount_candidate, wake_indoor_lighting,
};
pub(crate) use room_summary::reset_indoor_light_consumers_for_world_replace;
pub use room_summary::{
    IndoorLightConsumerMetrics, RoomIlluminationCache, RoomIlluminationState,
    RoomIlluminationSummarySet, SoulLightRecoverySet, read_room_illumination_state,
    update_room_illumination_summaries_system,
};
pub use runtime::{
    DoorLockToggleMetrics, DoorManualMutationSet, IndoorLightAvailability, IndoorLightRuntime,
    IndoorLightingAllocationProbe, IndoorLightingCollectSet, IndoorLightingDirty,
    IndoorLightingDirtyCollectSet, IndoorLightingEmitterSyncSet, IndoorLightingLifecycleProbe,
    IndoorLightingMetrics, IndoorLightingRebuildSet, collect_indoor_lighting_snapshot_system,
    consume_door_lock_toggle_requests_system, mark_indoor_lighting_dirty_system,
    read_indoor_light_snapshot, rebuild_indoor_lighting_field_system,
    sync_outdoor_lamp_emitters_system,
};

#[cfg(test)]
mod tests;
