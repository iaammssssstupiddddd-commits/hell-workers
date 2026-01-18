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
use std::collections::HashMap;

/// 激励のクールダウン管理リソース
#[derive(Resource, Default)]
pub struct EncouragementCooldowns {
    /// Soul Entity -> 次回激励可能になる時間
    pub cooldowns: HashMap<Entity, f32>,
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
    q_souls: Query<Entity, With<DamnedSoul>>,
    soul_grid: Res<SpatialGrid>,
    mut cooldowns: ResMut<EncouragementCooldowns>,
) {
    let current_time = time.elapsed_secs();
    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();

    // クールダウンのクリーンアップ（古いエントリを削除）
    cooldowns
        .cooldowns
        .retain(|_, expiry| *expiry > current_time);

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
                // クールダウン中は除外
                if cooldowns.cooldowns.contains_key(&soul_entity) {
                    return None;
                }

                // Soulエンティティであることを確認
                if q_souls.get(soul_entity).is_err() {
                    return None;
                }

                Some(soul_entity)
            })
            .collect();

        if let Some(&target_soul) = valid_targets.choose(&mut rng) {
            // イベント発火
            commands.trigger(OnEncouraged {
                familiar_entity: fam_entity,
                soul_entity: target_soul,
            });

            // クールダウン設定
            cooldowns
                .cooldowns
                .insert(target_soul, current_time + ENCOURAGEMENT_COOLDOWN);

            // 1フレームにつき1体まで激励
            break;
        }
    }
}
