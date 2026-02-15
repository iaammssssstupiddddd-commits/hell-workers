//! Dream蓄積システム
//!
//! 睡眠中のSoulからDreamポイントをグローバルプールに蓄積する。
//! 夢の質はストレスと集会参加状態で決定される。

use bevy::prelude::*;

use crate::constants::*;
use crate::entities::damned_soul::{
    DamnedSoul, DreamPool, DreamQuality, DreamState, GatheringBehavior, IdleBehavior, IdleState,
};
use crate::relationships::ParticipatingIn;

/// 睡眠中のSoulのDream蓄積を処理するシステム
pub fn dream_update_system(
    time: Res<Time>,
    mut dream_pool: ResMut<DreamPool>,
    mut q_souls: Query<(
        &DamnedSoul,
        &IdleState,
        &mut DreamState,
        Option<&ParticipatingIn>,
    )>,
) {
    let dt = time.delta_secs();

    for (soul, idle, mut dream, participating_in) in q_souls.iter_mut() {
        // 睡眠中かどうかを判定
        let is_sleeping = idle.behavior == IdleBehavior::Sleeping
            || (idle.behavior == IdleBehavior::Gathering
                && idle.gathering_behavior == GatheringBehavior::Sleeping
                && participating_in.is_some());

        if !is_sleeping {
            // 起きている場合はAwakeにリセット
            if dream.quality != DreamQuality::Awake {
                dream.quality = DreamQuality::Awake;
            }
            continue;
        }

        // 睡眠開始時（Awake → 睡眠中）に夢の質を判定
        if dream.quality == DreamQuality::Awake {
            dream.quality = determine_dream_quality(soul, participating_in.is_some());
        }

        // 質に応じたレートでDreamを蓄積
        let rate = match dream.quality {
            DreamQuality::VividDream => DREAM_RATE_VIVID,
            DreamQuality::NormalDream => DREAM_RATE_NORMAL,
            DreamQuality::NightTerror | DreamQuality::Awake => 0.0,
        };

        if rate > 0.0 {
            dream_pool.points += rate * dt;
        }
    }
}

/// ストレスと集会参加状態から夢の質を判定
fn determine_dream_quality(soul: &DamnedSoul, is_in_gathering: bool) -> DreamQuality {
    if soul.stress > DREAM_NIGHTMARE_STRESS_THRESHOLD {
        DreamQuality::NightTerror
    } else if soul.stress < DREAM_VIVID_STRESS_THRESHOLD && is_in_gathering {
        DreamQuality::VividDream
    } else {
        DreamQuality::NormalDream
    }
}
