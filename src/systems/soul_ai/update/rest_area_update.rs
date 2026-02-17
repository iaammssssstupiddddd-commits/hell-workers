use bevy::prelude::*;

use crate::constants::{
    REST_AREA_DREAM_RATE, REST_AREA_FATIGUE_RECOVERY_RATE, REST_AREA_RESTING_DURATION,
    REST_AREA_STRESS_RECOVERY_RATE,
};
use crate::entities::damned_soul::{
    DamnedSoul, DreamPool, IdleBehavior, IdleState, RestAreaCooldown,
};
use crate::events::{IdleBehaviorOperation, IdleBehaviorRequest};
use crate::relationships::{RestAreaOccupants, RestingIn};
use crate::systems::jobs::RestArea;

/// 休憩所の滞在効果を更新する（Dream生成、バイタル回復、自動退出、クールダウン）
pub fn rest_area_update_system(
    time: Res<Time>,
    mut commands: Commands,
    mut dream_pool: ResMut<DreamPool>,
    mut request_writer: MessageWriter<IdleBehaviorRequest>,
    q_rest_areas: Query<(&RestArea, Option<&RestAreaOccupants>)>,
    mut q_resting_souls: Query<(Entity, &mut DamnedSoul, &mut IdleState), With<RestingIn>>,
    mut q_cooldowns: Query<(Entity, &mut RestAreaCooldown)>,
) {
    let dt = time.delta_secs();

    for (rest_area, occupants_opt) in q_rest_areas.iter() {
        let occupant_count = occupants_opt
            .map(crate::relationships::RestAreaOccupants::len)
            .unwrap_or(0)
            .min(rest_area.capacity);
        if occupant_count > 0 {
            dream_pool.points += occupant_count as f32 * REST_AREA_DREAM_RATE * dt;
        }
    }

    for (entity, mut soul, mut idle) in q_resting_souls.iter_mut() {
        if idle.behavior != IdleBehavior::Resting {
            continue;
        }
        soul.fatigue = (soul.fatigue - dt * REST_AREA_FATIGUE_RECOVERY_RATE).max(0.0);
        soul.stress = (soul.stress - dt * REST_AREA_STRESS_RECOVERY_RATE).max(0.0);

        idle.idle_timer += dt;
        if idle.idle_timer >= REST_AREA_RESTING_DURATION {
            request_writer.write(IdleBehaviorRequest {
                entity,
                operation: IdleBehaviorOperation::LeaveRestArea,
            });
        }
    }

    for (entity, mut cooldown) in q_cooldowns.iter_mut() {
        cooldown.remaining_secs = (cooldown.remaining_secs - dt).max(0.0);
        if cooldown.remaining_secs <= f32::EPSILON {
            commands.entity(entity).remove::<RestAreaCooldown>();
        }
    }
}
