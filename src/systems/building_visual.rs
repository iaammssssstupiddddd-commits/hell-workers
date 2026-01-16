//! 建築ビジュアルシステム
//!
//! 設計図（Blueprint）の視覚的フィードバックを管理するモジュール。
//! - 透明度の動的変化
//! - プログレスバー表示
//! - 状態別カラーオーバーレイ
//! - アニメーション効果

use bevy::prelude::*;

use crate::assets::GameAssets;
use crate::systems::jobs::Blueprint;
use crate::systems::logistics::ResourceType;
use crate::systems::utils::{
    animations::{BounceAnimation, PulseAnimation, update_bounce_animation, update_pulse_animation},
    floating_text::{FloatingText, FloatingTextConfig, spawn_floating_text, update_floating_text},
    progress_bar::{
        ProgressBarConfig, GenericProgressBar, ProgressBarBackground, ProgressBarFill,
        spawn_progress_bar, update_progress_bar_fill, sync_progress_bar_position,
        sync_progress_bar_fill_position,
    },
};
use std::collections::HashMap;

// ============================================================================
// コンポーネント定義
// ============================================================================

/// 設計図の現在の状態
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlueprintState {
    /// 資材が不足している
    #[default]
    NeedsMaterials,
    /// 資材運搬中（一部搬入済み）
    Preparing,
    /// 資材が揃い、建築可能
    ReadyToBuild,
    /// 建築作業中
    Building,
}

/// 設計図のビジュアル状態を管理するコンポーネント
#[derive(Component, Default)]
pub struct BlueprintVisual {
    /// 現在の状態
    pub state: BlueprintState,
    /// パルスアニメーション（utilを使用）
    pub pulse_animation: Option<PulseAnimation>,
    /// 前フレームの搬入済み資材数（ポップアップ検出用）
    pub last_delivered: HashMap<ResourceType, u32>,
}

/// 資材アイコン表示用コンポーネント
#[derive(Component)]
pub struct MaterialIcon {
    /// 親となる設計図エンティティ
    pub blueprint: Entity,
    /// 表示する資材タイプ
    pub _resource_type: ResourceType,
}

/// 資材カウンター表示用コンポーネント
#[derive(Component)]
pub struct MaterialCounter {
    /// 親となる設計図エンティティ
    pub blueprint: Entity,
    /// 表示する資材タイプ
    pub resource_type: ResourceType,
}

/// 搬入時の「+1」ポップアップ（util::FloatingTextのラッパー）
#[derive(Component)]
pub struct DeliveryPopup {
    /// 内部のFloatingTextコンポーネント
    pub floating_text: FloatingText,
}

/// 完成時のフローティングテキスト（util::FloatingTextのラッパー）
#[derive(Component)]
pub struct CompletionText {
    /// 内部のFloatingTextコンポーネント
    pub floating_text: FloatingText,
}

/// 完成した建物に付与する一時的なバウンス（跳ねる）アニメーション（util::BounceAnimationのラッパー）
#[derive(Component)]
pub struct BuildingBounceEffect {
    /// 内部のBounceAnimationコンポーネント
    pub bounce_animation: BounceAnimation,
}

/// 建築中のワーカー頭上に表示されるハンマーアイコン
#[derive(Component)]
pub struct WorkerHammerIcon {
    pub worker: Entity,
}

/// インジケータが既に付与されていることを示すマーカー
#[derive(Component)]
pub struct HasWorkerIndicator;

/// プログレスバーのマーカーコンポーネント（util::GenericProgressBarのラッパー）
#[derive(Component)]
pub struct ProgressBar {
    /// 親となる設計図エンティティ
    pub blueprint: Entity,
}

// ============================================================================
// 定数
// ============================================================================

/// プログレスバーの幅
pub const PROGRESS_BAR_WIDTH: f32 = 24.0;
/// プログレスバーの高さ
pub const PROGRESS_BAR_HEIGHT: f32 = 4.0;
/// プログレスバーのY軸オフセット（設計図の下）
pub const PROGRESS_BAR_Y_OFFSET: f32 = -18.0;


