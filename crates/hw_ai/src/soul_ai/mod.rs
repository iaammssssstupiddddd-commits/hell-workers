use bevy::prelude::*;

use hw_core::system_sets::SoulAiSystemSet;

pub mod execute;
pub mod helpers;
pub mod update;

pub struct SoulAiCorePlugin;

impl Plugin for SoulAiCorePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<helpers::gathering::GatheringUpdateTimer>()
            .register_type::<helpers::gathering::GatheringSpot>()
            .add_systems(
                Update,
                (
                    helpers::gathering::tick_gathering_timer_system,
                    update::gathering_tick::gathering_grace_tick_system,
                    update::vitals_update::fatigue_update_system,
                    update::vitals_update::fatigue_penalty_system,
                    update::rest_area_update::rest_area_update_system,
                    update::state_sanity::ensure_rest_area_component_system,
                    update::state_sanity::clear_stale_working_on_system,
                    update::state_sanity::reconcile_rest_state_system,
                    update::dream_update::dream_update_system,
                )
                    .in_set(SoulAiSystemSet::Update),
            )
            .add_systems(
                Update,
                execute::designation_apply::apply_designation_requests_system
                    .in_set(SoulAiSystemSet::Execute),
            )
            .add_observer(update::vitals::on_task_completed_motivation_bonus)
            .add_observer(update::vitals::on_encouraged_effect)
            .add_observer(update::vitals::on_soul_recruited_effect);
    }
}
