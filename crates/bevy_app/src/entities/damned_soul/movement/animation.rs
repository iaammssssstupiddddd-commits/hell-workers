//! ソウルのアニメーション（スプライト選択・浮遊揺れ）

use crate::assets::GameAssets;
use crate::entities::damned_soul::{
    ConversationExpression, ConversationExpressionKind, GatheringBehavior, IdleBehavior, IdleState,
    StressBreakdown,
};
use crate::systems::soul_ai::execute::task_execution::AssignedTask;
use bevy::prelude::*;

pub(crate) fn resolve_soul_billboard_frame(
    idle: &IdleState,
    breakdown_opt: Option<&StressBreakdown>,
    expression_opt: Option<&ConversationExpression>,
    is_working_or_moving: bool,
) -> hw_visual::SoulBillboardFrame {
    if let Some(breakdown) = breakdown_opt {
        if breakdown.is_frozen {
            return hw_visual::SoulBillboardFrame::StressBreakdown;
        }
        return hw_visual::SoulBillboardFrame::Stress;
    }

    if let Some(expression) = expression_opt {
        match expression.kind {
            ConversationExpressionKind::Positive => return hw_visual::SoulBillboardFrame::Happy,
            ConversationExpressionKind::Negative => return hw_visual::SoulBillboardFrame::Stress,
            ConversationExpressionKind::Exhausted => {
                return hw_visual::SoulBillboardFrame::Exhausted;
            }
            ConversationExpressionKind::GatheringWine => {
                return hw_visual::SoulBillboardFrame::Wine;
            }
            ConversationExpressionKind::GatheringTrump => {
                return hw_visual::SoulBillboardFrame::Trump;
            }
        }
    }

    let allow_sleep_visual = !is_working_or_moving;

    match idle.behavior {
        IdleBehavior::Sleeping | IdleBehavior::Resting if allow_sleep_visual => {
            hw_visual::SoulBillboardFrame::Sleep
        }
        IdleBehavior::Sleeping | IdleBehavior::Resting => hw_visual::SoulBillboardFrame::Normal,
        IdleBehavior::GoingToRest => hw_visual::SoulBillboardFrame::Normal,
        IdleBehavior::ExhaustedGathering => hw_visual::SoulBillboardFrame::Exhausted,
        IdleBehavior::Escaping => hw_visual::SoulBillboardFrame::Normal,
        IdleBehavior::Drifting => hw_visual::SoulBillboardFrame::Normal,
        IdleBehavior::Gathering => match idle.gathering_behavior {
            GatheringBehavior::Sleeping if allow_sleep_visual => {
                hw_visual::SoulBillboardFrame::Sleep
            }
            GatheringBehavior::Sleeping
            | GatheringBehavior::Wandering
            | GatheringBehavior::Standing
            | GatheringBehavior::Dancing => hw_visual::SoulBillboardFrame::Normal,
        },
        IdleBehavior::Wandering | IdleBehavior::Sitting => hw_visual::SoulBillboardFrame::Normal,
    }
}

fn image_for_frame(
    game_assets: &GameAssets,
    frame: hw_visual::SoulBillboardFrame,
) -> &Handle<Image> {
    match frame {
        hw_visual::SoulBillboardFrame::Normal => &game_assets.soul,
        hw_visual::SoulBillboardFrame::Exhausted => &game_assets.soul_exhausted,
        hw_visual::SoulBillboardFrame::Happy => &game_assets.soul_lough,
        hw_visual::SoulBillboardFrame::Sleep => &game_assets.soul_sleep,
        hw_visual::SoulBillboardFrame::Wine => &game_assets.soul_wine,
        hw_visual::SoulBillboardFrame::Trump => &game_assets.soul_trump,
        hw_visual::SoulBillboardFrame::Stress => &game_assets.soul_stress,
        hw_visual::SoulBillboardFrame::StressBreakdown => &game_assets.soul_stress_breakdown,
    }
}

type AnimationQuery<'w, 's> = Query<
    'w,
    's,
    (
        Option<&'static mut Sprite>,
        &'static mut crate::entities::damned_soul::AnimationState,
        &'static IdleState,
        &'static AssignedTask,
        Option<&'static StressBreakdown>,
        Option<&'static ConversationExpression>,
    ),
>;

/// アニメーションシステム
pub fn animation_system(time: Res<Time>, game_assets: Res<GameAssets>, mut query: AnimationQuery) {
    for (sprite_opt, mut anim, idle, task, breakdown_opt, expression_opt) in query.iter_mut() {
        if let Some(mut sprite) = sprite_opt {
            // 進行方向に応じて左右反転（facing_right は movement 側で更新）
            sprite.flip_x = anim.facing_right;
            let is_working_or_moving = !matches!(*task, AssignedTask::None) || anim.is_moving;
            let desired_frame = resolve_soul_billboard_frame(
                idle,
                breakdown_opt,
                expression_opt,
                is_working_or_moving,
            );
            let desired_image = image_for_frame(&game_assets, desired_frame);
            if sprite.image != *desired_image {
                sprite.image = desired_image.clone();
            }
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
            // Wheelbarrow 等が読む visual-only timer は残す。Soul の通常表示は
            // root Transform を読まない独立 GLB proxy なので、scale / rotation
            // の書き込みは行わない。
            anim.bob_timer += time.delta_secs();
        }
    }
}
