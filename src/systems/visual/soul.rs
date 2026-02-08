//! ソウル用ビジュアルシステム
//!
//! DamnedSoul（亡者）のプログレスバー、ステータスアイコン、タスクリンク表示

use crate::assets::GameAssets;
use crate::constants::TILE_SIZE;
use crate::constants::*;
use crate::entities::damned_soul::{
    DamnedSoul, GatheringBehavior, IdleBehavior, IdleState, SoulUiLinks,
};
use crate::systems::soul_ai::execute::task_execution::types::{
    AssignedTask, GatherPhase, HaulPhase,
};
use crate::systems::utils::progress_bar::{
    GenericProgressBar, ProgressBarBackground, ProgressBarConfig, ProgressBarFill,
    spawn_progress_bar, sync_progress_bar_fill_position, sync_progress_bar_position,
    update_progress_bar_fill,
};
use bevy::prelude::ChildOf;
use bevy::prelude::*;

/// ソウル用プログレスバーのラッパーコンポーネント
#[derive(Component)]
pub struct SoulProgressBar;

#[derive(Component)]
pub struct StatusIcon;

pub fn progress_bar_system(
    mut commands: Commands,
    q_soul_bars: Query<(Entity, &ChildOf), With<SoulProgressBar>>,
    mut q_souls: Query<(Entity, &AssignedTask, &Transform, &mut SoulUiLinks), With<DamnedSoul>>,
) {
    for (soul_entity, task, transform, mut ui_links) in q_souls.iter_mut() {
        if let AssignedTask::Gather(data) = task {
            if matches!(data.phase, GatherPhase::Collecting { .. }) {
                if ui_links.bar_entity.is_none() {
                    // utilを使用してプログレスバーを生成
                    let config = ProgressBarConfig {
                        width: TILE_SIZE * 0.8,
                        height: TILE_SIZE * 0.15,
                        y_offset: TILE_SIZE * 0.6,
                        bg_color: Color::srgba(0.0, 0.0, 0.0, 0.8),
                        fill_color: Color::srgb(0.0, 1.0, 0.0),
                        z_index: Z_BAR_BG,
                    };

                    let (bg_entity, fill_entity) =
                        spawn_progress_bar(&mut commands, soul_entity, transform, config);

                    // 親子関係を設定（Lifecycle管理のため）
                    commands.entity(soul_entity).add_child(bg_entity);
                    commands.entity(soul_entity).add_child(fill_entity);

                    commands.entity(bg_entity).insert(SoulProgressBar);
                    commands.entity(fill_entity).insert(SoulProgressBar);

                    // Fillの色を緑に設定
                    commands.entity(fill_entity).insert(Sprite {
                        color: Color::srgb(0.0, 1.0, 0.0),
                        ..default()
                    });

                    ui_links.bar_entity = Some(bg_entity);
                }
            }
        } else if let Some(bar_entity) = ui_links.bar_entity.take() {
            // プログレスバーを削除（背景とFillの両方）
            // SoulProgressBarコンポーネントを持つ全てのエンティティを削除
            let mut to_despawn = vec![bar_entity];

            // fill_entityも探して削除（Parentコンポーネントで紐付けられているもの）
            for (entity, child_of) in q_soul_bars.iter() {
                if child_of.parent() == soul_entity {
                    to_despawn.push(entity);
                }
            }

            for entity in to_despawn {
                commands.entity(entity).despawn();
            }
        }
    }
}

