//! Dream蓄積システム
//!
//! 起きているSoulにはdreamが行動に応じて蓄積し、睡眠中はDreamPoolへ放出する。
//! 夢の質はビジュアル用として維持される（放出レートには影響しない）。

use bevy::prelude::*;

use crate::constants::*;
use crate::entities::damned_soul::{
    DamnedSoul, DreamPool, DreamQuality, DreamState, GatheringBehavior, IdleBehavior, IdleState,
};
use crate::relationships::ParticipatingIn;
use crate::systems::soul_ai::execute::task_execution::AssignedTask;

/// SoulのDream蓄積・放出を処理するシステム
pub fn dream_update_system(
    time: Res<Time>,
    mut dream_pool: ResMut<DreamPool>,
    mut q_souls: Query<(
        &mut DamnedSoul,
        &IdleState,
        &mut DreamState,
        &AssignedTask,
        Option<&ParticipatingIn>,
    )>,
) {
    let dt = time.delta_secs();

    for (mut soul, idle, mut dream, task, participating_in) in q_souls.iter_mut() {
        let is_sleeping = idle.behavior == IdleBehavior::Sleeping
            || (idle.behavior == IdleBehavior::Gathering
                && idle.gathering_behavior == GatheringBehavior::Sleeping
                && participating_in.is_some());
        let is_resting = idle.behavior == IdleBehavior::Resting;

        if !is_sleeping {
            // 起きている（休憩中含む）はAwakeにリセット
            if dream.quality != DreamQuality::Awake {
                dream.quality = DreamQuality::Awake;
            }
            if !is_resting {
                // 非睡眠・非休憩中はdream蓄積
                let has_task = !matches!(*task, AssignedTask::None);
                let rate = if has_task {
                    DREAM_ACCUMULATE_RATE_WORKING
                } else {
                    match idle.behavior {
                        IdleBehavior::Gathering | IdleBehavior::ExhaustedGathering => {
                            DREAM_ACCUMULATE_RATE_GATHERING
                        }
                        IdleBehavior::Escaping => DREAM_ACCUMULATE_RATE_ESCAPING,
                        _ => DREAM_ACCUMULATE_RATE_IDLE,
                    }
                };
                soul.dream = (soul.dream + rate * dt).min(DREAM_MAX);
            }
            continue;
        }

        // 睡眠中: DreamQuality判定（ビジュアル用、放出レートには影響しない）
        if dream.quality == DreamQuality::Awake {
            dream.quality = determine_dream_quality(&soul, participating_in.is_some());
        }

        // 一律レートでsoul.dreamをDreamPoolへ放出
        let drain = (DREAM_DRAIN_RATE * dt).min(soul.dream);
        if drain > 0.0 {
            soul.dream -= drain;
            dream_pool.points += drain;
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
