use crate::constants::TILE_SIZE;
use crate::entities::damned_soul::DamnedSoul;
use crate::entities::familiar::{ActiveCommand, Familiar, FamiliarCommand, UnderCommand};
use crate::interface::camera::MainCamera;
use crate::systems::work::{AssignedTask, FamiliarSpatialGrid};
use bevy::prelude::*;

/// やる気・怠惰の更新システム
pub fn motivation_system(
    time: Res<Time>,
    familiar_grid: Res<FamiliarSpatialGrid>,
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
        let has_task = !matches!(*task, AssignedTask::None);

        // 空間グリッドを使用して近傍の使い魔のみをチェック
        let max_radius = TILE_SIZE * 10.0;
        let nearby_familiar_entities = familiar_grid.get_nearby_in_radius(soul_pos, max_radius);

        let best_influence = nearby_familiar_entities
            .iter()
            .filter_map(|&fam_entity| {
                let Ok((fam_transform, familiar, command)) = q_familiars.get(fam_entity) else {
                    return None;
                };
                let influence_center = fam_transform.translation.truncate();
                let distance_sq = soul_pos.distance_squared(influence_center);
                let radius_sq = familiar.command_radius * familiar.command_radius;

                if distance_sq < radius_sq {
                    let distance = distance_sq.sqrt();
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

        // --- モチベーションと怠惰の更新 ---
        if best_influence > 0.0 {
            soul.motivation = (soul.motivation + best_influence * dt * 4.0).min(1.0);
            soul.laziness = (soul.laziness - best_influence * dt * 2.5).max(0.0);
        } else if has_task || under_command.is_some() {
            soul.motivation = (soul.motivation - dt * 0.02).max(0.0);
            soul.laziness = (soul.laziness - dt * 0.1).max(0.0);
        } else {
            soul.motivation = (soul.motivation - dt * 0.1).max(0.0);
            soul.laziness = (soul.laziness + dt * 0.05).min(1.0);
        }
    }
}

/// 使い魔にホバーした際、使役中の魂との間に細い線を引く
pub fn familiar_hover_visualization_system(
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    q_familiars: Query<(Entity, &GlobalTransform, &ActiveCommand), With<Familiar>>,
    q_souls: Query<(&GlobalTransform, &crate::entities::familiar::UnderCommand), With<DamnedSoul>>,
    mut gizmos: Gizmos,
) {
    let Ok(window) = q_window.single() else {
        return;
    };
    let Ok((camera, camera_transform)) = q_camera.single() else {
        return;
    };

    if let Some(cursor_pos) = window.cursor_position() {
        if let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) {
            for (fam_entity, fam_transform, _) in q_familiars.iter() {
                let fam_pos = fam_transform.translation().truncate();

                // マウスが使い魔の上にあるかチェック
                if fam_pos.distance(world_pos) < TILE_SIZE * 0.5 {
                    // 使役中の魂全員（UnderCommand(fam_entity)を持つソウル）に対して線を引く
                    for (soul_transform, under_command) in q_souls.iter() {
                        if under_command.0 == fam_entity {
                            let soul_pos = soul_transform.translation().truncate();
                            gizmos.line_2d(fam_pos, soul_pos, Color::srgba(1.0, 1.0, 1.0, 0.4));
                        }
                    }
                }
            }
        }
    }
}
