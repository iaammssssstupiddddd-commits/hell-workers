use bevy::prelude::*;
use std::collections::HashSet;

use hw_core::constants::{
    DREAM_DRAIN_RATE_REST, REST_AREA_FATIGUE_RECOVERY_RATE, REST_AREA_RESTING_DURATION,
    REST_AREA_STRESS_RECOVERY_RATE,
};
use hw_core::events::{IdleBehaviorOperation, IdleBehaviorRequest};
use hw_core::relationships::RestingIn;
use hw_core::soul::{DamnedSoul, DreamPool, IdleBehavior, IdleState, RestAreaCooldown};

use super::slow_simulation::SlowSimulationClock;

pub(crate) type RestingSoulQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static mut DamnedSoul, &'static mut IdleState), With<RestingIn>>;

pub(crate) type RestCooldownQuery<'w, 's> = Query<'w, 's, (Entity, &'static mut RestAreaCooldown)>;

/// 休憩所の滞在効果を更新する（Dream放出、バイタル回復、自動退出、クールダウン）
pub fn rest_area_update_system(
    clock: Res<SlowSimulationClock>,
    mut commands: Commands,
    mut dream_pool: ResMut<DreamPool>,
    mut request_writer: MessageWriter<IdleBehaviorRequest>,
    mut exit_requests: Local<HashSet<Entity>>,
    mut q_resting_souls: RestingSoulQuery,
    mut q_cooldowns: RestCooldownQuery,
) {
    exit_requests.clear();
    for _ in 0..clock.steps_this_frame() {
        rest_area_update_step(
            clock.step_secs(),
            &mut commands,
            &mut dream_pool,
            &mut request_writer,
            &mut exit_requests,
            &mut q_resting_souls,
            &mut q_cooldowns,
        );
    }
}

pub(crate) fn rest_area_update_step(
    dt: f32,
    commands: &mut Commands,
    dream_pool: &mut DreamPool,
    request_writer: &mut MessageWriter<IdleBehaviorRequest>,
    exit_requests: &mut HashSet<Entity>,
    q_resting_souls: &mut RestingSoulQuery,
    q_cooldowns: &mut RestCooldownQuery,
) {
    for (entity, mut soul, mut idle) in q_resting_souls.iter_mut() {
        if idle.behavior != IdleBehavior::Resting || exit_requests.contains(&entity) {
            continue;
        }
        soul.fatigue = (soul.fatigue - dt * REST_AREA_FATIGUE_RECOVERY_RATE).max(0.0);
        soul.stress = (soul.stress - dt * REST_AREA_STRESS_RECOVERY_RATE).max(0.0);

        // per-soul dream放出
        let drain = (DREAM_DRAIN_RATE_REST * dt).min(soul.dream);
        if drain > 0.0 {
            soul.dream -= drain;
            dream_pool.points += drain;
        }

        let previous_idle_timer = idle.idle_timer;
        idle.idle_timer += dt;
        let duration_reached = previous_idle_timer < REST_AREA_RESTING_DURATION
            && idle.idle_timer >= REST_AREA_RESTING_DURATION;
        if (duration_reached || soul.dream <= 0.0) && exit_requests.insert(entity) {
            // A capped catch-up frame can contain five substeps. Emit the
            // one-shot exit only on the threshold crossing, rather than
            // repeatedly while deferred Commands are pending.
            request_writer.write(IdleBehaviorRequest {
                entity,
                operation: IdleBehaviorOperation::LeaveRestArea,
            });
        }
    }

    for (entity, mut cooldown) in q_cooldowns.iter_mut() {
        let was_active = cooldown.remaining_secs > f32::EPSILON;
        cooldown.remaining_secs = (cooldown.remaining_secs - dt).max(0.0);
        if was_active && cooldown.remaining_secs <= f32::EPSILON {
            commands.entity(entity).remove::<RestAreaCooldown>();
        }
    }
}