/// 資材アイコンのオフセット
pub const MATERIAL_ICON_X_OFFSET: f32 = 20.0;
pub const MATERIAL_ICON_Y_OFFSET: f32 = 10.0;
/// カウンターテキストのオフセット（アイコンからの相対）
pub const COUNTER_TEXT_OFFSET: Vec3 = Vec3::new(12.0, 0.0, 0.0);

/// ポップアップの表示時間
pub const POPUP_LIFETIME: f32 = 1.0;
/// 完成テキストの表示時間
pub const COMPLETION_TEXT_LIFETIME: f32 = 1.5;
/// バウンスアニメーションの持続時間
pub const BOUNCE_DURATION: f32 = 0.4;

// ============================================================================
// カラー定義
// ============================================================================

/// 青写真（未着手）の基本色：鮮やかなシアンブルー
pub const COLOR_BLUEPRINT: Color = Color::srgba(0.1, 0.5, 1.0, 1.0);
/// 建築開始後の基本色：本来のテクスチャ色（ホワイトティント）
pub const COLOR_NORMAL: Color = Color::srgba(1.0, 1.0, 1.0, 1.0);

/// プログレスバー背景色
pub const COLOR_PROGRESS_BG: Color = Color::srgba(0.1, 0.1, 0.1, 0.9);
/// プログレスバー前景色（資材搬入中）
pub const COLOR_PROGRESS_MATERIAL: Color = Color::srgba(1.0, 0.7, 0.1, 1.0);
/// プログレスバー前景色（建築中）
pub const COLOR_PROGRESS_BUILD: Color = Color::srgba(0.1, 0.9, 0.3, 1.0);

// ============================================================================
// ユーティリティ関数
// ============================================================================

/// 設計図の状態を計算する
pub fn calculate_blueprint_state(bp: &Blueprint) -> BlueprintState {
    if bp.progress > 0.0 {
        BlueprintState::Building
    } else if bp.materials_complete() {
        BlueprintState::ReadyToBuild
    } else {
        let total_delivered: u32 = bp.delivered_materials.values().sum();
        if total_delivered > 0 {
            BlueprintState::Preparing
        } else {
            BlueprintState::NeedsMaterials
        }
    }
}

/// 設計図の表示設定（色と透明度）を計算する
pub fn calculate_blueprint_visual_props(bp: &Blueprint) -> (Color, f32) {
    let total_required: u32 = bp.required_materials.values().sum();
    let total_delivered: u32 = bp.delivered_materials.values().sum();

    let material_ratio = if total_required > 0 {
        (total_delivered as f32 / total_required as f32).min(1.0)
    } else {
        1.0
    };

    // 透明度: 0.4(ベース) + 0.2(搬入) + 0.4(建築) = 最高 1.0
    let opacity = 0.4 + 0.2 * material_ratio + 0.4 * bp.progress.min(1.0);

    // 色: 未着手(progress=0)は BLUEPRINT、建築開始後は進捗に応じて NORMAL へ
    let color = if bp.progress > 0.0 {
        // 建築が始まったら、本来の色（ホワイトティント）にする
        COLOR_NORMAL
    } else {
        // 未着手時は青写真
        COLOR_BLUEPRINT
    };

    (color, opacity)
}

// ============================================================================
// システム
// ============================================================================

/// BlueprintVisual コンポーネントを持たない Blueprint に自動的に追加する
pub fn attach_blueprint_visual_system(
    mut commands: Commands,
    q_blueprints: Query<Entity, (With<Blueprint>, Without<BlueprintVisual>)>,
) {
    for entity in q_blueprints.iter() {
        commands.entity(entity).insert(BlueprintVisual::default());
    }
}

