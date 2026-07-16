use bevy::prelude::*;
use std::collections::HashSet;

use hw_core::constants::*;
use hw_core::events::publish_soul_exhausted;
use hw_core::relationships::CommandedBy;
use hw_core::soul::{DamnedSoul, IdleBehavior, IdleState};
use hw_jobs::AssignedTask;

use super::slow_simulation::SlowSimulationClock;

pub(crate) type FatigueUpdateQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut DamnedSoul,
        &'static AssignedTask,
        &'static IdleState,
        Option<&'static CommandedBy>,
    ),
>;

pub(crate) type FatiguePenaltyQuery<'w, 's> = Query<'w, 's, &'static mut DamnedSoul>;

/// 疲労の増減を管理するシステム
pub fn fatigue_update_system(
    clock: Res<SlowSimulationClock>,
    mut commands: Commands,
    mut exhausted_notifications: Local<HashSet<Entity>>,
    mut q_souls: FatigueUpdateQuery,
) {
    exhausted_notifications.clear();
    for _ in 0..clock.steps_this_frame() {
        fatigue_update_step(
            clock.step_secs(),
            &mut commands,
            &mut exhausted_notifications,
            &mut q_souls,
        );
    }
}

pub(crate) fn fatigue_update_step(
    dt: f32,
    commands: &mut Commands,
    exhausted_notifications: &mut HashSet<Entity>,
    q_souls: &mut FatigueUpdateQuery,
) -> u64 {
    let mut souls_updated = 0_u64;
    for (entity, mut soul, task, idle, under_command) in q_souls.iter_mut() {
        souls_updated = souls_updated.saturating_add(1);
        let has_task = !matches!(task, AssignedTask::None);
        let prev_fatigue = soul.fatigue;

        if has_task {
            // タスク実行中: 疲労増加
            soul.fatigue = (soul.fatigue + dt * FATIGUE_WORK_RATE).min(1.0);
        } else if under_command.is_some() {
            // 使役中の待機: 疲労減少（遅い）
            soul.fatigue = (soul.fatigue - dt * FATIGUE_RECOVERY_RATE_COMMANDED).max(0.0);
        } else {
            // 通常の待機: 疲労減少（速い）
            soul.fatigue = (soul.fatigue - dt * FATIGUE_RECOVERY_RATE_IDLE).max(0.0);
        }

        let crossed_exhausted_threshold = prev_fatigue <= FATIGUE_GATHERING_THRESHOLD
            && soul.fatigue > FATIGUE_GATHERING_THRESHOLD;

        if crossed_exhausted_threshold
            && idle.behavior != IdleBehavior::ExhaustedGathering
            && exhausted_notifications.insert(entity)
        {
            publish_soul_exhausted(commands, entity);
        }
    }
    souls_updated
}

/// 疲労が限界に達した際のペナルティシステム
pub fn fatigue_penalty_system(
    clock: Res<SlowSimulationClock>,
    mut q_souls: Query<&mut DamnedSoul>,
) {
    for _ in 0..clock.steps_this_frame() {
        // Keep the legacy standalone system usable for focused tests and
        // plugins. The unified driver below uses `fatigue_penalty_step` with
        // its ParamSet query; forwarding this concrete query through the type
        // alias would unnecessarily require its borrow to be `'static`.
        for mut soul in q_souls.iter_mut() {
            if soul.fatigue > FATIGUE_MOTIVATION_PENALTY_THRESHOLD {
                soul.motivation = (soul.motivation
                    - clock.step_secs() * FATIGUE_MOTIVATION_PENALTY_RATE)
                    .max(0.0);
            }
        }
    }
}

pub(crate) fn fatigue_penalty_step(dt: f32, q_souls: &mut FatiguePenaltyQuery) {
    for mut soul in q_souls.iter_mut() {
        if soul.fatigue > FATIGUE_MOTIVATION_PENALTY_THRESHOLD {
            soul.motivation = (soul.motivation - dt * FATIGUE_MOTIVATION_PENALTY_RATE).max(0.0);
        }
    }
}
