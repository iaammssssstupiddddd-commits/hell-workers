use super::components::*;
use bevy::prelude::*;
use hw_core::constants::*;

type SpeechBubblesQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static SpeechBubble,
        &'static mut BubbleAnimation,
        &'static mut Transform,
        Option<&'static mut TextColor>,
        Option<&'static Children>,
    ),
>;

/// 吹き出しのアニメーション処理
pub fn animate_speech_bubbles(
    time: Res<Time>,
    mut q_bubbles: SpeechBubblesQuery,
    mut q_sprites: Query<&mut Sprite>,
) {
    let dt = time.delta_secs();

    for (_entity, bubble, mut anim, mut transform, text_color, children) in q_bubbles.iter_mut() {
        anim.elapsed += dt;

        match anim.phase {
            AnimationPhase::PopIn => {
                let progress = (anim.elapsed / BUBBLE_ANIM_POP_IN_DURATION).clamp(0.0, 1.0);

                // バウンス効果: 0 -> 1.2 -> 1.0
                let scale = if progress < 0.7 {
                    let p = progress / 0.7;
                    p * BUBBLE_ANIM_POP_IN_OVERSHOOT
                } else {
                    let p = (progress - 0.7) / 0.3;
                    BUBBLE_ANIM_POP_IN_OVERSHOOT - p * (BUBBLE_ANIM_POP_IN_OVERSHOOT - 1.0)
                };

                transform.scale = Vec3::splat(scale);

                if progress >= 1.0 {
                    anim.phase = AnimationPhase::Idle;
                    anim.elapsed = 0.0;
                }
            }
            AnimationPhase::Idle => {
                transform.scale = Vec3::ONE;
                transform.translation.x = bubble.offset.x;
                transform.translation.y = bubble.offset.y;

                // 感情別の待機アニメーション。基準offsetから絶対量を適用し、
                // frame deltaや前frameのTransformを振幅へ混ぜない。
                match bubble.emotion {
                    BubbleEmotion::Exhausted => {
                        let offset =
                            (time.elapsed_secs() * BUBBLE_BOB_SPEED).sin() * BUBBLE_BOB_AMPLITUDE;
                        transform.translation.y += offset;
                    }
                    BubbleEmotion::Stressed => {
                        let shake = (time.elapsed_secs() * BUBBLE_SHAKE_SPEED).sin()
                            * BUBBLE_SHAKE_INTENSITY;
                        transform.translation.x += shake;
                    }
                    _ => {}
                }

                // 残り時間チェックで PopOut へ移行
                if bubble.elapsed >= bubble.duration - BUBBLE_ANIM_POP_OUT_DURATION {
                    anim.phase = AnimationPhase::PopOut;
                    anim.elapsed = 0.0;
                }
            }
            AnimationPhase::PopOut => {
                let progress = (anim.elapsed / BUBBLE_ANIM_POP_OUT_DURATION).clamp(0.0, 1.0);
                let scale = 1.0 - progress;
                transform.scale = Vec3::splat(scale);

                // テキストのフェードアウト
                if let Some(mut color) = text_color {
                    color.0.set_alpha(1.0 - progress);
                }

                // 子エンティティ（背景スプライト）のフェードアウト
                if let Some(children) = children {
                    for &child in children {
                        if let Ok(mut sprite) = q_sprites.get_mut(child) {
                            let mut color = sprite.color;
                            color.set_alpha((1.0 - progress) * 0.85);
                            sprite.color = color;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn idle_bubble_translation(emotion: BubbleEmotion, elapsed: f32, delta: f32) -> Vec3 {
        let mut app = App::new();
        app.insert_resource(Time::<()>::default())
            .add_systems(Update, animate_speech_bubbles);
        let base = Vec3::new(17.0, 23.0, 0.0);
        let bubble = app
            .world_mut()
            .spawn((
                SpeechBubble {
                    elapsed: 0.0,
                    duration: 10.0,
                    speaker: Entity::PLACEHOLDER,
                    offset: base.truncate(),
                    emotion,
                    background: None,
                },
                BubbleAnimation {
                    phase: AnimationPhase::Idle,
                    elapsed: 0.0,
                },
                Transform::from_translation(base),
            ))
            .id();
        {
            let mut time = app.world_mut().resource_mut::<Time<()>>();
            time.advance_by(Duration::from_secs_f32(elapsed - delta));
            time.advance_by(Duration::from_secs_f32(delta));
        }

        app.update();
        app.world().get::<Transform>(bubble).unwrap().translation
    }

    #[test]
    fn idle_emotion_offsets_do_not_depend_on_frame_delta() {
        for (emotion, speed) in [
            (BubbleEmotion::Exhausted, BUBBLE_BOB_SPEED),
            (BubbleEmotion::Stressed, BUBBLE_SHAKE_SPEED),
        ] {
            let elapsed = std::f32::consts::FRAC_PI_2 / speed;
            let slow_frame = idle_bubble_translation(emotion, elapsed, 1.0 / 30.0);
            let fast_frame = idle_bubble_translation(emotion, elapsed, 1.0 / 120.0);
            assert!(
                slow_frame.abs_diff_eq(fast_frame, 0.0001),
                "{emotion:?} offset changed with delta: {slow_frame:?} != {fast_frame:?}"
            );
        }
    }
}
