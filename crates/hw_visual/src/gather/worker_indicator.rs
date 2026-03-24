//! 伐採・採掘中のワーカーインジケータ（斧/ツルハシアイコン）

use bevy::prelude::*;

use super::components::{HasGatherIndicator, WorkerGatherIcon};
use super::{
    COLOR_CHOP_ICON, COLOR_MINE_ICON, GATHER_ICON_BOB_AMPLITUDE, GATHER_ICON_BOB_SPEED,
    GATHER_ICON_SIZE, GATHER_ICON_Y_OFFSET,
};
use crate::handles::WorkIconHandles;
use crate::worker_icon::{
    WorkerIcon, WorkerIconConfig, spawn_worker_icon, update_worker_icon_position,
};
use hw_core::soul::DamnedSoul;
use hw_core::visual_mirror::task::{SoulTaskPhaseVisual, SoulTaskVisualState};

#[allow(clippy::type_complexity)]
pub fn spawn_gather_indicators_system(
    mut commands: Commands,
    handles: Res<WorkIconHandles>,
    q_workers: Query<
        (Entity, &SoulTaskVisualState, &Transform),
        (With<DamnedSoul>, Without<HasGatherIndicator>),
    >,
) {
    for (worker_entity, task_vs, transform) in q_workers.iter() {
        let (icon_handle, icon_color) = match task_vs.phase {
            SoulTaskPhaseVisual::GatherChop if task_vs.progress.is_some() => {
                (handles.axe.clone(), COLOR_CHOP_ICON)
            }
            SoulTaskPhaseVisual::GatherMine if task_vs.progress.is_some() => {
                (handles.pick.clone(), COLOR_MINE_ICON)
            }
            _ => continue,
        };

        info!(
            "VISUAL: Spawning gather icon for worker {:?} ({:?})",
            worker_entity, task_vs.phase
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

        commands.entity(icon_entity).insert(WorkerGatherIcon {
            worker: worker_entity,
        });

        commands
            .entity(worker_entity)
            .try_insert(HasGatherIndicator);
    }
}

pub fn update_gather_indicators_system(
    mut commands: Commands,
    time: Res<Time>,
    q_workers: Query<(Entity, &SoulTaskVisualState, &Transform), With<DamnedSoul>>,
    mut q_icons: Query<
        (Entity, &WorkerGatherIcon, &WorkerIcon, &mut Transform),
        Without<DamnedSoul>,
    >,
) {
    for (icon_entity, gather_icon, worker_icon, mut icon_transform) in q_icons.iter_mut() {
        let mut should_despawn = true;

        if let Ok((_w_entity, task_vs, worker_transform)) = q_workers.get(gather_icon.worker) {
            let is_collecting = matches!(
                task_vs.phase,
                SoulTaskPhaseVisual::GatherChop | SoulTaskPhaseVisual::GatherMine
            ) && task_vs.progress.is_some();

            if is_collecting {
                should_despawn = false;

                update_worker_icon_position(
                    &time,
                    Some(worker_transform),
                    worker_icon,
                    &mut icon_transform,
                );
            }
        }

        if should_despawn {
            info!(
                "VISUAL: Despawning gather icon for worker {:?}",
                gather_icon.worker
            );
            commands.entity(icon_entity).try_despawn();
            if let Ok(mut entity_commands) = commands.get_entity(gather_icon.worker) {
                entity_commands.try_remove::<HasGatherIndicator>();
            }
        }
    }
}
