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
    pub parent: Entity,
}

pub fn progress_bar_system(
    mut commands: Commands,
    q_tasks: Query<(Entity, &AssignedTask, &Transform), Without<ProgressBarFill>>,
    q_bars: Query<(Entity, &ProgressBar)>,
    mut q_fills: Query<
        (&mut Transform, &ProgressBar),
        (With<ProgressBarFill>, Without<AssignedTask>),
    >,
) {
    // 1. 進行中のタスクを特定
    for (soul_entity, task, transform) in q_tasks.iter() {
        if let AssignedTask::Gather {
            phase: GatherPhase::Collecting { progress },
            ..
        } = task
        {
            // バーが既にあるかチェック
            let has_bar = q_bars.iter().any(|(_, bar)| bar.parent == soul_entity);

            if !has_bar {
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

                commands
                    .spawn((
                        ProgressBarFill,
                        ProgressBar {
                            parent: soul_entity,
                        },
                        Sprite {
                            color: Color::srgb(0.0, 1.0, 0.0),
                            custom_size: Some(Vec2::new(TILE_SIZE * 0.8, TILE_SIZE * 0.15)),
                            ..default()
                        },
                        Transform::from_translation(
                            transform.translation + Vec3::new(0.0, TILE_SIZE * 0.6, 0.2),
                        ),
                    ))
                    .set_parent(bar_background);
            } else {
                // バーを更新
                for (mut fill_transform, bar) in q_fills.iter_mut() {
                    if bar.parent == soul_entity {
                        fill_transform.scale.x = *progress;
                        // 左寄せにするために移動
                        fill_transform.translation.x = (progress - 1.0) * TILE_SIZE * 0.4;
                    }
                }

                // 親のバー（背景）の位置も追従させる
                // (本来は親子関係で動くはずだが、グローバル座標でスポーンしている場合は手動更新が必要)
                // 今回は追従システムを分けるか、ここでやる。
            }
        } else {
            // 作業中でない魂にバーが残っていれば削除
            for (bar_entity, bar) in q_bars.iter() {
                if bar.parent == soul_entity {
                    commands.entity(bar_entity).despawn_recursive();
                }
            }
        }
    }

    // 2. 存在しない親を持つバーを削除 (クリーンアップ)
    for (bar_entity, bar) in q_bars.iter() {
        if q_tasks.get(bar.parent).is_err() {
            commands.entity(bar_entity).despawn_recursive();
        }
    }
}

/// バーを親エンティティに追従させるシステム
pub fn sync_progress_bar_position_system(
    q_parents: Query<&Transform, (With<AssignedTask>, Without<ProgressBar>)>,
    mut q_bars: Query<(&mut Transform, &ProgressBar)>,
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
            }
        }
    }
}

pub fn soul_status_visual_system(
    mut commands: Commands,
    q_souls: Query<(Entity, &Transform, &DamnedSoul, &AssignedTask)>,
    q_icons: Query<(Entity, &StatusIcon)>,
    mut q_text: Query<&mut Text2d, With<StatusIcon>>,
) {
    for (soul_entity, transform, soul, task) in q_souls.iter() {
        let status = if soul.fatigue > 0.8 {
            Some(("!", Color::srgb(1.0, 0.0, 0.0))) // 疲労蓄積
        } else if soul.motivation < 0.2 {
            Some(("?", Color::srgb(0.5, 0.5, 1.0))) // やる気なし
        } else if matches!(task, AssignedTask::None) {
            Some(("Zzz", Color::srgb(0.5, 0.7, 1.0))) // 待機中
        } else {
            None
        };

        let existing_icon = q_icons.iter().find(|(_, icon)| icon.parent == soul_entity);

        if let Some((text, color)) = status {
            if let Some((icon_entity, _)) = existing_icon {
                if let Ok(mut text2d) = q_text.get_mut(icon_entity) {
                    text2d.0 = text.to_string();
                }
                // 位置の追従 (本来は親子関係がいいが、ここでは単純に更新)
                commands
                    .entity(icon_entity)
                    .insert(Transform::from_translation(
                        transform.translation + Vec3::new(TILE_SIZE * 0.4, TILE_SIZE * 0.4, 0.5),
                    ));
            } else {
                commands.spawn((
                    StatusIcon {
                        parent: soul_entity,
                    },
                    Text2d::new(text),
                    TextFont {
                        font_size: 16.0,
                        ..default()
                    },
                    TextColor(color),
                    Transform::from_translation(
                        transform.translation + Vec3::new(TILE_SIZE * 0.4, TILE_SIZE * 0.4, 0.5),
                    ),
                ));
            }
        } else {
            // ステータス異常がなければアイコン削除
            if let Some((icon_entity, _)) = existing_icon {
                commands.entity(icon_entity).despawn_recursive();
            }
        }
    }

    // クリーンアップ
    for (icon_entity, icon) in q_icons.iter() {
        if q_souls.get(icon.parent).is_err() {
            commands.entity(icon_entity).despawn_recursive();
        }
    }
}
