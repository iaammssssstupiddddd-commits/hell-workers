//! 使い魔による激励システム（Decide）

use crate::constants::*;
use crate::entities::damned_soul::DamnedSoul;
use crate::entities::familiar::{ActiveCommand, Familiar, FamiliarCommand};
use crate::events::EncouragementRequest;
use crate::systems::familiar_ai::FamiliarAiState;
use crate::systems::spatial::{SpatialGrid, SpatialGridOps};
use bevy::prelude::*;
use rand::Rng;
use rand::seq::SliceRandom;

/// 激励のクールダウン管理コンポーネント
#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct EncouragementCooldown {
    /// 次回激励可能になる時間（elapsed_secs）
    pub expiry: f32,
}

/// 激励要求を生成するシステム（Decide Phase）
pub fn encouragement_decision_system(
    time: Res<Time>,
    q_familiars: Query<(
        Entity,
        &GlobalTransform,
        &Familiar,
        &FamiliarAiState,
        &ActiveCommand,
    )>,
    q_souls: Query<(Entity, Has<EncouragementCooldown>), With<DamnedSoul>>,
    soul_grid: Res<SpatialGrid>,
    mut request_writer: MessageWriter<EncouragementRequest>,
) {
    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();

    for (fam_entity, fam_transform, familiar, state, active_cmd) in q_familiars.iter() {
        if !matches!(state, FamiliarAiState::Supervising { .. }) {
            continue;
        }
        if matches!(active_cmd.command, FamiliarCommand::Idle) {
            continue;
        }

        let check_chance = dt / ((ENCOURAGEMENT_INTERVAL_MIN + ENCOURAGEMENT_INTERVAL_MAX) / 2.0);
        if !rng.gen_bool(check_chance.clamp(0.0, 1.0) as f64) {
            continue;
        }

        let fam_pos = fam_transform.translation().truncate();
        let search_radius = familiar.command_radius;
        let nearby = soul_grid.get_nearby_in_radius(fam_pos, search_radius);

        let valid_targets: Vec<Entity> = nearby
            .iter()
            .filter_map(|&soul_entity| {
                if let Ok((entity, has_cooldown)) = q_souls.get(soul_entity) {
                    if has_cooldown {
                        return None;
                    }
                    Some(entity)
                } else {
                    None
                }
            })
            .collect();

        if let Some(&target_soul) = valid_targets.choose(&mut rng) {
            request_writer.write(EncouragementRequest {
                familiar_entity: fam_entity,
                soul_entity: target_soul,
            });
            break;
        }
    }
}
