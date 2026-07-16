//! Fixed-cadence simulation clock for vitals and other non-movement updates.
//!
//! The accumulator is driven by `Time<Virtual>` so pause/unpause retains the
//! existing gameplay clock contract. A long render frame processes at most five
//! 100 ms steps, but leaves the remainder in the accumulator rather than
//! silently discarding simulation time.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::time::Virtual;
use hw_core::events::IdleBehaviorRequest;
use hw_core::familiar::{ActiveCommand, Familiar};
use hw_core::soul::DreamPool;
use hw_spatial::FamiliarSpatialGrid;
use std::collections::HashSet;
use std::time::Duration;

use super::{dream_update, rest_area_update, vitals_influence, vitals_update};

pub const SLOW_SIMULATION_STEP: Duration = Duration::from_millis(100);
pub const MAX_SLOW_SIMULATION_STEPS_PER_FRAME: u8 = 5;

/// Feature-gated work counters for the fixed-cadence Soul update path.
/// They count executed work, not wall-clock time, so captures remain useful
/// across machines with different CPUs.
#[cfg(feature = "profiling")]
#[derive(Resource, Debug, Default)]
pub struct SlowSimulationPerfMetrics {
    pub steps: u64,
    pub souls_updated: u64,
    pub idle_decisions: u64,
    pub idle_spatial_target_lookups: u64,
    pub state_sanity_audits: u64,
}

#[derive(Resource, Debug)]
pub struct SlowSimulationClock {
    accumulator: Duration,
    steps_this_frame: u8,
}

impl Default for SlowSimulationClock {
    fn default() -> Self {
        Self {
            accumulator: Duration::ZERO,
            steps_this_frame: 0,
        }
    }
}

impl SlowSimulationClock {
    pub const fn steps_this_frame(&self) -> u8 {
        self.steps_this_frame
    }

    pub const fn step_secs(&self) -> f32 {
        SLOW_SIMULATION_STEP.as_millis() as f32 / 1000.0
    }

    fn advance(&mut self, delta: Duration) {
        self.accumulator = self.accumulator.saturating_add(delta);
        self.steps_this_frame = 0;
        while self.accumulator >= SLOW_SIMULATION_STEP
            && self.steps_this_frame < MAX_SLOW_SIMULATION_STEPS_PER_FRAME
        {
            self.accumulator = self.accumulator.saturating_sub(SLOW_SIMULATION_STEP);
            self.steps_this_frame += 1;
        }
    }
}

pub fn advance_slow_simulation_clock_system(
    virtual_time: Res<Time<Virtual>>,
    mut clock: ResMut<SlowSimulationClock>,
) {
    clock.advance(virtual_time.delta());
}

#[derive(SystemParam)]
pub(crate) struct SlowSimulationDriverParams<'w, 's> {
    commands: Commands<'w, 's>,
    dream_pool: ResMut<'w, DreamPool>,
    request_writer: MessageWriter<'w, IdleBehaviorRequest>,
    familiar_grid: Res<'w, FamiliarSpatialGrid>,
    q_familiars: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static Familiar,
            &'static ActiveCommand,
        ),
    >,
    q_cooldowns: rest_area_update::RestCooldownQuery<'w, 's>,
    nearby_buf: Local<'s, Vec<Entity>>,
    exit_requests: Local<'s, HashSet<Entity>>,
    breakdown_notifications: Local<'s, HashSet<Entity>>,
    exhausted_notifications: Local<'s, HashSet<Entity>>,
    #[cfg(feature = "profiling")]
    metrics: ResMut<'w, SlowSimulationPerfMetrics>,
    queries: ParamSet<
        'w,
        's,
        (
            vitals_update::FatigueUpdateQuery<'w, 's>,
            vitals_update::FatiguePenaltyQuery<'w, 's>,
            rest_area_update::RestingSoulQuery<'w, 's>,
            dream_update::DreamUpdateQuery<'w, 's>,
            vitals_influence::SoulVitalsQuery<'w, 's>,
        ),
    >,
}

/// Runs every slow Soul effect in its original phase order for each 100 ms
/// simulation step. A capped render frame therefore interleaves effects as
/// `fatigue → penalty → rest → dream → influence/stress` instead of applying
/// five complete passes of one subsystem before advancing the next one.
pub(crate) fn slow_simulation_driver_system(
    clock: Res<SlowSimulationClock>,
    mut params: SlowSimulationDriverParams,
) {
    params.exit_requests.clear();
    params.breakdown_notifications.clear();
    params.exhausted_notifications.clear();

    for _ in 0..clock.steps_this_frame() {
        let dt = clock.step_secs();
        #[cfg(feature = "profiling")]
        {
            params.metrics.steps = params.metrics.steps.saturating_add(1);
        }
        {
            let mut q_souls = params.queries.p0();
            let _souls_updated = vitals_update::fatigue_update_step(
                dt,
                &mut params.commands,
                &mut params.exhausted_notifications,
                &mut q_souls,
            );
            #[cfg(feature = "profiling")]
            {
                params.metrics.souls_updated =
                    params.metrics.souls_updated.saturating_add(_souls_updated);
            }
        }
        {
            let mut q_souls = params.queries.p1();
            vitals_update::fatigue_penalty_step(dt, &mut q_souls);
        }
        {
            let mut q_resting_souls = params.queries.p2();
            rest_area_update::rest_area_update_step(
                dt,
                &mut params.commands,
                &mut params.dream_pool,
                &mut params.request_writer,
                &mut params.exit_requests,
                &mut q_resting_souls,
                &mut params.q_cooldowns,
            );
        }
        {
            let mut q_souls = params.queries.p3();
            dream_update::dream_update_step(dt, &mut params.dream_pool, &mut q_souls);
        }
        {
            let mut q_souls = params.queries.p4();
            vitals_influence::familiar_influence_step(
                dt,
                &mut params.commands,
                &params.familiar_grid,
                &params.q_familiars,
                &mut params.nearby_buf,
                &mut params.breakdown_notifications,
                &mut q_souls,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retains_long_frame_remainder_after_the_step_cap() {
        let mut clock = SlowSimulationClock::default();
        clock.advance(Duration::from_secs(1));

        assert_eq!(
            clock.steps_this_frame(),
            MAX_SLOW_SIMULATION_STEPS_PER_FRAME
        );

        // No additional virtual time is needed to consume the retained 0.5 s.
        clock.advance(Duration::ZERO);
        assert_eq!(
            clock.steps_this_frame(),
            MAX_SLOW_SIMULATION_STEPS_PER_FRAME
        );
    }

    #[test]
    fn pauses_without_accumulating_or_catching_up() {
        let mut clock = SlowSimulationClock::default();
        clock.advance(Duration::from_millis(99));
        assert_eq!(clock.steps_this_frame(), 0);

        clock.advance(Duration::ZERO);
        assert_eq!(clock.steps_this_frame(), 0);

        clock.advance(Duration::from_millis(1));
        assert_eq!(clock.steps_this_frame(), 1);
    }
}
