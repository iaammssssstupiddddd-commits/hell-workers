//! Fixed-cadence simulation clock for vitals and other non-movement updates.
//!
//! The accumulator is driven by `Time<Virtual>` so pause/unpause retains the
//! existing gameplay clock contract. A long render frame processes at most five
//! 100 ms steps, but leaves the remainder in the accumulator rather than
//! silently discarding simulation time.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::time::Virtual;
use hw_core::events::{
    DreamTransferVisualSource, DreamTransferredVisualMessage, IdleBehaviorRequest,
};
use hw_core::familiar::{ActiveCommand, Familiar};
use hw_core::soul::{DreamPool, DreamQuality};
use hw_spatial::FamiliarSpatialGrid;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

use super::{dream_update, rest_area_update, vitals_influence, vitals_update};

pub const SLOW_SIMULATION_STEP: Duration = Duration::from_millis(100);
pub const MAX_SLOW_SIMULATION_STEPS_PER_FRAME: u8 = 5;

#[derive(Debug, Clone, Copy, PartialEq)]
struct PendingDreamTransfer {
    amount: f32,
    quality: DreamQuality,
    source: DreamTransferVisualSource,
    is_final: bool,
}

/// Aggregates all slow steps in one driver invocation into at most one visual
/// message per Soul. The first positive transfer fixes the visual snapshot;
/// later steps must describe the same source and quality.
#[derive(Default)]
pub(crate) struct DreamTransferAccumulator {
    transfers: HashMap<Entity, PendingDreamTransfer>,
}

impl DreamTransferAccumulator {
    pub(crate) fn clear(&mut self) {
        self.transfers.clear();
    }

    pub(crate) fn record(
        &mut self,
        soul: Entity,
        amount: f32,
        quality: DreamQuality,
        source: DreamTransferVisualSource,
        is_final: bool,
    ) {
        debug_assert!(amount.is_finite() && amount > 0.0);
        if !amount.is_finite() || amount <= 0.0 {
            return;
        }

        self.transfers
            .entry(soul)
            .and_modify(|pending| {
                let same_snapshot = pending.quality == quality
                    && match (pending.source, source) {
                        (
                            DreamTransferVisualSource::Sleeping { .. },
                            DreamTransferVisualSource::Sleeping { .. },
                        ) => true,
                        (
                            DreamTransferVisualSource::RestArea {
                                rest_area: pending, ..
                            },
                            DreamTransferVisualSource::RestArea {
                                rest_area: current, ..
                            },
                        ) => pending == current,
                        _ => false,
                    };
                debug_assert!(
                    same_snapshot,
                    "a Soul changed Dream transfer source within one slow-simulation driver run"
                );
                if !same_snapshot {
                    error!(
                        ?soul,
                        ?source,
                        first_source = ?pending.source,
                        "keeping the first Dream transfer visual snapshot"
                    );
                }
                pending.amount += amount;
                pending.is_final |= is_final;
            })
            .or_insert(PendingDreamTransfer {
                amount,
                quality,
                source,
                is_final,
            });
    }

