use bevy::ecs::schedule::ApplyDeferred;
use bevy::prelude::*;
use hw_core::WorldEpoch;

use crate::systems::GameSystemSet;
use crate::systems::lighting::{
    DoorLockToggleMetrics, DoorManualMutationSet, IndoorLightRuntime,
    IndoorLightingAllocationProbe, IndoorLightingCollectSet, IndoorLightingDirty,
    IndoorLightingDirtyCollectSet, IndoorLightingEmitterSyncSet, IndoorLightingLifecycleProbe,
    IndoorLightingRebuildSet, collect_indoor_lighting_snapshot_system,
    consume_door_lock_toggle_requests_system, mark_indoor_lighting_dirty_system,
    rebuild_indoor_lighting_field_system, reset_indoor_lighting_for_world_replace,
    sync_outdoor_lamp_emitters_system,
};
use crate::systems::save::SaveRecoveryMode;

pub struct IndoorLightingPlugin;

fn lighting_runtime_is_trusted(recovery_mode: Option<Res<SaveRecoveryMode>>) -> bool {
    recovery_mode_is_trusted(recovery_mode.as_deref())
}

fn recovery_mode_is_trusted(recovery_mode: Option<&SaveRecoveryMode>) -> bool {
    recovery_mode.is_none_or(|mode| *mode == SaveRecoveryMode::Healthy)
}

impl Plugin for IndoorLightingPlugin {
    fn build(&self, app: &mut App) {
        crate::systems::save::register_lighting_rehydrate_pipeline(app);
        crate::systems::save::register_load_reset_hook(
            app,
            "lighting-runtime",
            reset_indoor_lighting_for_world_replace,
        );
        app.init_resource::<DoorLockToggleMetrics>()
            .init_resource::<IndoorLightingDirty>()
            .init_resource::<IndoorLightRuntime>()
            .init_resource::<IndoorLightingAllocationProbe>()
            .init_resource::<IndoorLightingLifecycleProbe>()
            .init_resource::<WorldEpoch>()
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
                    .run_if(lighting_runtime_is_trusted)
                    .in_set(IndoorLightingEmitterSyncSet),
            )
            .add_systems(
                Update,
                mark_indoor_lighting_dirty_system
                    .run_if(lighting_runtime_is_trusted)
                    .in_set(IndoorLightingDirtyCollectSet),
            )
            .add_systems(
                Update,
                collect_indoor_lighting_snapshot_system
                    .run_if(lighting_runtime_is_trusted)
                    .in_set(IndoorLightingCollectSet),
            )
            .add_systems(
                Update,
                rebuild_indoor_lighting_field_system
                    .run_if(lighting_runtime_is_trusted)
                    .in_set(IndoorLightingRebuildSet),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_failed_disables_lighting_runtime_updates() {
        assert!(recovery_mode_is_trusted(None));
        assert!(recovery_mode_is_trusted(Some(&SaveRecoveryMode::Healthy)));
        assert!(!recovery_mode_is_trusted(Some(
            &SaveRecoveryMode::RecoveryFailed
        )));
    }
}
