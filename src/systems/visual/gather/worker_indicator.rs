//! 伐採・採掘中のワーカーインジケータ（斧/ツルハシアイコン）

use bevy::prelude::*;

use super::components::{HasGatherIndicator, WorkerGatherIcon};
use super::{
    COLOR_CHOP_ICON, COLOR_MINE_ICON, GATHER_ICON_BOB_AMPLITUDE, GATHER_ICON_BOB_SPEED,
    GATHER_ICON_SIZE, GATHER_ICON_Y_OFFSET,
};
use crate::assets::GameAssets;
use crate::entities::damned_soul::DamnedSoul;
use crate::systems::jobs::WorkType;
use crate::systems::soul_ai::execute::task_execution::types::{AssignedTask, GatherPhase};
use crate::systems::utils::worker_icon::{
    WorkerIcon, WorkerIconConfig, spawn_worker_icon, update_worker_icon_position,
};

/// 伐採・採掘中のワーカーにアイコンを付与する
pub fn spawn_gather_indicators_system(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    q_workers: Query<
        (Entity, &AssignedTask, &Transform),
        (With<DamnedSoul>, Without<HasGatherIndicator>),
    >,
) {
    for (worker_entity, assigned_task, transform) in q_workers.iter() {
        if let AssignedTask::Gather(data) = assigned_task {
            let phase = &data.phase;
            let work_type = &data.work_type;
            // 採取中のみアイコン表示
            if matches!(phase, GatherPhase::Collecting { .. }) {
                // WorkTypeに応じてアイコンと色を決定
                let (icon_handle, icon_color) = match work_type {
                    WorkType::Chop => (game_assets.icon_axe.clone(), COLOR_CHOP_ICON),
                    WorkType::Mine => (game_assets.icon_pick.clone(), COLOR_MINE_ICON),
                    _ => continue, // Haulなど他のタイプはスキップ
                };

                info!(
                    "VISUAL: Spawning gather icon for worker {:?} ({:?})",
                    worker_entity, work_type
                );

                let config = WorkerIconConfig {
                    size: Vec2::splat(GATHER_ICON_SIZE),
                    y_offset: GATHER_ICON_Y_OFFSET,
                    color: icon_color,
                    bob_speed: GATHER_ICON_BOB_SPEED,
                    bob_amplitude: GATHER_ICON_BOB_AMPLITUDE,
                    z_index: 0.5,
                };

                let icon_entity =
                    spawn_worker_icon(&mut commands, worker_entity, transform, icon_handle, config);

                // ラッパーコンポーネントを追加
                commands.entity(icon_entity).insert(WorkerGatherIcon {
                    worker: worker_entity,
                });

                commands.entity(worker_entity).insert(HasGatherIndicator);
            }
        }
    }
}

/// ワーカーインジケータの位置更新とクリーンアップ
pub fn update_gather_indicators_system(
    mut commands: Commands,
    time: Res<Time>,
    q_workers: Query<(Entity, &AssignedTask, &Transform), With<DamnedSoul>>,
    mut q_icons: Query<
        (Entity, &WorkerGatherIcon, &WorkerIcon, &mut Transform),
        Without<DamnedSoul>,
    >,
) {
    for (icon_entity, gather_icon, worker_icon, mut icon_transform) in q_icons.iter_mut() {
        let mut should_despawn = true;

        if let Ok((_w_entity, assigned_task, worker_transform)) = q_workers.get(gather_icon.worker)
        {
            if let AssignedTask::Gather(data) = assigned_task {
                let phase = &data.phase;
                if matches!(phase, GatherPhase::Collecting { .. }) {
                    should_despawn = false;

                    // utilを使用して位置更新（bob付き）
                    update_worker_icon_position(
                        &time,
                        Some(worker_transform),
                        worker_icon,
                        &mut icon_transform,
                    );
                }
            }
        }

        if should_despawn {
            info!(
                "VISUAL: Despawning gather icon for worker {:?}",
                gather_icon.worker
            );
            commands.entity(icon_entity).despawn();
            // HasGatherIndicatorを削除
            if let Ok(mut entity_commands) = commands.get_entity(gather_icon.worker) {
                entity_commands.remove::<HasGatherIndicator>();
            }
        }
    }
}
