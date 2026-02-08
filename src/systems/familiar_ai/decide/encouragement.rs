//! 使い魔による激励システム
//!
//! 監視中の使い魔が、範囲内のソウルを激励（Encourage）し、
//! モチベーションを回復させつつ、プレッシャー（ストレス）を与えるシステム。

use crate::constants::*;
use crate::entities::damned_soul::DamnedSoul;
use crate::entities::familiar::{ActiveCommand, Familiar, FamiliarCommand};
use crate::events::OnEncouraged;
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

/// 激励システム
pub fn encouragement_system(
    mut commands: Commands,
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
) {
    let current_time = time.elapsed_secs();
    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();

    for (fam_entity, fam_transform, familiar, state, active_cmd) in q_familiars.iter() {
        // 監視モード中のみ有効
        if !matches!(state, FamiliarAiState::Supervising { .. }) {
            continue;
        }
        // Idle命令中は行わない（プレイヤー操作優先）
        if matches!(active_cmd.command, FamiliarCommand::Idle) {
            continue;
        }

        // ランダムなタイミング判定
        let check_chance = dt / ((ENCOURAGEMENT_INTERVAL_MIN + ENCOURAGEMENT_INTERVAL_MAX) / 2.0);

        if !rng.gen_bool(check_chance.clamp(0.0, 1.0) as f64) {
            continue;
        }

        // 激励チャンス到来：対象を探す
        let fam_pos = fam_transform.translation().truncate();
        let search_radius = familiar.command_radius;
        let nearby = soul_grid.get_nearby_in_radius(fam_pos, search_radius);

        // 有効なターゲット候補を抽出
        let valid_targets: Vec<Entity> = nearby
            .iter()
            .filter_map(|&soul_entity| {
                // Soulエンティティであることを確認し、クールダウン中でないことを確認
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
            // イベント発火
            commands.trigger(OnEncouraged {
                familiar_entity: fam_entity,
                soul_entity: target_soul,
            });

            // クールダウン設定
            commands.entity(target_soul).insert(EncouragementCooldown {
                expiry: current_time + ENCOURAGEMENT_COOLDOWN,
            });

            // 1フレームにつき1体まで激励
            break;
        }
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
