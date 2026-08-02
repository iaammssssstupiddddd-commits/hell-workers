use crate::systems::GameSystemSet;
use crate::systems::soul_ai::scheduling::FamiliarAiSystemSet;
use bevy::prelude::*;

pub mod diagnostics;
pub mod perceive;

pub use hw_core::familiar::FamiliarAiState;

pub struct FamiliarAiPlugin;

impl Plugin for FamiliarAiPlugin {
    fn build(&self, app: &mut App) {
        // hw_ai の FamiliarAiCorePlugin でコアシステムを登録
        app.add_plugins(hw_familiar_ai::FamiliarAiCorePlugin);

        #[cfg(feature = "profiling")]
        app.init_resource::<perceive::resource_sync::ReservationSyncPerfMetrics>();

        app.init_resource::<diagnostics::TaskDiagnosticExternalRevisionState>();
        crate::systems::save::register_load_reset_hook(
            app,
            "task-diagnostics",
            diagnostics::reset_task_diagnostics_for_world_replace,
        );

        app.configure_sets(
            Update,
            hw_familiar_ai::FamiliarSettingsApplySet
                .in_set(GameSystemSet::Logic)
                .before(FamiliarAiSystemSet::Perceive),
        )
        .add_systems(
            Update,
            (
                hw_familiar_ai::apply_familiar_settings_change_requests_system,
                ApplyDeferred,
            )
                .chain()
                .in_set(hw_familiar_ai::FamiliarSettingsApplySet),
        )
        .configure_sets(
            Update,
            (
                FamiliarAiSystemSet::Perceive,
                FamiliarAiSystemSet::Update,
                FamiliarAiSystemSet::Decide,
                FamiliarAiSystemSet::Execute,
            )
                .chain()
                .in_set(GameSystemSet::Logic),
        )
        .init_resource::<perceive::resource_sync::SharedResourceCache>()
        .init_resource::<perceive::resource_sync::ReservationSyncTimer>()
        .init_resource::<perceive::resource_sync::ReservationSignatureCache>()
        .add_systems(
            Update,
            diagnostics::sync_task_diagnostic_revisions_system
                .in_set(hw_familiar_ai::FamiliarTaskDecisionSet::TaskRevisionSync),
        )
        .configure_sets(
            Update,
            hw_familiar_ai::FamiliarTaskDecisionSet::Delegation
                .after(hw_logistics::transport_request::TransportRequestSet::Execute),
        )
        .add_systems(
            Update,
            (
                // === Perceive Phase ===
                (perceive::resource_sync::sync_reservations_system,)
                    .in_set(FamiliarAiSystemSet::Perceive),
                ApplyDeferred
                    .after(FamiliarAiSystemSet::Perceive)
                    .before(FamiliarAiSystemSet::Update),
                ApplyDeferred
                    .after(FamiliarAiSystemSet::Update)
                    .before(FamiliarAiSystemSet::Decide),
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::damned_soul::{DamnedSoul, Path};
    use hw_core::events::{
        FamiliarRosterReleasedVisualMessage, ResourceReservationRequest, SoulTaskUnassignRequest,
    };
    use hw_core::familiar::{Familiar, FamiliarOperation, FamiliarPolicy, FamiliarSettingsPatch};
    use hw_core::relationships::{CommandedBy, Commanding, WorkingOn};
    use hw_core::system_sets::SoulAiSystemSet;
    use hw_jobs::{ActiveTaskIdentity, GeneratePowerData, GeneratePowerPhase, WorkType};
    use hw_logistics::SharedResourceCache;
    use hw_soul_ai::soul_ai::execute::task_execution::AssignedTask;
    use hw_world::WorldMap;

    #[test]
    fn settings_release_cleans_relationship_and_task_in_the_same_update() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<hw_familiar_ai::FamiliarSettingsChangeRequest>()
            .add_message::<hw_familiar_ai::FamiliarSettingsChangeOutcome>()
            .add_message::<SoulTaskUnassignRequest>()
            .add_message::<FamiliarRosterReleasedVisualMessage>()
            .add_message::<ResourceReservationRequest>()
            .init_resource::<SharedResourceCache>()
            .init_resource::<WorldMap>()
            .configure_sets(
                Update,
                (
                    hw_familiar_ai::FamiliarSettingsApplySet,
                    SoulAiSystemSet::Perceive,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    hw_familiar_ai::apply_familiar_settings_change_requests_system,
                    ApplyDeferred,
                )
                    .chain()
                    .in_set(hw_familiar_ai::FamiliarSettingsApplySet),
            )
            .add_systems(
                Update,
                (
                    hw_soul_ai::soul_ai::execute::task_unassign_apply::handle_soul_task_unassign_system,
                    ApplyDeferred,
                )
                    .chain()
                    .in_set(SoulAiSystemSet::Perceive),
            );

        let familiar = app
            .world_mut()
            .spawn((
                Familiar::default(),
                FamiliarOperation {
                    max_controlled_soul: 2,
                    ..default()
                },
                FamiliarPolicy::default(),
                Commanding::default(),
            ))
            .id();
        let target = app.world_mut().spawn_empty().id();
        let spawn_soul = |world: &mut World| {
            world
                .spawn((
                    Transform::default(),
                    DamnedSoul::default(),
                    Path::default(),
                    AssignedTask::GeneratePower(GeneratePowerData {
                        tile: target,
                        tile_pos: Vec2::ZERO,
                        phase: GeneratePowerPhase::Generating,
                    }),
                    ActiveTaskIdentity::new(target, target, WorkType::GeneratePower),
                    WorkingOn(target),
                    CommandedBy(familiar),
                ))
                .id()
        };
        let retained = spawn_soul(app.world_mut());
        let released = spawn_soul(app.world_mut());
        app.world_mut().flush();
        app.world_mut()
            .write_message(hw_familiar_ai::FamiliarSettingsChangeRequest {
                target: familiar,
                patch: FamiliarSettingsPatch::AdjustMaxControlledSoul { delta: -1 },
            });

        app.update();

        assert_eq!(
            app.world()
                .get::<FamiliarOperation>(familiar)
                .unwrap()
                .max_controlled_soul,
            1
        );
        assert_eq!(
            app.world()
                .get::<Commanding>(familiar)
                .unwrap()
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            vec![retained]
        );
        assert!(app.world().get::<CommandedBy>(released).is_none());
        assert!(matches!(
            app.world().get::<AssignedTask>(released),
            Some(AssignedTask::None)
        ));
        assert!(app.world().get::<ActiveTaskIdentity>(released).is_none());
        assert!(app.world().get::<WorkingOn>(released).is_none());
    }
}
