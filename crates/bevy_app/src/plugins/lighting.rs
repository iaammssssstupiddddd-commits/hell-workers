use bevy::ecs::schedule::ApplyDeferred;
use bevy::prelude::*;
use hw_core::WorldEpoch;

use crate::systems::GameSystemSet;
use crate::systems::energy::lamp_buff::apply_light_recovery_effect_system;
use crate::systems::lighting::{
    DoorLockToggleMetrics, DoorManualMutationSet, IndoorLightConsumerMetrics, IndoorLightRuntime,
    IndoorLightingAllocationProbe, IndoorLightingCollectSet, IndoorLightingDirty,
    IndoorLightingDirtyCollectSet, IndoorLightingEmitterSyncSet, IndoorLightingLifecycleProbe,
    IndoorLightingRebuildSet, RoomIlluminationCache, RoomIlluminationSummarySet,
    SoulLightRecoverySet, collect_indoor_lighting_snapshot_system,
    consume_door_lock_toggle_requests_system, mark_indoor_lighting_dirty_system,
    rebuild_indoor_lighting_field_system, reset_indoor_light_consumers_for_world_replace,
    reset_indoor_lighting_for_world_replace, sync_outdoor_lamp_emitters_system,
    update_room_illumination_summaries_system,
};
use crate::systems::save::SaveRecoveryMode;

pub struct IndoorLightingPlugin;

fn lighting_runtime_is_trusted(recovery_mode: Option<Res<SaveRecoveryMode>>) -> bool {
    recovery_mode_is_trusted(recovery_mode.as_deref())
}

fn recovery_mode_is_trusted(recovery_mode: Option<&SaveRecoveryMode>) -> bool {
    recovery_mode.is_none_or(|mode| *mode == SaveRecoveryMode::Healthy)
}

fn configure_indoor_light_consumer_schedule(app: &mut App) {
    app.configure_sets(
        Update,
        (
            SoulLightRecoverySet
                .after(IndoorLightingRebuildSet)
                .in_set(GameSystemSet::PostActor),
            RoomIlluminationSummarySet
                .after(SoulLightRecoverySet)
                .in_set(GameSystemSet::PostActor),
        )
            .chain(),
    );
}

impl Plugin for IndoorLightingPlugin {
    fn build(&self, app: &mut App) {
        crate::systems::save::register_lighting_rehydrate_pipeline(app);
        crate::systems::save::register_load_reset_hook(
            app,
            "lighting-runtime",
            reset_indoor_lighting_for_world_replace,
        );
        crate::systems::save::register_load_reset_hook(
            app,
            "lighting-consumers",
            reset_indoor_light_consumers_for_world_replace,
        );
        app.init_resource::<DoorLockToggleMetrics>()
            .init_resource::<IndoorLightingDirty>()
            .init_resource::<IndoorLightRuntime>()
            .init_resource::<IndoorLightingAllocationProbe>()
            .init_resource::<IndoorLightingLifecycleProbe>()
            .init_resource::<IndoorLightConsumerMetrics>()
            .init_resource::<RoomIlluminationCache>()
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
            )
            .add_systems(
                Update,
                apply_light_recovery_effect_system
                    .run_if(lighting_runtime_is_trusted)
                    .run_if(|time: Res<Time<Virtual>>| !time.is_paused())
                    .in_set(SoulLightRecoverySet),
            )
            .add_systems(
                Update,
                (update_room_illumination_summaries_system, ApplyDeferred)
                    .chain()
                    .run_if(lighting_runtime_is_trusted)
                    .in_set(RoomIlluminationSummarySet),
            );
        configure_indoor_light_consumer_schedule(app);
        #[cfg(feature = "profiling")]
        app.init_resource::<crate::systems::lighting::IndoorLightCrossConsumerObservation>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Resource, Default)]
    struct ScheduleTrace(Vec<&'static str>);

    fn trace_rebuild(mut trace: ResMut<ScheduleTrace>) {
        trace.0.push("rebuild");
    }

    fn trace_recovery(mut trace: ResMut<ScheduleTrace>) {
        trace.0.push("recovery");
    }

    fn trace_room_summary(mut trace: ResMut<ScheduleTrace>) {
        trace.0.push("room-summary");
    }

    #[test]
    fn recovery_failed_disables_lighting_runtime_updates() {
        assert!(recovery_mode_is_trusted(None));
        assert!(recovery_mode_is_trusted(Some(&SaveRecoveryMode::Healthy)));
        assert!(!recovery_mode_is_trusted(Some(
            &SaveRecoveryMode::RecoveryFailed
        )));
    }

    #[test]
    fn consumer_sets_follow_the_current_field_in_post_actor() {
        let mut app = App::new();
        app.init_resource::<ScheduleTrace>().add_systems(
            Update,
            (
                trace_rebuild.in_set(IndoorLightingRebuildSet),
                trace_recovery.in_set(SoulLightRecoverySet),
                trace_room_summary.in_set(RoomIlluminationSummarySet),
            ),
        );
        configure_indoor_light_consumer_schedule(&mut app);

        app.update();

        assert_eq!(
            app.world().resource::<ScheduleTrace>().0,
            ["rebuild", "recovery", "room-summary"]
        );
    }
}