/// 設計図のビジュアル（色と透明度）を更新する
pub fn update_blueprint_visual_system(
    mut q_blueprints: Query<(&Blueprint, &mut BlueprintVisual, &mut Sprite)>,
) {
    for (bp, mut visual, mut sprite) in q_blueprints.iter_mut() {
        visual.state = calculate_blueprint_state(bp);

        let (color, opacity) = calculate_blueprint_visual_props(bp);
        sprite.color = color.with_alpha(opacity);
    }
}

/// 建築中のパルスアニメーション
pub fn blueprint_pulse_animation_system(
    time: Res<Time>,
    mut q_blueprints: Query<(&mut BlueprintVisual, &mut Sprite)>,
) {
    for (mut visual, mut sprite) in q_blueprints.iter_mut() {
        if visual.state == BlueprintState::Building {
            // パルスアニメーションを初期化（まだない場合）
            if visual.pulse_animation.is_none() {
                visual.pulse_animation = Some(PulseAnimation::default());
            }

            // パルスアニメーションを更新
            if let Some(ref mut pulse) = visual.pulse_animation {
                let pulse_alpha = update_pulse_animation(&time, pulse);
                sprite.color = sprite.color.with_alpha(pulse_alpha);
            }
        } else {
            visual.pulse_animation = None;
        }
    }
}

/// 進捗に応じたスケールアニメーション
pub fn blueprint_scale_animation_system(
    mut q_blueprints: Query<(&Blueprint, &mut Transform), With<BlueprintVisual>>,
) {
    for (bp, mut transform) in q_blueprints.iter_mut() {
        // scale = 0.9 + 0.1 * progress
        let scale = 0.9 + 0.1 * bp.progress.min(1.0);
        transform.scale = Vec3::splat(scale);
    }
}

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
            z_index: 0.5,
        };

        let (bg_entity, fill_entity) = spawn_progress_bar(&mut commands, bp_entity, bp_transform, config);

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

