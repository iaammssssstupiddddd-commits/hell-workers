//! 建築中のワーカーインジケータ（ハンマーアイコン）

use crate::constants::*;
use bevy::prelude::ChildOf;
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
    for (worker_entity, assigned_task, _transform) in q_workers.iter() {
        if let AssignedTask::Build(data) = assigned_task {
            let blueprint = data.blueprint;
            let phase = &data.phase;
            if matches!(phase, BuildPhase::Building { .. }) {
                info!(
                    "VISUAL: Spawning hammer icon for worker {:?} (building {:?})",
                    worker_entity, blueprint
                );

                let hammer_id = commands
                    .spawn((
                        WorkerHammerIcon,
                        Sprite {
                            image: game_assets.icon_hammer.clone(),
                            custom_size: Some(Vec2::splat(16.0)),
                            color: Color::srgb(1.0, 0.8, 0.2), // 建築らしいオレンジ寄りの黄色
                            ..default()
                        },
                        Transform::from_translation(Vec3::new(
                            0.0,
                            32.0,
                            Z_VISUAL_EFFECT - Z_CHARACTER,
                        )),
                        Name::new("WorkerHammerIcon"),
                    ))
                    .id();
                commands.entity(worker_entity).add_child(hammer_id);

                commands.entity(worker_entity).insert(HasWorkerIndicator);
            }
        }
    }
}

/// ワーカーインジケータの位置更新とクリーンアップ
pub fn update_worker_indicators_system(
    mut commands: Commands,
    time: Res<Time>,
    q_workers: Query<&AssignedTask, With<DamnedSoul>>,
    mut q_hammers: Query<
        (Entity, &ChildOf, &mut Transform),
        (With<WorkerHammerIcon>, Without<DamnedSoul>),
    >,
) {
    for (hammer_entity, child_of, mut hammer_transform) in q_hammers.iter_mut() {
        let mut should_despawn = true;
        let worker_entity: Entity = child_of.parent();

        if let Ok(assigned_task) = q_workers.get(worker_entity) {
            if let AssignedTask::Build(data) = assigned_task {
                let phase = &data.phase;
                if matches!(phase, BuildPhase::Building { .. }) {
                    should_despawn = false;

                    // 子エンティティなのでローカル座標で更新する
                    let bob = (time.elapsed_secs() * 5.0).sin() * 2.5;
                    hammer_transform.translation =
                        Vec3::new(0.0, 32.0 + bob, Z_VISUAL_EFFECT - Z_CHARACTER);
                }
            }
        }

        if should_despawn {
            info!("VISUAL: Despawning hammer for worker {:?}", worker_entity);
            commands.entity(hammer_entity).despawn();
            commands
                .entity(worker_entity)
                .remove::<HasWorkerIndicator>();
        }
    }
}
