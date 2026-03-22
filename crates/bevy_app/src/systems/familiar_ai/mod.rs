use crate::systems::GameSystemSet;
use crate::systems::soul_ai::scheduling::FamiliarAiSystemSet;
use bevy::prelude::*;

pub mod perceive;

pub use hw_core::familiar::FamiliarAiState;

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
        .init_resource::<perceive::resource_sync::SharedResourceCache>()
        .init_resource::<perceive::resource_sync::ReservationSyncTimer>()
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
