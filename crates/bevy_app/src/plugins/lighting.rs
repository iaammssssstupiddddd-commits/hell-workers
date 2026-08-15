use bevy::ecs::schedule::ApplyDeferred;
use bevy::prelude::*;

use crate::systems::GameSystemSet;
use crate::systems::lighting::{
    DoorLockToggleMetrics, DoorManualMutationSet, IndoorLightRuntime,
    IndoorLightingAllocationProbe, IndoorLightingCollectSet, IndoorLightingDirty,
    IndoorLightingDirtyCollectSet, IndoorLightingEmitterSyncSet, IndoorLightingRebuildSet,
    collect_indoor_lighting_snapshot_system, consume_door_lock_toggle_requests_system,
    mark_indoor_lighting_dirty_system, rebuild_indoor_lighting_field_system,
    sync_outdoor_lamp_emitters_system,
};

pub struct IndoorLightingPlugin;

impl Plugin for IndoorLightingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DoorLockToggleMetrics>()
            .init_resource::<IndoorLightingDirty>()
            .init_resource::<IndoorLightRuntime>()
            .init_resource::<IndoorLightingAllocationProbe>()
            .configure_sets(
                Update,
                DoorManualMutationSet.in_set(GameSystemSet::PreActor),
            )
            .configure_sets(
                Update,
                (
                    IndoorLightingEmitterSyncSet.in_set(GameSystemSet::PostActor),
                    IndoorLightingDirtyCollectSet.in_set(GameSystemSet::PostActor),
                    IndoorLightingCollectSet.in_set(GameSystemSet::PostActor),
                    IndoorLightingRebuildSet.in_set(GameSystemSet::PostActor),
                )
                    .chain(),
            )
            .add_systems(
                Update,
                consume_door_lock_toggle_requests_system.in_set(DoorManualMutationSet),
            )
            .add_systems(
                Update,
                (sync_outdoor_lamp_emitters_system, ApplyDeferred)
                    .chain()
                    .in_set(IndoorLightingEmitterSyncSet),
            )
            .add_systems(
                Update,
                mark_indoor_lighting_dirty_system.in_set(IndoorLightingDirtyCollectSet),
            )
            .add_systems(
                Update,
                collect_indoor_lighting_snapshot_system.in_set(IndoorLightingCollectSet),
            )
            .add_systems(
                Update,
                rebuild_indoor_lighting_field_system.in_set(IndoorLightingRebuildSet),
            );
    }
}
