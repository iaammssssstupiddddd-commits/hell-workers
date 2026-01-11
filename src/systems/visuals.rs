use crate::constants::TILE_SIZE;
use crate::entities::damned_soul::DamnedSoul;
use crate::systems::work::{AssignedTask, GatherPhase, HaulPhase};
use bevy::prelude::*;

#[derive(Component)]
pub struct ProgressBar {
    pub parent: Entity,
}

#[derive(Component)]
pub struct ProgressBarFill;

#[derive(Component)]
pub struct StatusIcon {
    pub _parent: Entity,
}

pub fn progress_bar_system(
    mut commands: Commands,
    mut q_souls: Query<(Entity, &AssignedTask, &Transform, &mut DamnedSoul)>,
) {
    for (soul_entity, task, transform, mut soul) in q_souls.iter_mut() {
        if let AssignedTask::Gather {
            phase: GatherPhase::Collecting { .. },
            ..
        } = task
        {
            if soul.bar_entity.is_none() {
                // バーをスポーン
                let bar_background = commands
                    .spawn((
                        ProgressBar {
                            parent: soul_entity,
                        },
                        Sprite {
                            color: Color::srgba(0.0, 0.0, 0.0, 0.8),
                            custom_size: Some(Vec2::new(TILE_SIZE * 0.8, TILE_SIZE * 0.15)),
                            ..default()
                        },
                        Transform::from_translation(
                            transform.translation + Vec3::new(0.0, TILE_SIZE * 0.6, 0.1),
                        ),
                    ))
                    .id();

                let _fill_entity = commands
                    .spawn((
                        ProgressBarFill,
                        Sprite {
                            color: Color::srgb(0.0, 1.0, 0.0),
                            custom_size: Some(Vec2::new(TILE_SIZE * 0.8, TILE_SIZE * 0.15)),
                            ..default()
                        },
                        Transform::from_translation(Vec3::new(0.0, 0.0, 0.1)),
                        ChildOf(bar_background),
                    ))
                    .id();

                soul.bar_entity = Some(bar_background);
            }
        } else if let Some(bar_entity) = soul.bar_entity.take() {
            commands.entity(bar_entity).despawn();
        }
    }
}

pub fn update_progress_bar_fill_system(
    q_souls: Query<&AssignedTask, With<DamnedSoul>>,
    q_bars: Query<&ProgressBar>,
    mut q_fills: Query<(&mut Transform, &ChildOf), With<ProgressBarFill>>,
) {
    for (mut fill_transform, parent) in q_fills.iter_mut() {
        if let Ok(bar) = q_bars.get(parent.0) {
            if let Ok(task) = q_souls.get(bar.parent) {
                if let AssignedTask::Gather {
                    phase: GatherPhase::Collecting { progress },
                    ..
                } = task
                {
                    fill_transform.scale.x = *progress;
                    fill_transform.translation.x = (progress - 1.0) * TILE_SIZE * 0.4;
                }
            }
        }
    }
}

/// バーを親エンティティに追従させるシステム
pub fn sync_progress_bar_position_system(
    q_parents: Query<&Transform, (With<AssignedTask>, Without<ProgressBar>)>,
    mut q_bars: Query<(&mut Transform, &ProgressBar), (With<ProgressBar>, Without<AssignedTask>)>,
) {
    for (mut transform, bar) in q_bars.iter_mut() {
        if let Ok(parent_transform) = q_parents.get(bar.parent) {
            transform.translation =
                parent_transform.translation + Vec3::new(0.0, TILE_SIZE * 0.6, 0.1);
        }
    }
}

pub fn task_link_system(
    q_souls: Query<(&GlobalTransform, &AssignedTask), With<DamnedSoul>>,
    q_targets: Query<&GlobalTransform>,
    mut gizmos: Gizmos,
) {
    for (soul_transform, task) in q_souls.iter() {
        let target_entity = match task {
            AssignedTask::Gather { target, .. } => Some(*target),
            AssignedTask::Haul {
                item,
                stockpile,
                phase,
            } => match phase {
                HaulPhase::GoingToItem => Some(*item),
                HaulPhase::GoingToStockpile => Some(*stockpile),
                _ => None,
            },
            _ => None,
        };

        if let Some(target) = target_entity {
            if let Ok(target_transform) = q_targets.get(target) {
                let start = soul_transform.translation().truncate();
                let end = target_transform.translation().truncate();

                // 線の色をタスクの種類で変える
                let color = match task {
                    AssignedTask::Gather { .. } => Color::srgba(0.0, 1.0, 0.0, 0.3), // 緑 (採取)
                    AssignedTask::Haul { .. } => Color::srgba(1.0, 1.0, 0.0, 0.3),   // 黄 (運搬)
                    _ => Color::srgba(1.0, 1.0, 1.0, 0.2),
                };

                gizmos.line_2d(start, end, color);
                debug!("HAUL_GIZMO: Drawing line from {:?} to {:?}", start, end);
            }
        }
    }
}

pub fn soul_status_visual_system(
    mut commands: Commands,
    mut q_souls: Query<(Entity, &Transform, &mut DamnedSoul, &AssignedTask)>,
    mut q_text: Query<&mut Text2d, With<StatusIcon>>,
) {
    for (soul_entity, transform, mut soul, task) in q_souls.iter_mut() {
        let status = if soul.fatigue > 0.8 {
            Some(("!", Color::srgb(1.0, 0.0, 0.0))) // 疲労蓄積
        } else if soul.motivation < 0.2 {
            Some(("?", Color::srgb(0.5, 0.5, 1.0))) // やる気なし
        } else if matches!(task, AssignedTask::None) {
            Some(("Zzz", Color::srgb(0.5, 0.7, 1.0))) // 待機中
        } else {
            None
        };

        if let Some((text, color)) = status {
            if let Some(icon_entity) = soul.icon_entity {
                if let Ok(mut text2d) = q_text.get_mut(icon_entity) {
                    text2d.0 = text.to_string();
                }
                // 位置の更新
                commands
                    .entity(icon_entity)
                    .insert(Transform::from_translation(
                        transform.translation + Vec3::new(TILE_SIZE * 0.4, TILE_SIZE * 0.4, 0.5),
                    ));
            } else {
                let icon_id = commands
                    .spawn((
                        StatusIcon {
                            _parent: soul_entity,
                        },
                        Text2d::new(text),
                        TextFont {
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(color),
                        Transform::from_translation(
                            transform.translation
                                + Vec3::new(TILE_SIZE * 0.4, TILE_SIZE * 0.4, 0.5),
                        ),
                    ))
                    .id();
                soul.icon_entity = Some(icon_id);
            }
        } else if let Some(icon_entity) = soul.icon_entity.take() {
            commands.entity(icon_entity).despawn();
        }
    }
}