    fn publish(&mut self, writer: &mut MessageWriter<DreamTransferredVisualMessage>) {
        for (soul, pending) in self.transfers.drain() {
            writer.write(DreamTransferredVisualMessage {
                soul,
                amount: pending.amount,
                quality: pending.quality,
                source: pending.source,
                is_final: pending.is_final,
            });
        }
    }
}

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
    dream_transfer_writer: MessageWriter<'w, DreamTransferredVisualMessage>,
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
    dream_transfers: Local<'s, DreamTransferAccumulator>,
    q_transforms: Query<'w, 's, &'static Transform>,
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
    params.dream_transfers.clear();

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
            let mut transfer = rest_area_update::RestDreamTransferContext {
                dream_pool: &mut params.dream_pool,
                transfers: &mut params.dream_transfers,
                q_transforms: &params.q_transforms,
            };
            rest_area_update::rest_area_update_step(
                dt,
                &mut params.commands,
                &mut params.request_writer,
                &mut params.exit_requests,
                &mut transfer,
                &mut q_resting_souls,
                &mut params.q_cooldowns,
            );
        }
        {
            let mut q_souls = params.queries.p3();
            dream_update::dream_update_step(
                dt,
                &mut params.dream_pool,
                &mut params.dream_transfers,
                &mut q_souls,
            );
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

    params
        .dream_transfers
        .publish(&mut params.dream_transfer_writer);
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_core::events::DreamTransferredVisualMessage;
    use hw_core::relationships::{RestAreaOccupants, RestingIn};
    use hw_core::soul::{DamnedSoul, DreamState, IdleBehavior, IdleState};
    use hw_jobs::AssignedTask;

    #[derive(Resource, Default)]
    struct TransferProbe(Vec<DreamTransferredVisualMessage>);

    fn collect_transfers(
        mut messages: MessageReader<DreamTransferredVisualMessage>,
        mut probe: ResMut<TransferProbe>,
    ) {
        probe.0.extend(messages.read().copied());
    }

    fn driver_test_app(steps: u8) -> App {
        let mut app = App::new();
        let clock = SlowSimulationClock {
            steps_this_frame: steps,
            ..default()
        };
        app.insert_resource(clock)
            .init_resource::<DreamPool>()
            .init_resource::<FamiliarSpatialGrid>()
            .init_resource::<TransferProbe>()
            .add_message::<IdleBehaviorRequest>()
            .add_message::<DreamTransferredVisualMessage>()
            .add_systems(
                Update,
                (slow_simulation_driver_system, collect_transfers).chain(),
            );
        app
    }

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

    #[test]
    fn aggregates_transfer_amount_without_replacing_the_first_snapshot() {
        let soul = Entity::from_raw_u32(1).unwrap();
        let source = DreamTransferVisualSource::Sleeping {
            origin: Vec2::new(4.0, 8.0),
        };
        let mut transfers = DreamTransferAccumulator::default();

        transfers.record(soul, 0.1, DreamQuality::NormalDream, source, false);
        transfers.record(soul, 0.2, DreamQuality::NormalDream, source, true);

        let pending = transfers.transfers.get(&soul).unwrap();
        assert!((pending.amount - 0.3).abs() <= f32::EPSILON);
        assert_eq!(pending.quality, DreamQuality::NormalDream);
        assert_eq!(pending.source, source);
        assert!(pending.is_final);
    }

    #[test]
    fn slow_simulation_emits_one_dream_transfer_per_soul_per_frame() {
        let mut app = driver_test_app(MAX_SLOW_SIMULATION_STEPS_PER_FRAME);

        let first = app
            .world_mut()
            .spawn((
                Transform::default(),
                DamnedSoul {
                    dream: 0.5,
                    ..default()
                },
                IdleState {
                    behavior: IdleBehavior::Sleeping,
                    ..default()
                },
                DreamState::default(),
                AssignedTask::None,
            ))
            .id();
        let second = app
            .world_mut()
            .spawn((
                Transform::default(),
                DamnedSoul {
                    dream: 0.3,
                    ..default()
                },
                IdleState {
                    behavior: IdleBehavior::Sleeping,
                    ..default()
                },
                DreamState::default(),
                AssignedTask::None,
            ))
            .id();

        app.update();

        let probe = app.world().resource::<TransferProbe>();
        assert_eq!(probe.0.len(), 2);
        for soul in [first, second] {
            assert_eq!(
                probe
                    .0
                    .iter()
                    .filter(|message| message.soul == soul)
                    .count(),
                1
            );
        }
    }

    #[test]
    fn sleeping_final_drain_preserves_transfer_mass() {
        let mut app = driver_test_app(MAX_SLOW_SIMULATION_STEPS_PER_FRAME);

        let soul = app
            .world_mut()
            .spawn((
                Transform::from_xyz(12.0, 34.0, 0.0),
                DamnedSoul {
                    dream: 0.25,
                    ..default()
                },
                IdleState {
                    behavior: IdleBehavior::Sleeping,
                    ..default()
                },
                DreamState::default(),
                AssignedTask::None,
            ))
            .id();

        app.update();

        let pool = app.world().resource::<DreamPool>();
        let probe = app.world().resource::<TransferProbe>();
        assert!((pool.points - 0.25).abs() <= 1e-5);
        assert_eq!(probe.0.len(), 1);
        assert_eq!(probe.0[0].soul, soul);
        assert!((probe.0[0].amount - pool.points).abs() <= 1e-5);
        assert_eq!(probe.0[0].quality, DreamQuality::NormalDream);
        assert!(probe.0[0].is_final);
        assert_eq!(
            probe.0[0].source,
            DreamTransferVisualSource::Sleeping {
                origin: Vec2::new(12.0, 34.0)
            }
        );
    }

    #[test]
    fn sleeping_partial_drain_is_not_marked_final() {
        let mut app = driver_test_app(1);

        app.world_mut().spawn((
            Transform::default(),
            DamnedSoul {
                dream: 1.0,
                ..default()
            },
            IdleState {
                behavior: IdleBehavior::Sleeping,
                ..default()
            },
            DreamState::default(),
            AssignedTask::None,
        ));

        app.update();

        let probe = app.world().resource::<TransferProbe>();
        assert_eq!(probe.0.len(), 1);
        assert!(!probe.0[0].is_final);
    }

    #[test]
    fn rest_area_final_drain_keeps_captured_anchor_after_exit() {
        let mut app = driver_test_app(MAX_SLOW_SIMULATION_STEPS_PER_FRAME);
        let rest_area = app
            .world_mut()
            .spawn((
                Transform::from_xyz(80.0, 96.0, 0.0),
                RestAreaOccupants::default(),
            ))
            .id();
        let soul = app
            .world_mut()
            .spawn((
                Transform::from_xyz(4.0, 6.0, 0.0),
                DamnedSoul {
                    dream: 0.1,
                    ..default()
                },
                IdleState {
                    behavior: IdleBehavior::Resting,
                    ..default()
                },
                DreamState::default(),
                AssignedTask::None,
                RestingIn(rest_area),
            ))
            .id();

        app.update();

        app.world_mut().entity_mut(soul).remove::<RestingIn>();
        assert!(app.world().get::<RestingIn>(soul).is_none());

        let pool = app.world().resource::<DreamPool>();
        let probe = app.world().resource::<TransferProbe>();
        assert!((pool.points - 0.1).abs() <= 1e-5);
        assert_eq!(probe.0.len(), 1);
        assert_eq!(probe.0[0].soul, soul);
        assert!((probe.0[0].amount - pool.points).abs() <= 1e-5);
        assert_eq!(probe.0[0].quality, DreamQuality::VividDream);
        assert!(probe.0[0].is_final);
        assert_eq!(
            probe.0[0].source,
            DreamTransferVisualSource::RestArea {
                rest_area,
                origin: Vec2::new(80.0, 96.0),
            }
        );
    }

    #[test]
    fn dream_quality_does_not_change_transfer_amount() {
        let mut app = driver_test_app(MAX_SLOW_SIMULATION_STEPS_PER_FRAME);
        let normal = app
            .world_mut()
            .spawn((
                Transform::default(),
                DamnedSoul {
                    dream: 0.25,
                    stress: 0.5,
                    ..default()
                },
                IdleState {
                    behavior: IdleBehavior::Sleeping,
                    ..default()
                },
                DreamState::default(),
                AssignedTask::None,
            ))
            .id();
        let terror = app
            .world_mut()
            .spawn((
                Transform::default(),
                DamnedSoul {
                    dream: 0.25,
                    stress: 0.8,
                    ..default()
                },
                IdleState {
                    behavior: IdleBehavior::Sleeping,
                    ..default()
                },
                DreamState::default(),
                AssignedTask::None,
            ))
            .id();

        app.update();

        let probe = app.world().resource::<TransferProbe>();
        let normal = probe
            .0
            .iter()
            .find(|message| message.soul == normal)
            .unwrap();
        let terror = probe
            .0
            .iter()
            .find(|message| message.soul == terror)
            .unwrap();
        assert_eq!(normal.quality, DreamQuality::NormalDream);
        assert_eq!(terror.quality, DreamQuality::NightTerror);
        assert!((normal.amount - terror.amount).abs() <= 1e-5);
    }
}
