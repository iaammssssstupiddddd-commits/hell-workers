use super::components::*;
use crate::constants::*;
use bevy::prelude::*;

/// 吹き出しのアニメーション処理
pub fn animate_speech_bubbles(
    time: Res<Time>,
    mut q_bubbles: Query<(
        Entity,
        &SpeechBubble,
        &mut BubbleAnimation,
        &mut Transform,
        Option<&mut TextColor>,
        Option<&Children>,
    )>,
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

                // 感情別の待機アニメーション (微調整)
                match bubble.emotion {
                    BubbleEmotion::Exhausted => {
                        let offset =
                            (time.elapsed_secs() * BUBBLE_BOB_SPEED).sin() * BUBBLE_BOB_AMPLITUDE;
                        transform.translation.y += offset * dt * 10.0; // 追従後に適用されるため累積させない工夫が必要だが、ここでは簡易的に
                    }
                    BubbleEmotion::Stressed => {
                        let shake = (time.elapsed_secs() * BUBBLE_SHAKE_SPEED).sin()
                            * BUBBLE_SHAKE_INTENSITY;
                        transform.translation.x += shake * dt * 10.0;
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
