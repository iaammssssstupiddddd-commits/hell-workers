//! 指揮元の使い魔が存在しない場合に、使役状態の魂をクリーンアップする。

use bevy::prelude::*;
use hw_core::events::OnReleasedFromService;
use hw_core::familiar::Familiar;
use hw_core::relationships::CommandedBy;
use hw_world::WorldMapRead;

use crate::soul_ai::execute::task_execution::context::TaskAssignmentQueries;
use crate::soul_ai::helpers::query_types::CleanupSoulQuery;
use crate::soul_ai::helpers::work::{SoulDropCtx, unassign_task};

/// 指揮元の使い魔が存在しない場合に、使役状態の魂をクリーンアップする
pub fn cleanup_commanded_souls_system(
    mut commands: Commands,
    mut q_souls: CleanupSoulQuery,
    mut queries: TaskAssignmentQueries,
    q_familiars: Query<(), With<Familiar>>,
    world_map: WorldMapRead,
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

        unassign_task(
            &mut commands,
            SoulDropCtx {
                soul_entity,
                drop_pos: transform.translation.truncate(),
                inventory: inventory_opt.as_deref_mut(),
                dropped_item_res: None,
            },
            &mut task,
            &mut path,
            &mut queries,
            world_map.as_ref(),
            false, // emit_abandoned_event: 解放時は個別のタスク中断セリフを出さない
        );

        commands.trigger(OnReleasedFromService {
            entity: soul_entity,
        });

        commands.entity(soul_entity).remove::<CommandedBy>();
    }
}
