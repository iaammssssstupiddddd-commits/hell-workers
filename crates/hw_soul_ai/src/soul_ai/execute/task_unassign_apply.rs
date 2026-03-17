//! 魂タスク解除要求の適用システム。
//!
//! `hw_familiar_ai` が `SoulTaskUnassignRequest` を送信し、
//! Soul AI の Perceive フェーズでこのシステムがそれを処理する。
//! これにより hw_familiar_ai → hw_soul_ai の直接依存を排除する。

use bevy::prelude::*;
use hw_core::events::SoulTaskUnassignRequest;
use hw_core::soul::{DamnedSoul, Path};
use hw_logistics::Inventory;
use hw_world::WorldMapRead;

use crate::soul_ai::execute::task_execution::{AssignedTask, TaskUnassignQueries};
use crate::soul_ai::helpers::work::unassign_task;

/// `SoulTaskUnassignRequest` を受け取り、対象の魂のタスクを解除する。
pub fn handle_soul_task_unassign_system(
    mut request_reader: MessageReader<SoulTaskUnassignRequest>,
    mut q_souls: Query<
        (
            Entity,
            &Transform,
            &mut AssignedTask,
            &mut Path,
            Option<&mut Inventory>,
        ),
        With<DamnedSoul>,
    >,
    mut queries: TaskUnassignQueries,
    world_map: WorldMapRead,
    mut commands: Commands,
) {
    for req in request_reader.read() {
        if let Ok((entity, transform, mut task, mut path, mut inventory_opt)) =
            q_souls.get_mut(req.soul_entity)
        {
            unassign_task(
                &mut commands,
                entity,
                transform.translation.truncate(),
                &mut task,
                &mut path,
                inventory_opt.as_deref_mut(),
                None,
                &mut queries,
                world_map.as_ref(),
                req.emit_abandoned,
            );
        }
    }
}