/// Blueprint に資材表示を生成する
pub fn spawn_material_display_system(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    q_blueprints: Query<(Entity, &Blueprint), (With<Blueprint>, Without<BlueprintVisual>)>,
) {
    for (bp_entity, bp) in q_blueprints.iter() {
        // BlueprintVisual がまだない = 初期段階
        // 必要な資材ごとにアイコンとカウンターを生成
        let mut i = 0;
        for (resource_type, _) in &bp.required_materials {
            let icon_image = match resource_type {
                ResourceType::Wood => game_assets.icon_wood_small.clone(),
                ResourceType::Stone => game_assets.icon_stone_small.clone(),
            };

            let offset = Vec3::new(
                MATERIAL_ICON_X_OFFSET,
                MATERIAL_ICON_Y_OFFSET - (i as f32 * 14.0),
                0.2,
            );

            commands.entity(bp_entity).with_children(|parent| {
                // アイコン
                parent.spawn((
                    MaterialIcon {
                        blueprint: bp_entity,
                        _resource_type: *resource_type,
                    },
                    Sprite {
                        image: icon_image,
                        custom_size: Some(Vec2::splat(12.0)),
                        ..default()
                    },
                    Transform::from_translation(offset),
                    Name::new(format!("MaterialIcon ({:?})", resource_type)),
                ));

                // カウンター
                parent.spawn((
                    MaterialCounter {
                        blueprint: bp_entity,
                        resource_type: *resource_type,
                    },
                    Text2d::new("0/0"),
                    TextFont {
                        font_size: 10.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    TextLayout::new_with_justify(Justify::Left),
                    Transform::from_translation(offset + COUNTER_TEXT_OFFSET),
                    Name::new(format!("MaterialCounter ({:?})", resource_type)),
                ));
            });

            i += 1;
        }
    }
}

/// 資材カウンターの数値を更新する
pub fn update_material_counter_system(
    q_blueprints: Query<(Entity, &Blueprint)>,
    mut q_counters: Query<(&MaterialCounter, &mut Text2d)>,
) {
    for (counter, mut text) in q_counters.iter_mut() {
        if let Some((_, bp)) = q_blueprints.iter().find(|(e, _)| *e == counter.blueprint) {
            let delivered = bp
                .delivered_materials
                .get(&counter.resource_type)
                .unwrap_or(&0);
            let required = bp
                .required_materials
                .get(&counter.resource_type)
                .unwrap_or(&0);
            text.0 = format!("{}/{}", delivered, required);
        }
    }
}

/// 資材搬入時のエフェクト（ポップアップ）を発生させる
pub fn material_delivery_vfx_system(
    mut commands: Commands,
    mut q_visuals: Query<(Entity, &mut BlueprintVisual, &Blueprint, &Transform)>,
) {
    for (_, mut visual, bp, transform) in q_visuals.iter_mut() {
        for (resource_type, &current_count) in &bp.delivered_materials {
            let last_count = visual.last_delivered.get(resource_type).unwrap_or(&0);
            if current_count > *last_count {
                // utilを使用してポップアップ生成
                let config = FloatingTextConfig {
                    lifetime: POPUP_LIFETIME,
                    velocity: Vec2::new(0.0, 20.0),
                    initial_color: Color::srgb(1.0, 1.0, 0.5),
                    fade_out: true,
                };

                let popup_entity = spawn_floating_text(
                    &mut commands,
                    "+1",
                    transform.translation + Vec3::new(0.0, 10.0, 1.0),
                    config.clone(),
                    Some(12.0),
                );

                // ラッパーコンポーネントを追加
                commands.entity(popup_entity).insert(DeliveryPopup {
                    floating_text: FloatingText {
                        lifetime: config.lifetime,
                        config,
                    },
                });
            }
            visual.last_delivered.insert(*resource_type, current_count);
        }
    }
}

/// 搬入ポップアップのアニメーションと削除
pub fn update_delivery_popup_system(
    mut commands: Commands,
    time: Res<Time>,
    mut q_popups: Query<(Entity, &mut DeliveryPopup, &mut FloatingText, &mut Transform, &mut TextColor)>,
) {
    for (entity, mut popup, mut floating_text, mut transform, mut color) in q_popups.iter_mut() {
        // utilを使用してフローティングテキストを更新
        let (should_despawn, new_position, alpha) =
            update_floating_text(&time, &mut floating_text, transform.translation);

        if should_despawn {
            commands.entity(entity).despawn();
            continue;
        }

        // ラッパーコンポーネントも更新
        popup.floating_text = (*floating_text).clone();

        transform.translation = new_position;
        color.0 = color.0.with_alpha(alpha);
    }
}

/// 資材アイコンとカウンターのクリーンアップ（親子関係により追従は自動）
pub fn cleanup_material_display_system(
    mut commands: Commands,
    q_blueprints: Query<Entity, With<Blueprint>>,
    q_icons: Query<(Entity, &MaterialIcon)>,
    q_counters: Query<(Entity, &MaterialCounter)>,
) {
    let bp_entities: std::collections::HashSet<Entity> = q_blueprints.iter().collect();

    for (entity, icon) in q_icons.iter() {
        if !bp_entities.contains(&icon.blueprint) {
            commands.entity(entity).despawn();
        }
    }

    for (entity, counter) in q_counters.iter() {
        if !bp_entities.contains(&counter.blueprint) {
            commands.entity(entity).despawn();
        }
    }
}

/// 完成時テキストのアニメーションと削除
pub fn update_completion_text_system(
    mut commands: Commands,
    time: Res<Time>,
    mut q_texts: Query<(Entity, &mut CompletionText, &mut FloatingText, &mut Transform, &mut TextColor)>,
) {
    for (entity, mut completion, mut floating_text, mut transform, mut color) in q_texts.iter_mut() {
        // utilを使用してフローティングテキストを更新
        let (should_despawn, new_position, alpha) =
            update_floating_text(&time, &mut floating_text, transform.translation);

        if should_despawn {
            commands.entity(entity).despawn();
            continue;
        }

        // ラッパーコンポーネントも更新
        completion.floating_text = (*floating_text).clone();

        transform.translation = new_position;
        color.0 = color.0.with_alpha(alpha);
    }
}

/// 完成した建物のバウンスアニメーション
pub fn building_bounce_animation_system(
    mut commands: Commands,
    time: Res<Time>,
    mut q_bounces: Query<(Entity, &mut BuildingBounceEffect, &mut Transform)>,
) {
    for (entity, mut bounce, mut transform) in q_bounces.iter_mut() {
        // utilを使用してバウンスアニメーションを更新
        if let Some(scale) = update_bounce_animation(&time, &mut bounce.bounce_animation) {
            transform.scale = Vec3::splat(scale);
        } else {
            // アニメーション完了
            transform.scale = Vec3::ONE;
            commands.entity(entity).remove::<BuildingBounceEffect>();
        }
    }
}

pub fn spawn_worker_indicators_system(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    q_workers: Query<
        (
            Entity,
            &crate::systems::soul_ai::task_execution::types::AssignedTask,
            &Transform,
        ),
        (
            With<crate::entities::damned_soul::DamnedSoul>,
            Without<HasWorkerIndicator>,
        ),
    >,
) {
    for (worker_entity, assigned_task, transform) in q_workers.iter() {
        if let crate::systems::soul_ai::task_execution::types::AssignedTask::Build {
            blueprint,
            phase,
        } = assigned_task
        {
            if matches!(
                phase,
                crate::systems::soul_ai::task_execution::types::BuildPhase::Building { .. }
            ) {
                info!(
                    "VISUAL: Spawning hammer icon for worker {:?} (building {:?})",
                    worker_entity, blueprint
                );

                // ハンマーアイコン（正常なアセットに復旧済み）
                commands.spawn((
                    WorkerHammerIcon {
                        worker: worker_entity,
                    },
                    Sprite {
                        image: game_assets.icon_hammer.clone(),
                        custom_size: Some(Vec2::splat(16.0)),
                        color: Color::srgb(1.0, 0.8, 0.2), // 建築らしいオレンジ寄りの黄色
                        ..default()
                    },
                    Transform::from_translation(transform.translation + Vec3::new(0.0, 32.0, 0.5)),
                    Name::new("WorkerHammerIcon"),
                ));

                commands.entity(worker_entity).insert(HasWorkerIndicator);
            }
        }
    }
}

pub fn update_worker_indicators_system(
    mut commands: Commands,
    time: Res<Time>,
    q_workers: Query<
        (
            Entity,
            &crate::systems::soul_ai::task_execution::types::AssignedTask,
            &Transform,
        ),
        With<crate::entities::damned_soul::DamnedSoul>,
    >,
    mut q_hammers: Query<
        (Entity, &WorkerHammerIcon, &mut Transform),
        Without<crate::entities::damned_soul::DamnedSoul>,
    >,
) {
    for (hammer_entity, hammer, mut hammer_transform) in q_hammers.iter_mut() {
        let mut should_despawn = true;

        if let Ok((_w_entity, assigned_task, worker_transform)) = q_workers.get(hammer.worker) {
            if let crate::systems::soul_ai::task_execution::types::AssignedTask::Build {
                phase,
                ..
            } = assigned_task
            {
                if matches!(
                    phase,
                    crate::systems::soul_ai::task_execution::types::BuildPhase::Building { .. }
                ) {
                    should_despawn = false;

                    // 位置同期（Z=0.5で固定）
                    let bob = (time.elapsed_secs() * 5.0).sin() * 2.5;
                    hammer_transform.translation =
                        worker_transform.translation + Vec3::new(0.0, 32.0 + bob, 0.5);
                }
            }
        }

        if should_despawn {
            info!("VISUAL: Despawning hammer for worker {:?}", hammer.worker);
            commands.entity(hammer_entity).despawn();
            commands
                .entity(hammer.worker)
                .remove::<HasWorkerIndicator>();
        }
    }
}