pub fn update_progress_bar_fill_system(
    q_souls: Query<&AssignedTask, With<DamnedSoul>>,
    q_generic_bars: Query<&GenericProgressBar>,
    q_soul_bars: Query<&ChildOf, With<SoulProgressBar>>,
    mut q_fills: Query<(Entity, &mut Sprite, &mut Transform), With<ProgressBarFill>>,
) {
    for (fill_entity, mut sprite, mut transform) in q_fills.iter_mut() {
        // Parentコンポーネントを介して親ソウルを取得
        if let Ok(child_of) = q_soul_bars.get(fill_entity) {
            if let Ok(task) = q_souls.get(child_of.parent()) {
                if let AssignedTask::Gather(data) = task {
                    if let GatherPhase::Collecting { progress } = data.phase {
                        if let Ok(generic_bar) = q_generic_bars.get(fill_entity) {
                            update_progress_bar_fill(
                                progress,
                                &generic_bar.config,
                                &mut sprite,
                                &mut transform,
                                None, // 色は変更しない（緑のまま）
                            );
                        }
                    }
                }
            }
        }
    }
}

/// バーを親エンティティに追従させるシステム
pub fn sync_progress_bar_position_system(
    q_parents: Query<&Transform, (With<AssignedTask>, Without<SoulProgressBar>)>,
    q_generic_bars: Query<&GenericProgressBar>,
    mut q_bg_bars: Query<
        (Entity, &ChildOf, &mut Transform),
        (
            With<SoulProgressBar>,
            With<ProgressBarBackground>,
            Without<AssignedTask>,
            Without<ProgressBarFill>,
        ),
    >,
    mut q_fill_bars: Query<
        (Entity, &ChildOf, &mut Transform, &Sprite),
        (
            With<SoulProgressBar>,
            With<ProgressBarFill>,
            Without<AssignedTask>,
            Without<ProgressBarBackground>,
        ),
    >,
) {
    // 背景バーを親位置に追従
    for (bg_entity, child_of, mut bar_transform) in q_bg_bars.iter_mut() {
        if let Ok(parent_transform) = q_parents.get(child_of.parent()) {
            if let Ok(generic_bar) = q_generic_bars.get(bg_entity) {
                sync_progress_bar_position(
                    parent_transform,
                    &generic_bar.config,
                    &mut bar_transform,
                );
            }
        }
    }

    // Fillバーを親位置に追従（左寄せオフセットを考慮）
    for (fill_entity, child_of, mut fill_transform, sprite) in q_fill_bars.iter_mut() {
        if let Ok(parent_transform) = q_parents.get(child_of.parent()) {
            if let Ok(generic_bar) = q_generic_bars.get(fill_entity) {
                let fill_width = sprite.custom_size.map(|s| s.x).unwrap_or(0.0);
                sync_progress_bar_fill_position(
                    parent_transform,
                    &generic_bar.config,
                    fill_width,
                    &mut fill_transform,
                );
            }
        }
    }
}

pub fn task_link_system(
    q_souls: Query<(&GlobalTransform, &AssignedTask), With<DamnedSoul>>,
    q_targets: Query<&GlobalTransform>,
    mut gizmos: Gizmos,
) {
    for (soul_transform, task) in q_souls.iter() {
        let (soul_transform, task): (&GlobalTransform, &AssignedTask) = (soul_transform, task);
        let target_entity = match task {
            AssignedTask::Gather(data) => Some(data.target),
            AssignedTask::GatherWater(data) => Some(data.bucket),
            AssignedTask::CollectSand(data) => Some(data.target),
            AssignedTask::Refine(data) => Some(data.mixer),
            AssignedTask::Haul(data) => match data.phase {
                HaulPhase::GoingToItem => Some(data.item),
                HaulPhase::GoingToStockpile => Some(data.stockpile),
                _ => None,
            },
            AssignedTask::Build(data) => Some(data.blueprint),
            AssignedTask::HaulToBlueprint(data) => Some(data.blueprint),
            AssignedTask::HaulWaterToMixer(data) => Some(data.bucket),
            _ => None,
        };

        if let Some(target) = target_entity {
            if let Ok(target_transform) = q_targets.get(target) {
                let start: Vec2 = soul_transform.translation().truncate();
                let end: Vec2 = target_transform.translation().truncate();

                // 線の色をタスクの種類で変える
                let color = match task {
                    AssignedTask::Gather(_) => Color::srgba(0.0, 1.0, 0.0, 0.4), // 緑 (採取)
                    AssignedTask::GatherWater(_) => Color::srgb(0.0, 0.5, 1.0),
                    AssignedTask::CollectSand(_) => Color::srgb(1.0, 0.8, 0.0),
                    AssignedTask::Refine(_) => Color::srgb(0.5, 0.0, 1.0),
                    AssignedTask::Haul(_) => Color::srgba(1.0, 1.0, 0.0, 0.4), // 黄 (運搬)
                    AssignedTask::Build(_) => Color::srgba(1.0, 1.0, 1.0, 0.5), // 白 (建築)
                    AssignedTask::HaulToBlueprint(_) => Color::srgba(1.0, 1.0, 0.5, 0.4), // 薄黄 (搬入)
                    AssignedTask::HaulWaterToMixer(_) => Color::srgb(0.0, 0.5, 1.0), // Same as GatherWater
                    _ => Color::srgba(1.0, 1.0, 1.0, 0.3),
                };

                // タスクリンク線を描画
                gizmos.line_2d(start, end, color);

                // 目標地点にマーカー円を描画
                let marker_color = color.with_alpha(0.6);
                gizmos.circle_2d(end, 4.0, marker_color);

                debug!("HAUL_GIZMO: Drawing line from {:?} to {:?}", start, end);
            }
        }
    }
}

