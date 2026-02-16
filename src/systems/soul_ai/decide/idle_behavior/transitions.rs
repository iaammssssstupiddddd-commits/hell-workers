//! IdleBehavior 遷移判定

use rand::Rng;

use crate::constants::*;
use crate::entities::damned_soul::{GatheringBehavior, IdleBehavior};

/// ランダムな集会中のサブ行動を選択
pub fn random_gathering_behavior() -> GatheringBehavior {
    let mut rng = rand::thread_rng();
    match rng.gen_range(0..4) {
        0 => GatheringBehavior::Wandering,
        1 => GatheringBehavior::Sleeping,
        2 => GatheringBehavior::Standing,
        _ => GatheringBehavior::Dancing,
    }
}

/// ランダムな集会行動の持続時間を取得
pub fn random_gathering_duration() -> f32 {
    let mut rng = rand::thread_rng();
    rng.gen_range(GATHERING_BEHAVIOR_DURATION_MIN..GATHERING_BEHAVIOR_DURATION_MAX)
}

/// 待機行動の持続時間を取得
pub fn behavior_duration_for(behavior: IdleBehavior) -> f32 {
    let mut rng = rand::thread_rng();
    match behavior {
        IdleBehavior::Sleeping => {
            rng.gen_range(IDLE_DURATION_SLEEP_MIN..IDLE_DURATION_SLEEP_MAX)
        }
        IdleBehavior::Sitting => {
            rng.gen_range(IDLE_DURATION_SIT_MIN..IDLE_DURATION_SIT_MAX)
        }
        IdleBehavior::Wandering => {
            rng.gen_range(IDLE_DURATION_WANDER_MIN..IDLE_DURATION_WANDER_MAX)
        }
        IdleBehavior::Gathering | IdleBehavior::ExhaustedGathering => {
            rng.gen_range(IDLE_DURATION_WANDER_MIN..IDLE_DURATION_WANDER_MAX)
        }
        IdleBehavior::Resting | IdleBehavior::GoingToRest => REST_AREA_RESTING_DURATION,
        IdleBehavior::Escaping => 2.0,
        IdleBehavior::Drifting => rng.gen_range(DRIFT_WANDER_DURATION_MIN..DRIFT_WANDER_DURATION_MAX),
    }
}

/// 次の IdleBehavior を選択（laziness に基づく）
pub fn select_next_behavior(
    laziness: f32,
    _fatigue: f32,
    _total_idle_time: f32,
) -> IdleBehavior {
    let mut rng = rand::thread_rng();
    let roll: f32 = rng.gen_range(0.0..1.0);

    if laziness > LAZINESS_THRESHOLD_HIGH {
        if roll < 0.6 {
            IdleBehavior::Sleeping
        } else if roll < 0.9 {
            IdleBehavior::Sitting
        } else {
            IdleBehavior::Wandering
        }
    } else if laziness > LAZINESS_THRESHOLD_MID {
        if roll < 0.3 {
            IdleBehavior::Sleeping
        } else if roll < 0.6 {
            IdleBehavior::Sitting
        } else {
            IdleBehavior::Wandering
        }
    } else if roll < 0.7 {
        IdleBehavior::Wandering
    } else {
        IdleBehavior::Sitting
    }
}
