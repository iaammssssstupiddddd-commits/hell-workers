//! Work helper facade.
//!
//! 純粋な可否判定は `hw_ai` にあり、`unassign_task` の公開 API は
//! root crate 側で所有する。

use bevy::prelude::*;

use crate::events::OnTaskAbandoned;
use crate::systems::soul_ai::execute::task_execution::context::TaskReservationAccess;

pub use hw_ai::soul_ai::helpers::work::is_soul_available_for_work;

pub fn unassign_task<'w, 's, Q: TaskReservationAccess<'w, 's>>(
    commands: &mut Commands,
    soul_entity: Entity,
    drop_pos: Vec2,
    task: &mut crate::systems::soul_ai::execute::task_execution::AssignedTask,
    path: &mut crate::entities::damned_soul::Path,
    inventory: Option<&mut crate::systems::logistics::Inventory>,
    dropped_item_res: Option<crate::systems::logistics::ResourceType>,
    queries: &mut Q,
    world_map: &crate::world::map::WorldMap,
    emit_abandoned_event: bool,
) {
    if emit_abandoned_event
        && !matches!(
            *task,
            crate::systems::soul_ai::execute::task_execution::AssignedTask::None
        )
    {
        commands.trigger(OnTaskAbandoned {
            entity: soul_entity,
        });
    }

    hw_ai::soul_ai::helpers::work::cleanup_task_assignment(
        commands,
        soul_entity,
        drop_pos,
        task,
        path,
        inventory,
        dropped_item_res,
        queries,
        world_map,
        false,
    );

    commands
        .entity(soul_entity)
        .remove::<crate::relationships::WorkingOn>();
}
