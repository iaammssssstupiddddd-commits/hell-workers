use hw_core::assigned_task::AssignedTask;
use hw_core::soul::{DamnedSoul, IdleBehavior, IdleState};

/// 魂が作業可能な状態（待機中かつ健康）であるかを確認する
pub fn is_soul_available_for_work(
    soul: &DamnedSoul,
    task: &AssignedTask,
    idle: &IdleState,
    has_breakdown: bool,
    fatigue_threshold: f32,
) -> bool {
    if !matches!(*task, AssignedTask::None) {
        return false;
    }
    if matches!(
        idle.behavior,
        IdleBehavior::ExhaustedGathering
            | IdleBehavior::Resting
            | IdleBehavior::GoingToRest
            | IdleBehavior::Escaping
            | IdleBehavior::Drifting
    ) {
        return false;
    }
    if soul.fatigue >= fatigue_threshold {
        return false;
    }
    if has_breakdown {
        return false;
    }
    true
}
