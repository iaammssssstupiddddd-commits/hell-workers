//! 指揮元の使い魔が存在しない場合に、使役状態の魂をクリーンアップする。

use bevy::prelude::*;
use hw_core::events::OnReleasedFromService;
use hw_core::familiar::Familiar;
use hw_core::relationships::CommandedBy;
use hw_world::WorldMapRead;

use crate::soul_ai::execute::task_execution::context::TaskUnassignQueries;
use crate::soul_ai::helpers::query_types::CleanupSoulQuery;
use crate::soul_ai::helpers::work::{SoulDropCtx, unassign_task};

/// 指揮元の使い魔が存在しない場合に、使役状態の魂をクリーンアップする
pub fn cleanup_commanded_souls_system(
    mut commands: Commands,
    mut q_souls: CleanupSoulQuery,
    mut queries: TaskUnassignQueries,
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

        commands.write_message(OnReleasedFromService {
            entity: soul_entity,
        });

        commands.entity(soul_entity).remove::<CommandedBy>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_core::events::{OnTaskAbandoned, ResourceReservationOp, ResourceReservationRequest};
    use hw_core::relationships::{DeliveringTo, WorkingOn};
    use hw_core::soul::Path;
    use hw_jobs::{ActiveTaskIdentity, AssignedTask, HaulData, HaulPhase, WorkType};
    use hw_logistics::{Inventory, ResourceItem, ResourceType, SharedResourceCache};
    use hw_world::WorldMap;

    #[test]
    fn cleanup_unassigns_without_assignment_messages_test() {
        let mut app = App::new();
        app.init_resource::<WorldMap>()
            .init_resource::<SharedResourceCache>()
            .add_message::<ResourceReservationRequest>()
            .add_message::<OnReleasedFromService>()
            .add_message::<OnTaskAbandoned>()
            .add_systems(Update, cleanup_commanded_souls_system);
        let commander = app.world_mut().spawn_empty().id();
        let destination = app.world_mut().spawn_empty().id();
        let item = app
            .world_mut()
            .spawn((
                Transform::default(),
                Visibility::Visible,
                ResourceItem(ResourceType::Wood),
                DeliveringTo(destination),
            ))
            .id();
        let soul = app
            .world_mut()
            .spawn((
                Transform::default(),
                CommandedBy(commander),
                AssignedTask::Haul(HaulData {
                    item,
                    stockpile: destination,
                    phase: HaulPhase::GoingToItem,
                }),
                Path::default(),
                Inventory::default(),
                WorkingOn(item),
                ActiveTaskIdentity::new(item, item, WorkType::Haul),
            ))
            .id();
        app.update();
        assert!(matches!(
            app.world().get::<AssignedTask>(soul),
            Some(AssignedTask::None)
        ));
        assert!(app.world().get::<WorkingOn>(soul).is_none());
        assert!(app.world().get::<ActiveTaskIdentity>(soul).is_none());
        assert!(app.world().get::<CommandedBy>(soul).is_none());
        assert!(app.world().get::<DeliveringTo>(item).is_none());
        let reservations = app
            .world()
            .resource::<Messages<ResourceReservationRequest>>();
        let mut cursor = reservations.get_cursor();
        let ops: Vec<_> = cursor
            .read(reservations)
            .map(|request| &request.op)
            .collect();
        assert!(
            matches!(ops.as_slice(), [ResourceReservationOp::ReleaseSource { source, .. }] if *source == hw_core::logistics::ResourceSourceKey::Entity(item))
        );
        assert_eq!(
            app.world()
                .resource::<Messages<OnReleasedFromService>>()
                .len(),
            1
        );
        assert_eq!(app.world().resource::<Messages<OnTaskAbandoned>>().len(), 0);
    }
}
