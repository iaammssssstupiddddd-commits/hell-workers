//! ソウル用ビジュアルシステム
//!
//! サブモジュール:
//! - `idle`: IdleBehavior ビジュアルフィードバック
//! - `gathering`: 集会オーラ・デバッグ可視化
//! - `vitals`: 使い魔ホバー線描画
//! - プログレスバー、ステータスアイコン、タスクリンク表示

pub mod gathering;
pub mod gathering_spawn;
pub mod idle;
pub mod vitals;

use crate::handles::SpeechHandles;
use crate::progress_bar::*;
use bevy::prelude::ChildOf;
use bevy::prelude::*;
use hw_core::constants::TILE_SIZE;
use hw_core::constants::*;
use hw_core::soul::{DamnedSoul, GatheringBehavior, IdleBehavior, IdleState, SoulUiLinks};
use hw_core::visual_mirror::task::{SoulTaskPhaseVisual, SoulTaskVisualState};

/// ソウル用プログレスバーのラッパーコンポーネント
#[derive(Component)]
pub struct SoulProgressBar;

#[derive(Component)]
pub struct StatusIcon;

pub fn progress_bar_system(
    mut commands: Commands,
    q_soul_bars: Query<(Entity, &ChildOf), With<SoulProgressBar>>,
    mut q_souls: Query<(Entity, &SoulTaskVisualState, &Transform, &mut SoulUiLinks), With<DamnedSoul>>,
) {
    for (soul_entity, task_vs, transform, mut ui_links) in q_souls.iter_mut() {
        let needs_bar = task_vs.progress.is_some();

        if needs_bar {
            if ui_links.bar_entity.is_none() {
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

                commands.entity(bg_entity).try_insert(ChildOf(soul_entity));
                commands
                    .entity(fill_entity)
                    .try_insert(ChildOf(soul_entity));

                commands.entity(bg_entity).insert(SoulProgressBar);
                commands.entity(fill_entity).insert(SoulProgressBar);

                commands.entity(fill_entity).insert(Sprite {
                    color: Color::srgb(0.0, 1.0, 0.0),
                    ..default()
                });

                ui_links.bar_entity = Some(bg_entity);
            }
        } else if let Some(bar_entity) = ui_links.bar_entity.take() {
            let mut to_despawn = vec![bar_entity];

            for (entity, child_of) in q_soul_bars.iter() {
                if child_of.parent() == soul_entity {
                    to_despawn.push(entity);
                }
            }

            for entity in to_despawn {
                commands.entity(entity).try_despawn();
            }
        }
    }
}

pub fn update_progress_bar_fill_system(
    q_souls: Query<&SoulTaskVisualState, With<DamnedSoul>>,
    q_generic_bars: Query<&GenericProgressBar>,
    q_soul_bars: Query<&ChildOf, With<SoulProgressBar>>,
    mut q_fills: Query<(Entity, &mut Sprite, &mut Transform), With<ProgressBarFill>>,
) {
    for (fill_entity, mut sprite, mut transform) in q_fills.iter_mut() {
        if let Ok(child_of) = q_soul_bars.get(fill_entity) {
            if let Ok(task_vs) = q_souls.get(child_of.parent()) {
                if let Some(progress) = task_vs.progress {
                    if let Ok(generic_bar) = q_generic_bars.get(fill_entity) {
                        update_progress_bar_fill(
                            progress,
                            &generic_bar.config,
                            &mut sprite,
                            &mut transform,
                            None,
                        );
                    }
                }
            }
        }
    }
}

