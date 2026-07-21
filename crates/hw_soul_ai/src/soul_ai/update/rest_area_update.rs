use bevy::prelude::*;
use std::collections::HashSet;

use hw_core::constants::{
    DREAM_DRAIN_RATE_REST, REST_AREA_FATIGUE_RECOVERY_RATE, REST_AREA_RESTING_DURATION,
    REST_AREA_STRESS_RECOVERY_RATE,
};
use hw_core::events::{DreamTransferVisualSource, IdleBehaviorOperation, IdleBehaviorRequest};
use hw_core::relationships::RestingIn;
use hw_core::soul::{
    DamnedSoul, DreamPool, DreamQuality, IdleBehavior, IdleState, RestAreaCooldown,
};

use super::slow_simulation::DreamTransferAccumulator;

pub(crate) type RestingSoulQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut DamnedSoul,
        &'static mut IdleState,
        &'static RestingIn,
    ),
>;

pub(crate) type RestCooldownQuery<'w, 's> = Query<'w, 's, (Entity, &'static mut RestAreaCooldown)>;

pub(crate) struct RestDreamTransferContext<'a, 'w, 's> {
    pub dream_pool: &'a mut DreamPool,
    pub transfers: &'a mut DreamTransferAccumulator,
    pub q_transforms: &'a Query<'w, 's, &'static Transform>,
}

pub(crate) fn rest_area_update_step(
    dt: f32,
    commands: &mut Commands,
    request_writer: &mut MessageWriter<IdleBehaviorRequest>,
    exit_requests: &mut HashSet<Entity>,
    transfer: &mut RestDreamTransferContext,
    q_resting_souls: &mut RestingSoulQuery,
    q_cooldowns: &mut RestCooldownQuery,
) {
    for (entity, mut soul, mut idle, resting_in) in q_resting_souls.iter_mut() {
        if idle.behavior != IdleBehavior::Resting || exit_requests.contains(&entity) {
            continue;
        }
        soul.fatigue = (soul.fatigue - dt * REST_AREA_FATIGUE_RECOVERY_RATE).max(0.0);
        soul.stress = (soul.stress - dt * REST_AREA_STRESS_RECOVERY_RATE).max(0.0);

        let previous_idle_timer = idle.idle_timer;
        idle.idle_timer += dt;
        let duration_reached = previous_idle_timer < REST_AREA_RESTING_DURATION
            && idle.idle_timer >= REST_AREA_RESTING_DURATION;

        // per-soul dream放出
        let drain = (DREAM_DRAIN_RATE_REST * dt).min(soul.dream);
        if drain > 0.0 {
            let is_final = duration_reached || drain >= soul.dream;
            soul.dream -= drain;
            transfer.dream_pool.points += drain;
            let origin = transfer
                .q_transforms
                .get(resting_in.0)
                .or_else(|_| transfer.q_transforms.get(entity))
                .map_or(Vec2::ZERO, |transform| transform.translation.truncate());
            transfer.transfers.record(
                entity,
                drain,
                DreamQuality::VividDream,
                DreamTransferVisualSource::RestArea {
                    rest_area: resting_in.0,
                    origin,
                },
                is_final,
            );
        }

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
