//! 建築中のワーカーインジケータ（ハンマーアイコン）

use crate::constants::*;
use bevy::prelude::*;

use super::components::{HasWorkerIndicator, WorkerHammerIcon};
use crate::assets::GameAssets;
use crate::entities::damned_soul::DamnedSoul;
use crate::systems::soul_ai::task_execution::types::{AssignedTask, BuildPhase};

/// 建築中のワーカーにハンマーアイコンを付与する
pub fn spawn_worker_indicators_system(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    q_workers: Query<
        (Entity, &AssignedTask, &Transform),
        (With<DamnedSoul>, Without<HasWorkerIndicator>),
    >,
) {
    for (worker_entity, assigned_task, transform) in q_workers.iter() {
        if let AssignedTask::Build { blueprint, phase } = assigned_task {
            if matches!(phase, BuildPhase::Building { .. }) {
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
                    Transform::from_translation(
                        transform.translation + Vec3::new(0.0, 32.0, Z_VISUAL_EFFECT - Z_CHARACTER),
                    ),
                    Name::new("WorkerHammerIcon"),
                ));

                commands.entity(worker_entity).insert(HasWorkerIndicator);
            }
        }
    }
}

/// ワーカーインジケータの位置更新とクリーンアップ
pub fn update_worker_indicators_system(
    mut commands: Commands,
    time: Res<Time>,
    q_workers: Query<(Entity, &AssignedTask, &Transform), With<DamnedSoul>>,
    mut q_hammers: Query<(Entity, &WorkerHammerIcon, &mut Transform), Without<DamnedSoul>>,
) {
    for (hammer_entity, hammer, mut hammer_transform) in q_hammers.iter_mut() {
        let mut should_despawn = true;

        if let Ok((_w_entity, assigned_task, worker_transform)) = q_workers.get(hammer.worker) {
            if let AssignedTask::Build { phase, .. } = assigned_task {
                if matches!(phase, BuildPhase::Building { .. }) {
                    should_despawn = false;

                    // 位置同期（Z=0.5で固定）
                    let bob = (time.elapsed_secs() * 5.0).sin() * 2.5;
                    hammer_transform.translation = worker_transform.translation
                        + Vec3::new(0.0, 32.0 + bob, Z_VISUAL_EFFECT - Z_CHARACTER);
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
