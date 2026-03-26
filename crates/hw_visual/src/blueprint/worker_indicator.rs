//! 建築中のワーカーインジケータ（ハンマーアイコン）

use bevy::prelude::ChildOf;
use bevy::prelude::*;
use hw_core::constants::*;

use super::components::{HasWorkerIndicator, WorkerHammerIcon};
use crate::handles::WorkIconHandles;
use hw_core::soul::DamnedSoul;
use hw_core::visual_mirror::task::{SoulTaskPhaseVisual, SoulTaskVisualState};

type BuildWorkersQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static SoulTaskVisualState, &'static Transform),
    (With<DamnedSoul>, Without<HasWorkerIndicator>),
>;

type HammerIconsQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static ChildOf, &'static mut Transform),
    (With<WorkerHammerIcon>, Without<DamnedSoul>),
>;

pub fn spawn_worker_indicators_system(
    mut commands: Commands,
    handles: Res<WorkIconHandles>,
    q_workers: BuildWorkersQuery,
) {
    for (worker_entity, task_vs, _transform) in q_workers.iter() {
        if matches!(task_vs.phase, SoulTaskPhaseVisual::Build) && task_vs.progress.is_none() {
            // progress is None for Build (non-progressed phase); check link_target for blueprint
            if task_vs.link_target.is_some() {
                info!(
                    "VISUAL: Spawning hammer icon for worker {:?}",
                    worker_entity
                );

                let hammer_id = commands
                    .spawn((
                        WorkerHammerIcon,
                        Sprite {
                            image: handles.hammer.clone(),
                            custom_size: Some(Vec2::splat(16.0)),
                            color: Color::srgb(1.0, 0.8, 0.2),
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
                commands
                    .entity(hammer_id)
                    .try_insert(ChildOf(worker_entity));

                commands.entity(worker_entity).insert(HasWorkerIndicator);
            }
        }
    }
}

pub fn update_worker_indicators_system(
    mut commands: Commands,
    time: Res<Time>,
    q_workers: Query<&SoulTaskVisualState, With<DamnedSoul>>,
    mut q_hammers: HammerIconsQuery,
) {
    for (hammer_entity, child_of, mut hammer_transform) in q_hammers.iter_mut() {
        let mut should_despawn = true;
        let worker_entity: Entity = child_of.parent();

        if let Ok(task_vs) = q_workers.get(worker_entity)
            && matches!(task_vs.phase, SoulTaskPhaseVisual::Build)
            && task_vs.link_target.is_some()
        {
            should_despawn = false;

            let bob = (time.elapsed_secs() * 5.0).sin() * 2.5;
            hammer_transform.translation =
                Vec3::new(0.0, 32.0 + bob, Z_VISUAL_EFFECT - Z_CHARACTER);
        }

        if should_despawn {
            info!("VISUAL: Despawning hammer for worker {:?}", worker_entity);
            commands.entity(hammer_entity).try_despawn();
            commands
                .entity(worker_entity)
                .try_remove::<HasWorkerIndicator>();
        }
    }
}
