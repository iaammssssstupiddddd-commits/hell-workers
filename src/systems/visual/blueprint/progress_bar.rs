//! 設計図のプログレスバー関連システム

use bevy::prelude::*;

use super::components::ProgressBar;
use super::{
    COLOR_PROGRESS_BG, COLOR_PROGRESS_BUILD, COLOR_PROGRESS_MATERIAL, PROGRESS_BAR_HEIGHT,
    PROGRESS_BAR_WIDTH, PROGRESS_BAR_Y_OFFSET,
};
use crate::systems::jobs::Blueprint;
use crate::systems::utils::progress_bar::{
    GenericProgressBar, ProgressBarBackground, ProgressBarConfig, ProgressBarFill,
    spawn_progress_bar, sync_progress_bar_fill_position, sync_progress_bar_position,
    update_progress_bar_fill,
};

/// プログレスバーを持たない Blueprint にプログレスバーを生成する
pub fn spawn_progress_bar_system(
    mut commands: Commands,
    q_blueprints: Query<(Entity, &Transform), (With<Blueprint>, Without<ProgressBar>)>,
    q_progress_bars: Query<&ProgressBar>,
) {
    for (bp_entity, bp_transform) in q_blueprints.iter() {
        // 既にこの Blueprint 用のプログレスバーがあるかチェック
        let has_bar = q_progress_bars.iter().any(|pb| pb.blueprint == bp_entity);
        if has_bar {
            continue;
        }

        // utilを使用してプログレスバーを生成
        let config = ProgressBarConfig {
            width: PROGRESS_BAR_WIDTH,
            height: PROGRESS_BAR_HEIGHT,
            y_offset: PROGRESS_BAR_Y_OFFSET,
            bg_color: COLOR_PROGRESS_BG,
            fill_color: COLOR_PROGRESS_MATERIAL,
            z_index: 0.5,
        };

        let (bg_entity, fill_entity) =
            spawn_progress_bar(&mut commands, bp_entity, bp_transform, config);

        // ラッパーコンポーネントを追加
        commands.entity(bg_entity).insert(ProgressBar {
            blueprint: bp_entity,
        });
        commands.entity(fill_entity).insert(ProgressBar {
            blueprint: bp_entity,
        });
    }
}

/// プログレスバーの進捗を更新する
pub fn update_progress_bar_fill_system(
    q_blueprints: Query<(Entity, &Blueprint)>,
    q_generic_bars: Query<&GenericProgressBar>,
    mut q_fills: Query<(Entity, &ProgressBar, &mut Sprite, &mut Transform), With<ProgressBarFill>>,
) {
    for (fill_entity, pb, mut sprite, mut transform) in q_fills.iter_mut() {
        if let Some((_, bp)) = q_blueprints.iter().find(|(e, _)| *e == pb.blueprint) {
            // 全体進捗 = 資材進捗(0~0.5) + 建築進捗(0~0.5)
            let total_required: u32 = bp.required_materials.values().sum();
            let total_delivered: u32 = bp.delivered_materials.values().sum();

            let material_ratio = if total_required > 0 {
                (total_delivered as f32 / total_required as f32).min(1.0)
            } else {
                1.0
            };

            let combined_progress = material_ratio * 0.5 + bp.progress.min(1.0) * 0.5;

            // GenericProgressBarから設定を取得
            if let Ok(generic_bar) = q_generic_bars.get(fill_entity) {
                // 資材搬入中と建築中で色を変える
                let fill_color = if bp.progress > 0.0 {
                    Some(COLOR_PROGRESS_BUILD)
                } else {
                    Some(COLOR_PROGRESS_MATERIAL)
                };

                update_progress_bar_fill(
                    combined_progress,
                    &generic_bar.config,
                    &mut sprite,
                    &mut transform,
                    fill_color,
                );
            }
        }
    }
}

/// プログレスバーの位置を Blueprint に追従させる
pub fn sync_progress_bar_position_system(
    q_blueprints: Query<(Entity, &Transform), With<Blueprint>>,
    q_generic_bars: Query<&GenericProgressBar>,
    mut q_bg_bars: Query<
        (Entity, &ProgressBar, &mut Transform),
        (
            With<ProgressBarBackground>,
            Without<Blueprint>,
            Without<ProgressBarFill>,
        ),
    >,
    mut q_fill_bars: Query<
        (Entity, &ProgressBar, &mut Transform, &Sprite),
        (
            With<ProgressBarFill>,
            Without<Blueprint>,
            Without<ProgressBarBackground>,
        ),
    >,
) {
    // 背景バーをBlueprint位置に追従
    for (bg_entity, pb, mut bar_transform) in q_bg_bars.iter_mut() {
        if let Some((_, bp_transform)) = q_blueprints.iter().find(|(e, _)| *e == pb.blueprint) {
            if let Ok(generic_bar) = q_generic_bars.get(bg_entity) {
                sync_progress_bar_position(bp_transform, &generic_bar.config, &mut bar_transform);
            }
        }
    }

    // Fillバーは左寄せオフセットを保持しつつBlueprint位置に追従
    for (fill_entity, pb, mut bar_transform, sprite) in q_fill_bars.iter_mut() {
        if let Some((_, bp_transform)) = q_blueprints.iter().find(|(e, _)| *e == pb.blueprint) {
            if let Ok(generic_bar) = q_generic_bars.get(fill_entity) {
                let fill_width = sprite.custom_size.map(|s| s.x).unwrap_or(0.0);
                sync_progress_bar_fill_position(
                    bp_transform,
                    &generic_bar.config,
                    fill_width,
                    &mut bar_transform,
                );
            }
        }
    }
}

/// Blueprint が削除されたらプログレスバーも削除する
pub fn cleanup_progress_bars_system(
    mut commands: Commands,
    q_blueprints: Query<Entity, With<Blueprint>>,
    q_bars: Query<(Entity, &ProgressBar)>,
) {
    let bp_entities: std::collections::HashSet<Entity> = q_blueprints.iter().collect();
    for (bar_entity, pb) in q_bars.iter() {
        if !bp_entities.contains(&pb.blueprint) {
            commands.entity(bar_entity).despawn();
        }
    }
}
