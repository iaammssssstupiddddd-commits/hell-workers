use crate::constants::TILE_SIZE;
use crate::entities::damned_soul::DamnedSoul;
use crate::entities::familiar::{ActiveCommand, Familiar, FamiliarCommand, UnderCommand};
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

/// ホバー線の描画用コンポーネント
#[derive(Component)]
pub struct HoverLineIndicator;

/// 使い魔にホバーした際、使役中の魂との間に線を引く (スプライトベース)
pub fn familiar_hover_visualization_system(
    mut commands: Commands,
    hovered_entity: Res<crate::interface::selection::HoveredEntity>,
    q_familiars: Query<(&GlobalTransform, &ActiveCommand), With<Familiar>>,
    q_souls: Query<(&GlobalTransform, &UnderCommand), With<DamnedSoul>>,
    q_lines: Query<Entity, With<HoverLineIndicator>>,
    mut gizmos: Gizmos,
) {
    // 既存のスプライト線をすべて削除
    for entity in q_lines.iter() {
        commands.entity(entity).despawn();
    }

    if let Some(hovered) = hovered_entity.0 {
        if let Ok((fam_transform, _)) = q_familiars.get(hovered) {
            let fam_pos = fam_transform.translation().truncate();

            for (soul_transform, under_command) in q_souls.iter() {
                if under_command.0 == hovered {
                    let soul_pos = soul_transform.translation().truncate();
                    // 白色のホバー線を Gizmos で描画
                    gizmos.line_2d(fam_pos, soul_pos, Color::srgba(1.0, 1.0, 1.0, 0.7));
                }
            }
        }
    }
}
