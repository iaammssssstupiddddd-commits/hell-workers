use bevy::prelude::*;
use crate::entities::damned_soul::DamnedSoul;
use crate::entities::familiar::{Familiar, ActiveCommand, FamiliarCommand};
use crate::systems::command::TaskArea;
use crate::systems::work::AssignedTask;

/// やる気・怠惰の更新システム
/// 使い魔の指定エリア内にいる人間はやる気が上がり、エリア外では怠惰に戻る
pub fn motivation_system(
    time: Res<Time>,
    q_familiars: Query<(&Transform, &Familiar, &ActiveCommand, Option<&TaskArea>)>,
    mut q_souls: Query<(&Transform, &mut DamnedSoul, &AssignedTask)>,
) {
    let dt = time.delta_secs();

    for (soul_transform, mut soul, _task) in q_souls.iter_mut() {
        let soul_pos = soul_transform.translation.truncate();
        
        // 近くにアクティブな使い魔がいるかチェック
        let mut best_influence = 0.0_f32;
        
        for (fam_transform, familiar, command, _task_area) in q_familiars.iter() {
            // モチベーションの源は常に使い魔の現在地
            let influence_center = fam_transform.translation.truncate();
            let influence_radius = familiar.command_radius;

            let distance = soul_pos.distance(influence_center);
            
            if distance < influence_radius {
                // 指示を出している時は効率アップ、待機中は控えめ
                let command_multiplier = if matches!(command.command, FamiliarCommand::Idle) {
                    0.4 // 待機中は40%の力
                } else {
                    1.0 // 指示中は100%の力
                };

                let distance_factor = 1.0 - (distance / influence_radius);
                let influence = familiar.efficiency * distance_factor * command_multiplier;
                best_influence = best_influence.max(influence);
            }
        }

        if best_influence > 0.0 {
            // 使い魔の影響下：やる気が上がる（速度アップ 2.5 -> 4.0）
            let motivation_boost = best_influence * dt * 4.0; 
            soul.motivation = (soul.motivation + motivation_boost).min(1.0);
            soul.laziness = (soul.laziness - best_influence * dt * 2.5).max(0.0);
        } else {
            // 使い魔の影響外：やる気が下がり、怠惰に戻る
            soul.motivation = (soul.motivation - dt * 0.1).max(0.0);
            soul.laziness = (soul.laziness + dt * 0.05).min(1.0);
            
            // 休憩すると疲労が回復
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
