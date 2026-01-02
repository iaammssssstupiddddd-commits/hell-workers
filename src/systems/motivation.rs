use bevy::prelude::*;
use crate::entities::damned_soul::DamnedSoul;
use crate::entities::familiar::{Familiar, ActiveCommand, FamiliarCommand};
use crate::systems::work::AssignedTask;

/// やる気・怠惰の更新システム
/// 使い魔の指定エリア内にいる人間はやる気が上がり、エリア外では怠惰に戻る
/// タスクが割り当てられているワーカーはモチベーションを維持する
pub fn motivation_system(
    time: Res<Time>,
    q_familiars: Query<(&Transform, &Familiar, &ActiveCommand)>,
    mut q_souls: Query<(&Transform, &mut DamnedSoul, &AssignedTask)>,
) {
    let dt = time.delta_secs();

    for (soul_transform, mut soul, task) in q_souls.iter_mut() {
        let soul_pos = soul_transform.translation.truncate();
        let has_task = !matches!(task, AssignedTask::None);
        
        let best_influence = q_familiars.iter()
            .filter_map(|(fam_transform, familiar, command)| {
                let influence_center = fam_transform.translation.truncate();
                let distance = soul_pos.distance(influence_center);
                
                if distance < familiar.command_radius {
                    let command_multiplier = if matches!(command.command, FamiliarCommand::Idle) { 0.4 } else { 1.0 };
                    let distance_factor = 1.0 - (distance / familiar.command_radius);
                    Some(familiar.efficiency * distance_factor * command_multiplier)
                } else {
                    None
                }
            })
            .fold(0.0_f32, |acc, x| acc.max(x));

        if best_influence > 0.0 {
            // 使い魔の影響下：やる気が上がる
            soul.motivation = (soul.motivation + best_influence * dt * 4.0).min(1.0);
            soul.laziness = (soul.laziness - best_influence * dt * 2.5).max(0.0);
        } else if has_task {
            // タスクがある場合：モチベーションをゆっくり維持
            soul.motivation = (soul.motivation - dt * 0.02).max(0.0);
            soul.laziness = (soul.laziness - dt * 0.1).max(0.0);
            soul.fatigue = (soul.fatigue + dt * 0.01).min(1.0);
        } else {
            // 使い魔の影響外でタスクもなし：やる気が下がり、怠惰に戻る
            soul.motivation = (soul.motivation - dt * 0.1).max(0.0);
            soul.laziness = (soul.laziness + dt * 0.05).min(1.0);
            soul.fatigue = (soul.fatigue - dt * 0.05).max(0.0);
        }
    }
}

/// 疲労が限界に達したら強制的に休憩させるシステム
pub fn fatigue_system(
    mut q_souls: Query<&mut DamnedSoul>,
) {
    for mut soul in q_souls.iter_mut() {
        // 疲労が限界に達したらやる気が強制的に下がる
        if soul.fatigue > 0.9 {
            soul.motivation = (soul.motivation - 0.5).max(0.0);
        }
    }
}
