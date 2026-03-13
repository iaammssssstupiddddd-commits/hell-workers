use crate::systems::GameSystemSet;
use crate::systems::soul_ai::scheduling::FamiliarAiSystemSet;
use bevy::prelude::*;
use hw_spatial::{DesignationSpatialGrid, TransportRequestSpatialGrid};

pub mod decide;
pub mod execute;
pub mod helpers;
pub mod perceive;
pub mod update;

pub use helpers::query_types::FamiliarSoulQuery;
pub use hw_core::familiar::FamiliarAiState;
pub use hw_familiar_ai::familiar_ai::decide::resources::{
    FamiliarDelegationPerfMetrics, FamiliarTaskDelegationTimer,
};

pub struct FamiliarAiPlugin;

impl Plugin for FamiliarAiPlugin {
    fn build(&self, app: &mut App) {
        // hw_ai の FamiliarAiCorePlugin でコアシステムを登録
        app.add_plugins(hw_familiar_ai::FamiliarAiCorePlugin);

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
        .register_type::<FamiliarAiState>()
        .init_resource::<perceive::resource_sync::SharedResourceCache>()
        .init_resource::<perceive::resource_sync::ReservationSyncTimer>()
        .init_resource::<DesignationSpatialGrid>()
        .init_resource::<TransportRequestSpatialGrid>()
        .init_resource::<decide::auto_gather_for_blueprint::BlueprintAutoGatherTimer>()
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
                // === Decide Phase ===
                ((
                    decide::auto_gather_for_blueprint::blueprint_auto_gather_system,
                    ApplyDeferred,
                    decide::encouragement::encouragement_decision_system,
                )
                    .chain()
                    .after(hw_familiar_ai::familiar_ai::decide::state_decision::familiar_ai_state_system),)
                    .in_set(FamiliarAiSystemSet::Decide),
                // === Execute Phase ===
                (
                    execute::max_soul_apply::handle_max_soul_changed_system,
                    execute::idle_visual_apply::familiar_idle_visual_apply_system,
                    execute::squad_apply::apply_squad_management_requests_system,
                    execute::encouragement_apply::encouragement_apply_system,
                    execute::encouragement_apply::cleanup_encouragement_cooldowns_system,
                )
                    .in_set(FamiliarAiSystemSet::Execute),
            ),
        );
    }
}
