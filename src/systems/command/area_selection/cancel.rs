//! cancel_single_designation と補助処理

use crate::events::OnTaskAbandoned;
use crate::relationships::{ManagedBy, TaskWorkers, WorkingOn};
use crate::systems::jobs::{Designation, TaskSlots};
use crate::systems::logistics::transport_request::ManualHaulPinnedSource;
use bevy::prelude::*;

/// Designation/Blueprint/TransportRequest を 1 件キャンセル
pub fn cancel_single_designation(
    commands: &mut Commands,
    target_entity: Entity,
    task_workers: Option<&TaskWorkers>,
    is_blueprint: bool,
    is_transport_request: bool,
    fixed_source: Option<Entity>,
) {
    fn trigger_task_abandoned_if_alive(commands: &mut Commands, soul: Entity) {
        commands.queue(move |world: &mut World| {
            if world.get_entity(soul).is_ok() {
                world.trigger(OnTaskAbandoned { entity: soul });
            }
        });
    }

    if let Some(workers) = task_workers {
        for &soul in workers.iter() {
            commands.entity(soul).try_remove::<WorkingOn>();
            trigger_task_abandoned_if_alive(commands, soul);
        }
    }

    if let Some(source_entity) = fixed_source {
        commands
            .entity(source_entity)
            .try_remove::<ManualHaulPinnedSource>();
    }

    if is_blueprint || is_transport_request {
        commands.entity(target_entity).try_despawn();
    } else {
        commands
            .entity(target_entity)
            .try_remove::<(Designation, TaskSlots, ManagedBy)>();
    }
}
