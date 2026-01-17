//! リソース（木、岩等）のハイライト表示システム

use bevy::prelude::*;

use super::components::{ResourceHighlightState, ResourceVisual};
use crate::relationships::TaskWorkers;
use crate::systems::jobs::{Designation, Rock, Tree};
use crate::systems::utils::animations::{
    PulseAnimation, PulseAnimationConfig, update_pulse_animation,
};

/// 指定済みのリソースにResourceVisualを付与するティントカラー
pub const COLOR_DESIGNATED_TINT: Color = Color::srgba(0.6, 0.8, 1.0, 1.0);
/// 作業中のリソースのティントカラー
pub const COLOR_WORKING_TINT: Color = Color::srgba(0.8, 0.9, 1.0, 1.0);

/// Designation が追加されたリソースに ResourceVisual を付与する
pub fn attach_resource_visual_system(
    mut commands: Commands,
    q_resources: Query<
        (Entity, &Sprite),
        (
            With<Designation>,
            Or<(With<Tree>, With<Rock>)>,
            Without<ResourceVisual>,
        ),
    >,
) {
    for (entity, sprite) in q_resources.iter() {
        commands.entity(entity).insert(ResourceVisual {
            state: ResourceHighlightState::Designated,
            pulse_animation: Some(PulseAnimation {
                timer: 0.0,
                config: PulseAnimationConfig {
                    period: 1.0,
                    min_value: 0.7,
                    max_value: 1.0,
                },
            }),
            original_color: Some(sprite.color),
        });
    }
}

/// リソースのビジュアル状態を更新する（パルス、透明度変化）
pub fn update_resource_visual_system(
    time: Res<Time>,
    q_task_workers: Query<&TaskWorkers>,
    mut q_resources: Query<(
        Entity,
        &mut ResourceVisual,
        &mut Sprite,
        Option<&Designation>,
    )>,
) {
    for (entity, mut visual, mut sprite, designation) in q_resources.iter_mut() {
        // Designationがない場合は状態をNormalに
        if designation.is_none() {
            if visual.state != ResourceHighlightState::Normal {
                visual.state = ResourceHighlightState::Normal;
                // 元の色に復元
                if let Some(original_color) = visual.original_color {
                    sprite.color = original_color;
                }
            }
            continue;
        }

        // TaskWorkersをチェックして作業中かどうか判定
        let is_being_worked = q_task_workers
            .get(entity)
            .map(|workers| workers.len() > 0)
            .unwrap_or(false);

        // 状態を更新
        let new_state = if is_being_worked {
            ResourceHighlightState::Working
        } else {
            ResourceHighlightState::Designated
        };

        if visual.state != new_state {
            visual.state = new_state;
            // 状態変更時にパルスアニメーションをリセット
            if new_state == ResourceHighlightState::Designated {
                visual.pulse_animation = Some(PulseAnimation {
                    timer: 0.0,
                    config: PulseAnimationConfig {
                        period: 1.0,
                        min_value: 0.7,
                        max_value: 1.0,
                    },
                });
            }
        }

        // 状態に応じた表示更新
        match visual.state {
            ResourceHighlightState::Designated => {
                // パルスアニメーション
                if let Some(ref mut pulse) = visual.pulse_animation {
                    let alpha = update_pulse_animation(&time, pulse);
                    sprite.color = COLOR_DESIGNATED_TINT.with_alpha(alpha);
                }
            }
            ResourceHighlightState::Working => {
                // 作業中は固定のティント
                sprite.color = COLOR_WORKING_TINT;
            }
            ResourceHighlightState::Normal => {
                // 元の色に復元（すでにcontinueで処理済み）
            }
        }
    }
}

/// Designation が削除されたリソースから ResourceVisual を削除する
pub fn cleanup_resource_visual_system(
    mut commands: Commands,
    q_resources: Query<(Entity, &ResourceVisual, &Sprite), Without<Designation>>,
) {
    for (entity, visual, sprite) in q_resources.iter() {
        // 元の色に復元してからコンポーネントを削除
        if let Some(original_color) = visual.original_color {
            let mut sprite_copy = sprite.clone();
            sprite_copy.color = original_color;
            commands.entity(entity).insert(sprite_copy);
        }
        commands.entity(entity).remove::<ResourceVisual>();
    }
}
