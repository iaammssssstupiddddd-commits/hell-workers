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
