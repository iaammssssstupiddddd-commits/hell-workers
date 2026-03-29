use bevy::prelude::*;

use hw_core::constants::*;
use hw_core::events::{OnEncouraged, OnSoulRecruited, OnTaskCompleted};
use hw_core::jobs::WorkType;
use hw_core::soul::DamnedSoul;

/// タスク完了時のモチベーションボーナス
pub fn on_task_completed_motivation_bonus(
    trigger: On<OnTaskCompleted>,
    mut q_souls: Query<&mut DamnedSoul>,
) {
    let event = trigger.event();
    if let Ok(mut soul) = q_souls.get_mut(event.entity) {
        let bonus = match event.work_type {
            WorkType::Chop | WorkType::Mine | WorkType::CollectSand | WorkType::CollectBone => {
                MOTIVATION_BONUS_GATHER
            }
            WorkType::Haul
            | WorkType::HaulToMixer
            | WorkType::GatherWater
            | WorkType::HaulWaterToMixer
            | WorkType::WheelbarrowHaul => MOTIVATION_BONUS_HAUL,
            WorkType::Build
            | WorkType::Move
            | WorkType::Refine
            | WorkType::ReinforceFloorTile
            | WorkType::PourFloorTile
            | WorkType::FrameWallTile
            | WorkType::CoatWall
            | WorkType::GeneratePower => MOTIVATION_BONUS_BUILD,
        };

        if bonus > 0.0 {
            soul.motivation = (soul.motivation + bonus).min(1.0);
        }
    }
}

/// 激励イベントによる効果適用
pub fn on_encouraged_effect(trigger: On<OnEncouraged>, mut q_souls: Query<&mut DamnedSoul>) {
    let event = trigger.event();
    if let Ok(mut soul) = q_souls.get_mut(event.soul_entity) {
        soul.motivation = (soul.motivation + ENCOURAGEMENT_MOTIVATION_BONUS).min(1.0);
        soul.stress = (soul.stress + ENCOURAGEMENT_STRESS_PENALTY).min(1.0);
    }
}

/// リクルート時のバイタル変化
pub fn on_soul_recruited_effect(trigger: On<OnSoulRecruited>, mut q_souls: Query<&mut DamnedSoul>) {
    let event = trigger.event();
    if let Ok(mut soul) = q_souls.get_mut(event.entity) {
        soul.motivation = (soul.motivation + RECRUIT_MOTIVATION_BONUS).min(1.0);
        soul.stress = (soul.stress + RECRUIT_STRESS_PENALTY).min(1.0);
    }
}