pub fn soul_status_visual_system(
    mut commands: Commands,
    mut q_souls: Query<
        (
            Entity,
            &Transform,
            &DamnedSoul,
            &mut SoulUiLinks,
            &AssignedTask,
            Option<&IdleState>,
        ),
        With<DamnedSoul>,
    >,
    mut q_text: Query<&mut Text2d, With<StatusIcon>>,
    game_assets: Res<GameAssets>,
) {
    for (soul_entity, _transform, soul, mut ui_links, task, idle_state) in q_souls.iter_mut() {
        let status = if soul.fatigue > 0.8 {
            Some(("!", Color::srgb(1.0, 0.0, 0.0))) // 疲労蓄積
        } else if soul.motivation < 0.2 {
            Some(("?", Color::srgb(0.5, 0.5, 1.0))) // やる気なし
        } else if matches!(task, AssignedTask::None) {
            // 待機中の場合、さらに詳細な状態を確認
            let is_sleeping = idle_state.map_or(false, |state| {
                state.behavior == IdleBehavior::Sleeping
                    || (state.behavior == IdleBehavior::Gathering
                        && state.gathering_behavior == GatheringBehavior::Sleeping)
            });

            if is_sleeping {
                Some(("Zzz", Color::srgb(0.5, 0.7, 1.0))) // 睡眠中
            } else {
                None // 通常の待機中は何も表示しない
            }
        } else {
            None
        };

        if let Some((text, color)) = status {
            if let Some(icon_entity) = ui_links.icon_entity {
                if let Ok(mut text2d) = q_text.get_mut(icon_entity) {
                    text2d.0 = text.to_string();
                }
                // 位置の更新
                commands
                    .entity(icon_entity)
                    .insert(Transform::from_translation(Vec3::new(
                        TILE_SIZE * 0.4,
                        TILE_SIZE * 0.4,
                        0.5,
                    )));
            } else {
                let icon_id = commands
                    .spawn((
                        StatusIcon,
                        Text2d::new(text),
                        TextFont {
                            font: game_assets.font_soul_name.clone(),
                            font_size: FONT_SIZE_BODY,
                            ..default()
                        },
                        TextColor(color),
                        Transform::from_translation(Vec3::new(
                            TILE_SIZE * 0.4,
                            TILE_SIZE * 0.4,
                            0.5,
                        )),
                    ))
                    .id();
                commands.entity(soul_entity).add_child(icon_id);
                ui_links.icon_entity = Some(icon_id);
            }
        } else if let Some(icon_entity) = ui_links.icon_entity.take() {
            commands.entity(icon_entity).despawn();
        }
    }
}
