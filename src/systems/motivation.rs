use crate::constants::TILE_SIZE;
use crate::entities::damned_soul::DamnedSoul;
use crate::entities::familiar::{ActiveCommand, Familiar, FamiliarCommand, UnderCommand};
use crate::interface::camera::MainCamera;
use crate::systems::work::AssignedTask;
use bevy::prelude::*;

/// やる気・怠惰の更新システム
/// 使い魔の指定エリア内にいる人間はやる気が上がり、エリア外では怠惰に戻る
/// タスクが割り当てられているワーカーはモチベーションを維持する
pub fn motivation_system(
    time: Res<Time>,
    q_familiars: Query<(&Transform, &Familiar, &ActiveCommand)>,
    mut q_souls: Query<(
        &Transform,
        &mut DamnedSoul,
        &AssignedTask,
        Option<&UnderCommand>,
    )>,
) {
    let dt = time.delta_secs();

    for (soul_transform, mut soul, task, under_command) in q_souls.iter_mut() {
        let soul_pos = soul_transform.translation.truncate();
        let has_task = !matches!(task, AssignedTask::None);

        let best_influence = q_familiars
            .iter()
            .filter_map(|(fam_transform, familiar, command)| {
                let influence_center = fam_transform.translation.truncate();
                let distance = soul_pos.distance(influence_center);

                if distance < familiar.command_radius {
                    let command_multiplier = if matches!(command.command, FamiliarCommand::Idle) {
                        0.4
                    } else {
                        1.0
                    };
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
        } else if has_task || under_command.is_some() {
            // タスクがあるか、使役状態の場合：モチベーションをゆっくり維持
            // （使役状態なら遠くてもサボりにくい）
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
pub fn fatigue_system(mut q_souls: Query<&mut DamnedSoul>) {
    for mut soul in q_souls.iter_mut() {
        // 疲労が限界に達したらやる気が強制的に下がる
        if soul.fatigue > 0.9 {
            soul.motivation = (soul.motivation - 0.5).max(0.0);
        }
    }
}

/// 使い魔にホバーした際、使役中の魂との間に細い線を引く
pub fn familiar_hover_visualization_system(
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    q_familiars: Query<(&GlobalTransform, &ActiveCommand), With<Familiar>>,
    q_souls: Query<&GlobalTransform, With<DamnedSoul>>,
    mut gizmos: Gizmos,
) {
    let Ok(window) = q_window.get_single() else {
        return;
    };
    let Ok((camera, camera_transform)) = q_camera.get_single() else {
        return;
    };

    if let Some(cursor_pos) = window.cursor_position() {
        if let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) {
            for (fam_transform, active_command) in q_familiars.iter() {
                let fam_pos = fam_transform.translation().truncate();

                // マウスが使い魔の上にあるかチェック
                if fam_pos.distance(world_pos) < TILE_SIZE * 0.5 {
                    // 使役中の魂全員に対して線を引く
                    for &soul_entity in &active_command.assigned_souls {
                        if let Ok(soul_transform) = q_souls.get(soul_entity) {
                            let soul_pos = soul_transform.translation().truncate();
                            gizmos.line_2d(fam_pos, soul_pos, Color::srgba(1.0, 1.0, 1.0, 0.4));
                        }
                    }
                }
            }
        }
    }
}