/// バーを親エンティティに追従させるシステム
pub fn sync_progress_bar_position_system(
    q_parents: Query<&Transform, (With<SoulTaskVisualState>, Without<SoulProgressBar>)>,
    q_generic_bars: Query<&GenericProgressBar>,
    mut q_bg_bars: Query<
        (Entity, &ChildOf, &mut Transform),
        (
            With<SoulProgressBar>,
            With<ProgressBarBackground>,
            Without<SoulTaskVisualState>,
            Without<ProgressBarFill>,
        ),
    >,
    mut q_fill_bars: Query<
        (Entity, &ChildOf, &mut Transform, &Sprite),
        (
            With<SoulProgressBar>,
            With<ProgressBarFill>,
            Without<SoulTaskVisualState>,
            Without<ProgressBarBackground>,
        ),
    >,
) {
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
    q_souls: Query<(&GlobalTransform, &SoulTaskVisualState), With<DamnedSoul>>,
    q_targets: Query<&GlobalTransform>,
    mut gizmos: Gizmos,
) {
    for (soul_transform, task_vs) in q_souls.iter() {
        let target_entity = task_vs.bucket_link.or(task_vs.link_target);

        if let Some(target) = target_entity {
            if let Ok(target_transform) = q_targets.get(target) {
                let start: Vec2 = soul_transform.translation().truncate();
                let end: Vec2 = target_transform.translation().truncate();

                let color = if task_vs.bucket_link.is_some() {
                    Color::srgb(0.0, 0.5, 1.0)
                } else {
                    match task_vs.phase {
                        SoulTaskPhaseVisual::GatherChop | SoulTaskPhaseVisual::GatherMine => {
                            Color::srgba(0.0, 1.0, 0.0, 0.4)
                        }
                        SoulTaskPhaseVisual::CollectSand => Color::srgb(1.0, 0.8, 0.0),
                        SoulTaskPhaseVisual::Refine => Color::srgb(0.5, 0.0, 1.0),
                        SoulTaskPhaseVisual::Haul => Color::srgba(1.0, 1.0, 0.0, 0.4),
                        SoulTaskPhaseVisual::Build => Color::srgba(1.0, 1.0, 1.0, 0.5),
                        SoulTaskPhaseVisual::HaulToBlueprint => Color::srgba(1.0, 1.0, 0.5, 0.4),
                        SoulTaskPhaseVisual::FrameWall | SoulTaskPhaseVisual::CoatWall => {
                            Color::srgba(1.0, 1.0, 1.0, 0.5)
                        }
                        _ => Color::srgba(1.0, 1.0, 1.0, 0.3),
                    }
                };

                gizmos.line_2d(start, end, color);

                let marker_color = color.with_alpha(0.6);
                gizmos.circle_2d(end, 4.0, marker_color);
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
            &SoulTaskVisualState,
            Option<&IdleState>,
        ),
        With<DamnedSoul>,
    >,
    mut q_text: Query<&mut Text2d, With<StatusIcon>>,
    speech_handles: Res<SpeechHandles>,
) {
    for (soul_entity, _transform, soul, mut ui_links, task_vs, idle_state) in q_souls.iter_mut() {
        let status = if soul.fatigue > 0.8 {
            Some(("!", Color::srgb(1.0, 0.0, 0.0)))
        } else if soul.motivation < 0.2 {
            Some(("?", Color::srgb(0.5, 0.5, 1.0)))
        } else if task_vs.phase == SoulTaskPhaseVisual::None {
            let is_sleeping = idle_state.map_or(false, |state| {
                state.behavior == IdleBehavior::Sleeping
                    || state.behavior == IdleBehavior::Resting
                    || (state.behavior == IdleBehavior::Gathering
                        && state.gathering_behavior == GatheringBehavior::Sleeping)
            });

            if is_sleeping {
                Some(("Zzz", Color::srgb(0.5, 0.7, 1.0)))
            } else {
                None
            }
        } else {
            None
        };

        if let Some((text, color)) = status {
            if let Some(icon_entity) = ui_links.icon_entity {
                if let Ok(mut text2d) = q_text.get_mut(icon_entity) {
                    text2d.0 = text.to_string();
                }
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
                            font: speech_handles.font_soul_name.clone(),
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
                commands.entity(icon_id).try_insert(ChildOf(soul_entity));
                ui_links.icon_entity = Some(icon_id);
            }
        } else if let Some(icon_entity) = ui_links.icon_entity.take() {
            commands.entity(icon_entity).try_despawn();
        }
    }
}
