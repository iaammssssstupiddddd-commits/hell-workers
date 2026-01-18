use crate::assets::GameAssets;
use crate::constants::*;
use crate::entities::damned_soul::{DamnedSoul, IdleBehavior, IdleState};
use crate::entities::familiar::UnderCommand;
use crate::systems::visual::speech::components::{BubbleEmotion, BubblePriority, SoulEmotionState};
use crate::systems::visual::speech::spawn::spawn_soul_bubble;
use bevy::prelude::*;
use rand::Rng;

/// 定期的に Soul の感情状態をチェックし、必要に応じて吹き出しを出すシステム
pub fn periodic_emotion_system(
    time: Res<Time>,
    mut commands: Commands,
    assets: Res<GameAssets>,
    mut query: Query<(
        Entity,
        &GlobalTransform,
        &DamnedSoul,
        &IdleState,
        Option<&UnderCommand>,
        &mut SoulEmotionState,
    )>,
) {
    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();

    for (entity, transform, soul, idle, under_command_opt, mut state) in query.iter_mut() {
        // タイマー更新
        state.tick(dt);

        // アイドル時間の更新
        if under_command_opt.is_none() && idle.behavior != IdleBehavior::Gathering {
            state.idle_time += dt;
        } else {
            state.idle_time = 0.0;
        }

        // ロック中ならスキップ
        if !state.is_ready(&time) {
            continue;
        }

        let mut triggered = None;

        // 優先順位付き判定 (if-else chain で排他的に)

        // 1. ストレス (Critical/High)
        if soul.stress > EMOTION_THRESHOLD_STRESSED {
            if rng.gen_bool(PROBABILITY_PERIODIC_STRESSED as f64) {
                triggered = Some(("😰", BubbleEmotion::Stressed, BubblePriority::High));
            }
        }
        // 2. 疲労 (High)
        else if soul.fatigue > EMOTION_THRESHOLD_EXHAUSTED {
            if rng.gen_bool(PROBABILITY_PERIODIC_EXHAUSTED as f64) {
                triggered = Some(("😴", BubbleEmotion::Exhausted, BubblePriority::High));
            }
        }
        // 3. やる気低下 (Low) - 使役中のみ
        else if under_command_opt.is_some() && soul.motivation < EMOTION_THRESHOLD_UNMOTIVATED {
            if rng.gen_bool(PROBABILITY_PERIODIC_UNMOTIVATED as f64) {
                triggered = Some(("😒", BubbleEmotion::Unmotivated, BubblePriority::Low));
            }
        }
        // 4. アイドル (Low)
        else if state.idle_time > IDLE_EMOTION_MIN_DURATION {
            if rng.gen_bool(PROBABILITY_PERIODIC_BORED as f64) {
                let emoji = match rng.gen_range(0..3) {
                    0 => "💤",
                    1 => "🥱",
                    _ => "😑",
                };
                triggered = Some((emoji, BubbleEmotion::Bored, BubblePriority::Low));
            }
        }

        // 発火処理
        if let Some((emoji, emotion, priority)) = triggered {
            spawn_soul_bubble(
                &mut commands,
                entity,
                emoji,
                transform.translation(),
                &assets,
                emotion,
                priority,
            );
            // 判定間隔に関わらず、一度出たら一定時間ロックする（定数で管理）
            state.lock(PERIODIC_EMOTION_LOCK_DURATION);
        }
    }
}
