//! ソウルのアニメーション（スプライト選択・浮遊揺れ）

use crate::assets::GameAssets;
use crate::constants::*;
use crate::entities::damned_soul::{
    ConversationExpression, ConversationExpressionKind, DamnedSoul, GatheringBehavior,
    IdleBehavior, IdleState, StressBreakdown,
};
use bevy::prelude::*;

fn select_soul_image<'a>(
    game_assets: &'a GameAssets,
    idle: &IdleState,
    breakdown_opt: Option<&StressBreakdown>,
    expression_opt: Option<&ConversationExpression>,
) -> &'a Handle<Image> {
    if let Some(breakdown) = breakdown_opt {
        if breakdown.is_frozen {
            return &game_assets.soul_stress_breakdown;
        }
        return &game_assets.soul_stress;
    }

    if let Some(expression) = expression_opt {
        match expression.kind {
            ConversationExpressionKind::Positive => return &game_assets.soul_lough,
            ConversationExpressionKind::Negative => return &game_assets.soul_stress,
            ConversationExpressionKind::Exhausted => return &game_assets.soul_exhausted,
            ConversationExpressionKind::GatheringWine => return &game_assets.soul_wine,
            ConversationExpressionKind::GatheringTrump => return &game_assets.soul_trump,
        }
    }

    match idle.behavior {
        IdleBehavior::Sleeping => &game_assets.soul_sleep,
        IdleBehavior::Resting => &game_assets.soul_sleep,
        IdleBehavior::ExhaustedGathering => &game_assets.soul_exhausted,
        IdleBehavior::Escaping => &game_assets.soul,
        IdleBehavior::Gathering => match idle.gathering_behavior {
            GatheringBehavior::Sleeping => &game_assets.soul_sleep,
            GatheringBehavior::Wandering
            | GatheringBehavior::Standing
            | GatheringBehavior::Dancing => &game_assets.soul,
        },
        IdleBehavior::Wandering | IdleBehavior::Sitting => &game_assets.soul,
    }
}

/// アニメーションシステム
pub fn animation_system(
    time: Res<Time>,
    game_assets: Res<GameAssets>,
    mut query: Query<(
        &mut Transform,
        &mut Sprite,
        &mut crate::entities::damned_soul::AnimationState,
        &DamnedSoul,
        &IdleState,
        Option<&StressBreakdown>,
        Option<&ConversationExpression>,
    )>,
) {
    for (mut transform, mut sprite, mut anim, soul, idle, breakdown_opt, expression_opt) in
        query.iter_mut()
    {
        // 進行方向に応じて左右反転（facing_right は movement 側で更新）
        sprite.flip_x = anim.facing_right;
        let desired_image = select_soul_image(&game_assets, idle, breakdown_opt, expression_opt);
        if sprite.image != *desired_image {
            sprite.image = desired_image.clone();
        }

        // 集会中の特定の行動では、idle_visual_systemがアニメーションを管理するため
        // ここでは通常の浮遊アニメーションをスキップ
        use crate::entities::damned_soul::{GatheringBehavior, IdleBehavior};
        let is_gathering_with_custom_animation = matches!(
            idle.behavior,
            IdleBehavior::Gathering | IdleBehavior::ExhaustedGathering
        ) && matches!(
            idle.gathering_behavior,
            GatheringBehavior::Dancing | GatheringBehavior::Standing | GatheringBehavior::Sleeping
        );

        if !is_gathering_with_custom_animation {
            // 浮遊アニメーション（translation はロジック座標と干渉するため変更しない）
            anim.bob_timer += time.delta_secs();
            let sway = (anim.bob_timer * SOUL_FLOAT_SWAY_SPEED).sin();

            let speed_scale = if anim.is_moving { 1.3 } else { 1.0 };
            let pulse_speed =
                (SOUL_FLOAT_PULSE_SPEED_BASE + (1.0 - soul.laziness) * 0.4) * speed_scale;
            let pulse = (anim.bob_timer * pulse_speed).sin();
            let pulse_amplitude = if anim.is_moving {
                SOUL_FLOAT_PULSE_AMPLITUDE_MOVE
            } else {
                SOUL_FLOAT_PULSE_AMPLITUDE_IDLE
            };
            let base_scale = if anim.is_moving { 1.02 } else { 1.0 };

            transform.scale = Vec3::new(
                base_scale + pulse * (pulse_amplitude * 0.6),
                base_scale + pulse * pulse_amplitude,
                1.0,
            );

            let tilt = if anim.is_moving {
                SOUL_FLOAT_SWAY_TILT_MOVE
            } else {
                SOUL_FLOAT_SWAY_TILT_IDLE
            };
            transform.rotation = Quat::from_rotation_z(sway * tilt);
        }
    }
}
