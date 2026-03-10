use super::components::{BubbleEmotion, BubblePriority, SoulEmotionState};
use super::spawn::spawn_soul_bubble;
use crate::handles::SpeechHandles;
use bevy::prelude::*;
use hw_core::constants::*;
use hw_core::relationships::CommandedBy;
use hw_core::soul::{DamnedSoul, IdleBehavior, IdleState};
use rand::Rng;

/// 分散実行用のフレームカウンタ
#[derive(Resource, Default)]
pub struct PeriodicEmotionFrameCounter(pub u32);

/// 定期的に Soul の感情状態をチェックし、必要に応じて吹き出しを出すシステム
/// パフォーマンス最適化: 毎フレーム全Soulをチェックせず、フレームごとに一部のみ処理
pub fn periodic_emotion_system(
    time: Res<Time>,
    mut commands: Commands,
    handles: Res<SpeechHandles>,
    mut frame_counter: ResMut<PeriodicEmotionFrameCounter>,
    mut query: Query<(
        Entity,
        &GlobalTransform,
        &DamnedSoul,
        &IdleState,
        Option<&CommandedBy>,
        &mut SoulEmotionState,
    )>,
) {
    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();

    // フレームカウンタをインクリメント（10フレームで1周）
    frame_counter.0 = (frame_counter.0 + 1) % PERIODIC_EMOTION_FRAME_DIVISOR;
    let current_frame = frame_counter.0;

    for (entity, transform, soul, idle, under_command_opt, state) in query.iter_mut() {
        let (entity, transform, soul, idle, under_command_opt, mut state): (
            Entity,
            &GlobalTransform,
            &DamnedSoul,
            &IdleState,
            Option<&CommandedBy>,
            Mut<SoulEmotionState>,
        ) = (entity, transform, soul, idle, under_command_opt, state);
        // 分散実行: エンティティのインデックスに基づいてフレームを分散
        // 全Soulを PERIODIC_EMOTION_FRAME_DIVISOR フレームかけて巡回
        if (entity.to_bits() as u32) % PERIODIC_EMOTION_FRAME_DIVISOR != current_frame {
            // このフレームでは処理しないが、タイマーは更新する必要がある
            state.tick(dt);
            // アイドル時間も更新
            if under_command_opt.is_none()
                && !matches!(
                    idle.behavior,
                    IdleBehavior::Gathering | IdleBehavior::Resting | IdleBehavior::GoingToRest
                )
            {
                state.idle_time += dt;
            } else {
                state.idle_time = 0.0;
            }
            continue;
        }

        // タイマー更新
        state.tick(dt);

        // アイドル時間の更新
        if under_command_opt.is_none()
            && !matches!(
                idle.behavior,
                IdleBehavior::Gathering | IdleBehavior::Resting | IdleBehavior::GoingToRest
            )
        {
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
                &handles,
                emotion,
                priority,
            );
            // 判定間隔に関わらず、一度出たら一定時間ロックする（定数で管理）
            state.lock(PERIODIC_EMOTION_LOCK_DURATION);
        }
    }
}
