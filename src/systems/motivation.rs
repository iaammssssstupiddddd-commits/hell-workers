use bevy::prelude::*;
use crate::entities::damned_soul::DamnedSoul;
use crate::entities::familiar::{Familiar, ActiveCommand, FamiliarCommand};

/// やる気・怠惰の更新システム
/// 使い魔の近くにいる人間はやる気が上がり、離れると怠惰に戻る
pub fn motivation_system(
    time: Res<Time>,
    q_familiars: Query<(&Transform, &Familiar, &ActiveCommand)>,
    mut q_souls: Query<(&Transform, &mut DamnedSoul)>,
) {
    let dt = time.delta_secs();

    for (soul_transform, mut soul) in q_souls.iter_mut() {
        let soul_pos = soul_transform.translation.truncate();
        
        // 近くにアクティブな使い魔がいるかチェック
        let mut best_influence = 0.0_f32;
        
        for (fam_transform, familiar, command) in q_familiars.iter() {
            // 待機中の使い魔は影響を与えない
            if matches!(command.command, FamiliarCommand::Idle) {
                continue;
            }

            let fam_pos = fam_transform.translation.truncate();
            let distance = soul_pos.distance(fam_pos);
            
            if distance < familiar.command_radius {
                // 距離が近いほど影響が大きい
                let distance_factor = 1.0 - (distance / familiar.command_radius);
                let influence = familiar.efficiency * distance_factor;
                best_influence = best_influence.max(influence);
            }
        }

        if best_influence > 0.0 {
            // 使い魔の影響下：やる気が上がり、怠惰が下がる
            soul.motivation = (soul.motivation + best_influence * dt * 0.5).min(1.0);
            soul.laziness = (soul.laziness - best_influence * dt * 0.3).max(0.0);
            
            // 働くと疲労が溜まる
            if soul.motivation > 0.5 {
                soul.fatigue = (soul.fatigue + dt * 0.1).min(1.0);
            }
        } else {
            // 使い魔の影響外：やる気が下がり、怠惰に戻る
            soul.motivation = (soul.motivation - dt * 0.2).max(0.0);
            soul.laziness = (soul.laziness + dt * 0.1).min(1.0);
            
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
