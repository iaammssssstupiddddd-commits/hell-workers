//! ソウルのイベントオブザーバー（ハンドラ）

use super::*;
use crate::systems::soul_ai::execute::task_execution::AssignedTask;
use crate::world::map::WorldMapRead;
use crate::{OnExhausted, OnSoulRecruited, OnStressBreakdown};
use hw_core::constants::*;
use hw_core::relationships::CommandedBy;
use hw_soul_ai::unassign_task;
use rand::Rng;

type StressBreakdownSoulQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static mut DamnedSoul,
        &'static mut AssignedTask,
        &'static mut Path,
        Option<&'static mut crate::systems::logistics::Inventory>,
        Option<&'static hw_core::relationships::CommandedBy>,
    ),
>;

type ExhaustedSoulQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static mut IdleState,
        &'static mut AssignedTask,
        &'static mut Path,
        &'static mut Destination,
        Option<&'static mut crate::systems::logistics::Inventory>,
        Option<&'static hw_core::relationships::CommandedBy>,
    ),
>;

pub fn on_soul_recruited(
    on: On<OnSoulRecruited>,
    mut commands: Commands,
    mut q_souls: Query<(&mut DamnedSoul, &mut IdleState, &mut Path)>,
) {
    // このObserverはsquad_logic.rsからも発火される（タスク割り当てを介さないリクルート経路）。
    // apply_task_assignment_requests_systemのnormalize_worker_idle_stateと一部重複するが、
    // squad管理経路では同関数が呼ばれないため維持する。
    let soul_entity = on.entity;
    let event = on.event();
    if let Ok((_soul, mut idle, mut path)) = q_souls.get_mut(soul_entity) {
        idle.total_idle_time = 0.0;
        if idle.behavior == IdleBehavior::Drifting {
            idle.behavior = IdleBehavior::Wandering;
            idle.idle_timer = 0.0;
            idle.behavior_duration = 3.0;
        }
        path.waypoints.clear();
        path.current_index = 0;
        commands
            .entity(soul_entity)
            .remove::<crate::entities::damned_soul::DriftingState>();
    }
    info!(
        "OBSERVER: Soul {:?} recruited by Familiar {:?}",
        soul_entity, event.familiar_entity
    );
}

pub fn on_stress_breakdown(
    on: On<OnStressBreakdown>,
    mut commands: Commands,
    mut q_souls: StressBreakdownSoulQuery,
    world_map: WorldMapRead,
    mut queries: crate::systems::soul_ai::execute::task_execution::context::TaskUnassignQueries,
) {
    let soul_entity = on.entity;
    if let Ok((
        entity,
        transform,
        mut _soul,
        mut task,
        mut path,
        mut inventory_opt,
        under_command,
    )) = q_souls.get_mut(soul_entity)
    {
        info!("OBSERVER: Soul {:?} had a stress breakdown!", entity);

        commands.entity(entity).insert(StressBreakdown {
            is_frozen: true,
            remaining_freeze_secs: STRESS_BREAKDOWN_FREEZE_SECS,
        });

        if !matches!(*task, AssignedTask::None) {
            unassign_task(
                &mut commands,
                hw_soul_ai::SoulDropCtx {
                    soul_entity: entity,
                    drop_pos: transform.translation.truncate(),
                    inventory: inventory_opt.as_deref_mut(),
                    dropped_item_res: None,
                },
                &mut task,
                &mut path,
                &mut queries,
                world_map.as_ref(),
                false,
            );
        }

        if under_command.is_some() {
            commands.entity(entity).remove::<CommandedBy>();
        }
    }
}

