use bevy::prelude::*;

use crate::entities::damned_soul::Path;
use crate::entities::familiar::{ActiveCommand, Familiar, FamiliarCommand, UnderCommand};
use crate::systems::familiar_ai::haul_cache::HaulReservationCache;

use crate::systems::soul_ai::task_execution::AssignedTask;
use crate::systems::soul_ai::work::helpers;

/// 使い魔が Idle コマンドの場合、または使い魔が存在しない場合に部下をリリースする
pub fn cleanup_commanded_souls_system(
    mut commands: Commands,
    mut q_souls: Query<(
        Entity,
        &Transform,
        &UnderCommand,
        &mut AssignedTask,
        &mut Path,
        Option<&mut crate::systems::logistics::Inventory>,
    )>,
    queries: crate::systems::soul_ai::task_execution::context::TaskQueries,
    q_familiars: Query<&ActiveCommand, With<Familiar>>,
    mut haul_cache: ResMut<HaulReservationCache>,
    world_map: Res<crate::world::map::WorldMap>,
) {
    for (soul_entity, transform, under_command, mut task, mut path, mut inventory_opt) in
        q_souls.iter_mut()
    {
        let should_release = match q_familiars.get(under_command.0) {
            Ok(active_cmd) => matches!(active_cmd.command, FamiliarCommand::Idle),
            Err(_) => true,
        };

        if should_release {
            info!(
                "RELEASE: Soul {:?} released from Familiar {:?}",
                soul_entity, under_command.0
            );

            helpers::unassign_task(
                &mut commands,
                soul_entity,
                transform.translation.truncate(),
                &mut task,
                &mut path,
                inventory_opt.as_deref_mut(),
                None,
                &queries,
                &mut *haul_cache,
                &world_map,
                false, // emit_abandoned_event: 解放時は個別のタスク中断セリフを出さない
            );

            commands.trigger(crate::events::OnReleasedFromService {
                entity: soul_entity,
            });

            commands.entity(soul_entity).remove::<UnderCommand>();
        }
    }
}
