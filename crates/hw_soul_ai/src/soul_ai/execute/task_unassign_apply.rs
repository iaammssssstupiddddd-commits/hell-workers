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
use crate::soul_ai::helpers::work::{SoulDropCtx, unassign_task};

type TaskUnassignSoulQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static mut AssignedTask,
        &'static mut Path,
        Option<&'static mut Inventory>,
    ),
    With<DamnedSoul>,
>;

/// `SoulTaskUnassignRequest` を受け取り、対象の魂のタスクを解除する。
pub fn handle_soul_task_unassign_system(
    mut request_reader: MessageReader<SoulTaskUnassignRequest>,
    mut q_souls: TaskUnassignSoulQuery,
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
                SoulDropCtx {
                    soul_entity: entity,
                    drop_pos: transform.translation.truncate(),
                    inventory: inventory_opt.as_deref_mut(),
                    dropped_item_res: None,
                },
                &mut task,
                &mut path,
                &mut queries,
                world_map.as_ref(),
                req.emit_abandoned,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::handle_soul_task_unassign_system;
    use bevy::ecs::schedule::ApplyDeferred;
    use bevy::prelude::*;
    use hw_core::events::{OnTaskAbandoned, ResourceReservationRequest, SoulTaskUnassignRequest};
    use hw_core::relationships::WorkingOn;
    use hw_core::soul::{DamnedSoul, Path};
    use hw_jobs::{ActiveTaskIdentity, GeneratePowerData, GeneratePowerPhase, WorkType};
    use hw_logistics::SharedResourceCache;
    use hw_world::WorldMap;

    use crate::soul_ai::execute::task_execution::AssignedTask;

    #[derive(Resource, Default)]
    struct Receipts {
        abandoned: Vec<OnTaskAbandoned>,
        reservation_count: usize,
    }

    fn collect_receipts(
        mut abandoned: MessageReader<OnTaskAbandoned>,
        mut reservations: MessageReader<ResourceReservationRequest>,
        mut receipts: ResMut<Receipts>,
    ) {
        receipts.abandoned.extend(abandoned.read().copied());
        receipts.reservation_count += reservations.read().count();
    }

    #[test]
    fn user_unassign_cleans_assignment_before_abandonment_notification() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(WorldMap::default())
            .init_resource::<SharedResourceCache>()
            .init_resource::<Receipts>()
            .add_message::<SoulTaskUnassignRequest>()
            .add_message::<ResourceReservationRequest>()
            .add_message::<OnTaskAbandoned>()
            .add_systems(
                Update,
                (
                    handle_soul_task_unassign_system,
                    ApplyDeferred,
                    collect_receipts,
                )
                    .chain(),
            );

        let target = app.world_mut().spawn_empty().id();
        let soul = app
            .world_mut()
            .spawn((
                Transform::default(),
                DamnedSoul::default(),
                AssignedTask::GeneratePower(GeneratePowerData {
                    tile: target,
                    tile_pos: Vec2::ZERO,
                    phase: GeneratePowerPhase::Generating,
                }),
                Path::default(),
                ActiveTaskIdentity::new(target, target, WorkType::GeneratePower),
                WorkingOn(target),
            ))
            .id();
        app.world_mut().write_message(SoulTaskUnassignRequest {
            soul_entity: soul,
            emit_abandoned: true,
        });

        app.update();

        let receipts = app.world().resource::<Receipts>();
        assert_eq!(receipts.abandoned, vec![OnTaskAbandoned { entity: soul }]);
        assert_eq!(receipts.reservation_count, 1);
        assert!(matches!(
            app.world().get::<AssignedTask>(soul),
            Some(AssignedTask::None)
        ));
        assert!(app.world().get::<ActiveTaskIdentity>(soul).is_none());
        assert!(app.world().get::<WorkingOn>(soul).is_none());
    }
}
