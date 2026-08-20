use crate::entities::damned_soul::{ConversationExpression, ConversationExpressionKind};
use bevy::prelude::*;
use hw_core::constants::EMOTION_THRESHOLD_EXHAUSTED;
use hw_core::soul::{AnimationState, DamnedSoul, IdleBehavior, IdleState, StressBreakdown};
use hw_core::visual_mirror::task::{SoulTaskPhaseVisual, SoulTaskVisualState};
use hw_visual::{SoulAnimVisualState, SoulBodyAnimState, SoulFaceState};

type SoulAnimOwnerQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut SoulAnimVisualState,
        &'static DamnedSoul,
        &'static IdleState,
        &'static AnimationState,
        &'static SoulTaskVisualState,
        Option<&'static StressBreakdown>,
        Option<&'static ConversationExpression>,
    ),
>;
pub fn sync_soul_anim_visual_state_system(mut q_souls: SoulAnimOwnerQuery) {
    for (mut anim_state, soul, idle, animation, task_visual, breakdown, expression) in &mut q_souls
    {
        let next_body =
            desired_body_state(soul, idle, animation, task_visual, breakdown, expression);
        let next_face =
            desired_face_state(soul, idle, animation, task_visual, breakdown, expression);

        if anim_state.body != next_body || anim_state.face != next_face {
            *anim_state = SoulAnimVisualState {
                body: next_body,
                face: next_face,
            };
        }
    }
}

fn desired_body_state(
    _soul: &DamnedSoul,
    idle: &IdleState,
    animation: &AnimationState,
    task_visual: &SoulTaskVisualState,
    breakdown: Option<&StressBreakdown>,
    _expression: Option<&ConversationExpression>,
) -> SoulBodyAnimState {
    if let Some(breakdown) = breakdown {
        return if breakdown.is_frozen {
            SoulBodyAnimState::Idle
        } else {
            SoulBodyAnimState::Fear
        };
    }

    if matches!(idle.behavior, IdleBehavior::ExhaustedGathering) {
        return SoulBodyAnimState::Exhausted;
    }

    if animation.is_moving {
        return if is_carry_phase(task_visual.phase) {
            SoulBodyAnimState::Carry
        } else {
            SoulBodyAnimState::Walk
        };
    }

    if is_carry_phase(task_visual.phase) {
        return SoulBodyAnimState::Carry;
    }

    if is_work_phase(task_visual.phase) {
        return SoulBodyAnimState::Work;
    }

    SoulBodyAnimState::Idle
}

fn desired_face_state(
    soul: &DamnedSoul,
    idle: &IdleState,
    animation: &AnimationState,
    task_visual: &SoulTaskVisualState,
    breakdown: Option<&StressBreakdown>,
    expression: Option<&ConversationExpression>,
) -> SoulFaceState {
    if breakdown.is_some() || matches_negative_expression(expression) {
        return SoulFaceState::Fear;
    }

    if matches_positive_expression(expression) {
        return SoulFaceState::Happy;
    }

    let is_busy = animation.is_moving || task_visual.phase != SoulTaskPhaseVisual::None;
    if matches!(
        idle.behavior,
        IdleBehavior::Sleeping | IdleBehavior::Resting
    ) && !is_busy
    {
        return SoulFaceState::Sleep;
    }

    if soul.fatigue >= EMOTION_THRESHOLD_EXHAUSTED
        || matches!(idle.behavior, IdleBehavior::ExhaustedGathering)
        || matches_exhausted_expression(expression)
    {
        return SoulFaceState::Exhausted;
    }

    if is_work_phase(task_visual.phase) {
        return SoulFaceState::Focused;
    }

    SoulFaceState::Normal
}

fn is_carry_phase(phase: SoulTaskPhaseVisual) -> bool {
    matches!(
        phase,
        SoulTaskPhaseVisual::Haul
            | SoulTaskPhaseVisual::HaulToBlueprint
            | SoulTaskPhaseVisual::BucketTransport
            | SoulTaskPhaseVisual::HaulToMixer
            | SoulTaskPhaseVisual::HaulWithWheelbarrow
    )
}

fn is_work_phase(phase: SoulTaskPhaseVisual) -> bool {
    matches!(
        phase,
        SoulTaskPhaseVisual::GatherChop
            | SoulTaskPhaseVisual::GatherMine
            | SoulTaskPhaseVisual::Build
            | SoulTaskPhaseVisual::ReinforceFloor
            | SoulTaskPhaseVisual::PourFloor
            | SoulTaskPhaseVisual::FrameWall
            | SoulTaskPhaseVisual::CoatWall
            | SoulTaskPhaseVisual::Refine
            | SoulTaskPhaseVisual::CollectBone
            | SoulTaskPhaseVisual::MovePlant
            | SoulTaskPhaseVisual::Deconstruct
    )
}

fn matches_negative_expression(expression: Option<&ConversationExpression>) -> bool {
    matches!(
        expression.map(|expr| expr.kind),
        Some(ConversationExpressionKind::Negative)
    )
}

fn matches_exhausted_expression(expression: Option<&ConversationExpression>) -> bool {
    matches!(
        expression.map(|expr| expr.kind),
        Some(ConversationExpressionKind::Exhausted)
    )
}

fn matches_positive_expression(expression: Option<&ConversationExpression>) -> bool {
    matches!(
        expression.map(|expr| expr.kind),
        Some(
            ConversationExpressionKind::Positive
                | ConversationExpressionKind::GatheringWine
                | ConversationExpressionKind::GatheringTrump
        )
    )
}
