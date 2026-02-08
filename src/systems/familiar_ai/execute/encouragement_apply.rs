use bevy::prelude::*;

use crate::constants::ENCOURAGEMENT_COOLDOWN;
use crate::events::{EncouragementRequest, OnEncouraged};
use crate::systems::familiar_ai::decide::encouragement::EncouragementCooldown;

/// 激励要求を適用する（Execute Phase）
pub fn encouragement_apply_system(
    mut commands: Commands,
    time: Res<Time>,
    mut request_reader: MessageReader<EncouragementRequest>,
) {
    let current_time = time.elapsed_secs();

    for request in request_reader.read() {
        commands.trigger(OnEncouraged {
            familiar_entity: request.familiar_entity,
            soul_entity: request.soul_entity,
        });

        commands
            .entity(request.soul_entity)
            .insert(EncouragementCooldown {
                expiry: current_time + ENCOURAGEMENT_COOLDOWN,
            });
    }
}

/// 期限切れのクールダウンを削除するシステム
pub fn cleanup_encouragement_cooldowns_system(
    mut commands: Commands,
    time: Res<Time>,
    q_cooldowns: Query<(Entity, &EncouragementCooldown)>,
) {
    let current_time = time.elapsed_secs();
    for (entity, cooldown) in q_cooldowns.iter() {
        if current_time >= cooldown.expiry {
            commands.entity(entity).remove::<EncouragementCooldown>();
        }
    }
}