pub fn on_exhausted(
    on: On<OnExhausted>,
    mut commands: Commands,
    q_spots: Query<&hw_soul_ai::soul_ai::helpers::gathering::GatheringSpot>,
    mut q_souls: ExhaustedSoulQuery,
    world_map: WorldMapRead,
    mut queries: crate::systems::soul_ai::execute::task_execution::context::TaskUnassignQueries,
) {
    let soul_entity = on.entity;
    if let Ok((
        entity,
        transform,
        mut idle,
        mut task,
        mut path,
        mut dest,
        mut inventory_opt,
        under_command_opt,
    )) = q_souls.get_mut(soul_entity)
    {
        info!(
            "OBSERVER: Soul {:?} is exhausted, heading to gathering area",
            entity
        );

        if under_command_opt.is_some() {
            commands.entity(entity).remove::<CommandedBy>();
        }

        if !matches!(*task, AssignedTask::None) {
            unassign_task(
                &mut commands,
                hw_soul_ai::SoulDropCtx {
                    soul_entity: entity,
                    drop_pos: transform.translation.truncate(),
                    inventory: inventory_opt.as_deref_mut(),
                    dropped_item_res: None,
                },
                &mut task,
                &mut path,
                &mut queries,
                world_map.as_ref(),
                false,
            );
        }

        if idle.behavior != IdleBehavior::ExhaustedGathering {
            if idle.behavior != IdleBehavior::Gathering {
                let mut rng = rand::thread_rng();
                idle.gathering_behavior = match rng.gen_range(0..4) {
                    0 => GatheringBehavior::Wandering,
                    1 => GatheringBehavior::Sleeping,
                    2 => GatheringBehavior::Standing,
                    _ => GatheringBehavior::Dancing,
                };
                idle.gathering_behavior_timer = 0.0;
                idle.gathering_behavior_duration = rng.gen_range(60.0..90.0);
                idle.needs_separation = true;
            }
            idle.behavior = IdleBehavior::ExhaustedGathering;
            idle.idle_timer = 0.0;
            let mut rng = rand::thread_rng();
            idle.behavior_duration = rng.gen_range(2.0..4.0);
        }

        // 最寄りの集会所を探す
        let current_pos = transform.translation.truncate();
        let gathering_center = q_spots
            .iter()
            .min_by(|a, b| {
                a.center
                    .distance_squared(current_pos)
                    .partial_cmp(&b.center.distance_squared(current_pos))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|s| s.center);

        if let Some(center) = gathering_center {
            let dist_from_center = (center - current_pos).length();

            if dist_from_center > TILE_SIZE * GATHERING_ARRIVAL_RADIUS_BASE {
                dest.0 = center;
                path.waypoints.clear();
                path.current_index = 0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_core::events::{
        OnReleasedFromService, OnTaskAbandoned, ResourceReservationOp, ResourceReservationRequest,
    };
    use hw_core::relationships::{DeliveringTo, WorkingOn};
    use hw_jobs::{ActiveTaskIdentity, HaulData, HaulPhase, WorkType};
    use hw_logistics::{Inventory, ResourceItem, ResourceType, SharedResourceCache};
    use hw_world::WorldMap;

    #[test]
    fn observers_unassign_without_assignment_messages_test() {
        for exhausted in [false, true] {
            let mut app = App::new();
            app.init_resource::<WorldMap>()
                .init_resource::<SharedResourceCache>()
                .add_message::<ResourceReservationRequest>()
                .add_message::<OnReleasedFromService>()
                .add_message::<OnTaskAbandoned>()
                .add_observer(on_stress_breakdown)
                .add_observer(on_exhausted);
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
                    DamnedSoul::default(),
                    IdleState::default(),
                    Destination(Vec2::ZERO),
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
            if exhausted {
                app.world_mut().trigger(OnExhausted { entity: soul });
            } else {
                app.world_mut().trigger(OnStressBreakdown { entity: soul });
            }
            app.world_mut().flush();
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
                matches!(ops.as_slice(), [ResourceReservationOp::ReleaseSource { source, .. }]
                if *source == hw_core::logistics::ResourceSourceKey::Entity(item))
            );
            assert_eq!(
                app.world()
                    .resource::<Messages<OnReleasedFromService>>()
                    .len(),
                0
            );
            assert_eq!(app.world().resource::<Messages<OnTaskAbandoned>>().len(), 0);
            if exhausted {
                assert_eq!(
                    app.world().get::<IdleState>(soul).unwrap().behavior,
                    IdleBehavior::ExhaustedGathering
                );
            } else {
                let stress = app.world().get::<StressBreakdown>(soul).unwrap();
                assert!(stress.is_frozen);
                assert_eq!(stress.remaining_freeze_secs, STRESS_BREAKDOWN_FREEZE_SECS);
            }
        }
    }
}
