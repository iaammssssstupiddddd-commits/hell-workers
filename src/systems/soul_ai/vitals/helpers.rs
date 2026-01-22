use crate::constants::*;
use crate::entities::familiar::{ActiveCommand, Familiar, FamiliarCommand};
use bevy::prelude::*;

/// 魂の位置における、付近の使い魔たちからの最大影響度を計算する
pub fn calculate_best_influence(
    soul_pos: Vec2,
    nearby_familiar_entities: &[Entity],
    q_familiars: &Query<(&Transform, &Familiar, &ActiveCommand)>,
) -> f32 {
    nearby_familiar_entities
        .iter()
        .filter_map(|&fam_entity| {
            let Ok((fam_transform, familiar, command)) = q_familiars.get(fam_entity) else {
                return None;
            };
            let influence_center = fam_transform.translation.truncate();
            let distance_sq = soul_pos.distance_squared(influence_center);
            let radius_sq = familiar.command_radius * familiar.command_radius;

            if distance_sq < radius_sq {
                let distance = distance_sq.sqrt();
                let command_multiplier = if matches!(command.command, FamiliarCommand::Idle) {
                    SUPERVISION_IDLE_MULTIPLIER
                } else {
                    1.0
                };
                let distance_factor = 1.0 - (distance / familiar.command_radius);
                Some(familiar.efficiency * distance_factor * command_multiplier)
            } else {
                None
            }
        })
        .fold(0.0_f32, |acc, x| acc.max(x))
}
