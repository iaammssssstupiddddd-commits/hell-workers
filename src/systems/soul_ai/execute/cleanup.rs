use bevy::prelude::*;

use crate::entities::familiar::Familiar;
use crate::relationships::CommandedBy;
// use crate::systems::familiar_ai::perceive::resource_sync::SharedResourceCache; // Removed unused import

use crate::systems::soul_ai::helpers::query_types::CleanupSoulQuery;
use crate::systems::soul_ai::helpers::work as helpers;

/// 指揮元の使い魔が存在しない場合に、使役状態の魂をクリーンアップする
pub fn cleanup_commanded_souls_system(
    mut commands: Commands,
    mut q_souls: CleanupSoulQuery,
    mut queries: crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    q_familiars: Query<(), With<Familiar>>,
    world_map: Res<crate::world::map::WorldMap>,
) {
    for (soul_entity, transform, under_command, mut task, mut path, mut inventory_opt) in
        q_souls.iter_mut()
    {
        if q_familiars.get(under_command.0).is_ok() {
            continue;
        }

        info!(
            "RELEASE: Soul {:?} released from missing Familiar {:?}",
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
            &mut queries,
            // haul_cache removed
            &world_map,
            false, // emit_abandoned_event: 解放時は個別のタスク中断セリフを出さない
        );

        commands.trigger(crate::events::OnReleasedFromService {
            entity: soul_entity,
        });

        commands.entity(soul_entity).remove::<CommandedBy>();
    }
}
