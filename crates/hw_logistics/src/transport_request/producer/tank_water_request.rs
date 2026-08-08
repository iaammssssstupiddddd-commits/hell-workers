//! Tank water request system

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::constants::BUCKET_CAPACITY;

use hw_core::relationships::{IncomingDeliveries, StoredItems, TaskWorkers};
use hw_jobs::{DeconstructionPending, Designation, MovePlanned, Priority, TaskSlots, WorkType};

use crate::transport_request::producer::active_unit_cache::{
    CachedActiveFamiliars, CachedActiveYards,
};
use crate::transport_request::{
    TransportDemand, TransportPolicy, TransportPriority, TransportRequest, TransportRequestKind,
    TransportRequestState,
};
use crate::types::ResourceType;
use crate::water::tank_can_accept_new_bucket;
use crate::zone::Stockpile;

#[derive(SystemParam)]
pub struct TankWaterRequestParams<'w, 's> {
    commands: Commands<'w, 's>,
    q_incoming: Query<'w, 's, &'static IncomingDeliveries>,
    familiars_cache: Res<'w, CachedActiveFamiliars>,
    yards_cache: Res<'w, CachedActiveYards>,
    q_tanks: Query<
        'w,
        's,
        (
            Entity,
            &'static Transform,
            &'static Stockpile,
            Option<&'static StoredItems>,
        ),
    >,
    q_tank_requests: Query<
        'w,
        's,
        (
            Entity,
            &'static TransportRequest,
            Option<&'static TaskWorkers>,
        ),
    >,
    q_move_planned: Query<'w, 's, (), With<MovePlanned>>,
    q_deconstruction_pending: Query<'w, 's, (), With<DeconstructionPending>>,
}

pub fn tank_water_request_system(params: TankWaterRequestParams) {
    let TankWaterRequestParams {
        mut commands,
        q_incoming,
        familiars_cache,
        yards_cache,
        q_tanks,
        q_tank_requests,
        q_move_planned,
        q_deconstruction_pending,
    } = params;
    let active_familiars = &familiars_cache.data;
    let active_yards = &yards_cache.data;
    let all_owners = super::collect_all_area_owners(active_familiars, active_yards);

    let mut desired_requests = std::collections::HashMap::<Entity, (Entity, u32, Vec2)>::new();

    for (tank_entity, tank_transform, tank_stock, stored_opt) in q_tanks.iter() {
        if q_move_planned.get(tank_entity).is_ok()
            || q_deconstruction_pending.get(tank_entity).is_ok()
        {
            continue;
        }
        if tank_stock.resource_type != Some(ResourceType::Water) {
            continue;
        }

        let tank_pos = tank_transform.translation.truncate();
        let Some((fam_entity, _)) = super::find_owner(tank_pos, &all_owners) else {
            continue;
        };

        let current_water = stored_opt.map(|s| s.len()).unwrap_or(0);
        let incoming_water_tasks = q_incoming
            .get(tank_entity)
            .ok()
            .map(|inc: &IncomingDeliveries| inc.len())
            .unwrap_or(0);
        let total_water = (current_water as u32) + (incoming_water_tasks as u32 * BUCKET_CAPACITY);

        if tank_can_accept_new_bucket(current_water, incoming_water_tasks, tank_stock.capacity) {
            let needed_water = tank_stock.capacity as u32 - total_water;
            let needed_tasks = needed_water / BUCKET_CAPACITY;

            if needed_tasks > 0 {
                desired_requests.insert(tank_entity, (fam_entity, needed_tasks, tank_pos));
            }
        }
    }

    let mut seen_existing = std::collections::HashSet::<Entity>::new();

    for (request_entity, request, workers_opt) in q_tank_requests.iter() {
        if request.kind != TransportRequestKind::GatherWaterToTank {
            continue;
        }
        let tank_entity = request.anchor;
        let workers = workers_opt.map(|w| w.len()).unwrap_or(0);

        if !super::upsert::process_duplicate_key(
            &mut commands,
            request_entity,
            workers,
            &mut seen_existing,
            tank_entity,
        ) {
            continue;
        }

        if let Some((issued_by, slots, tank_pos)) = desired_requests.get(&tank_entity) {
            commands.entity(request_entity).try_insert((
                Transform::from_xyz(tank_pos.x, tank_pos.y, 0.0),
                Visibility::Hidden,
                Designation {
                    work_type: WorkType::GatherWater,
                },
                hw_core::relationships::ManagedBy(*issued_by),
                TaskSlots::new(*slots),
                Priority(3),
                TransportRequest {
                    kind: TransportRequestKind::GatherWaterToTank,
                    anchor: tank_entity,
                    resource_type: ResourceType::Water,
                    issued_by: *issued_by,
                    priority: TransportPriority::Normal,
                    stockpile_group: vec![],
                },
                TransportDemand {
                    desired_slots: *slots,
                    inflight: 0,
                },
                TransportRequestState::Pending,
                TransportPolicy::default(),
            ));
            continue;
        }

        if workers == 0 {
            super::upsert::disable_request(&mut commands, request_entity);
        }
    }

    for (tank_entity, (issued_by, slots, tank_pos)) in desired_requests {
        if seen_existing.contains(&tank_entity) {
            continue;
        }

        commands.spawn((
            Name::new("TransportRequest::GatherWaterToTank"),
            Transform::from_xyz(tank_pos.x, tank_pos.y, 0.0),
            Visibility::Hidden,
            Designation {
                work_type: WorkType::GatherWater,
            },
            hw_core::relationships::ManagedBy(issued_by),
            TaskSlots::new(slots),
            Priority(3),
            TransportRequest {
                kind: TransportRequestKind::GatherWaterToTank,
                anchor: tank_entity,
                resource_type: ResourceType::Water,
                issued_by,
                priority: TransportPriority::Normal,
                stockpile_group: vec![],
            },
            TransportDemand {
                desired_slots: slots,
                inflight: 0,
            },
            TransportRequestState::Pending,
            TransportPolicy::default(),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_world::Yard;

    #[test]
    fn pending_tank_produces_no_water_request_while_live_tank_does() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<CachedActiveFamiliars>()
            .init_resource::<CachedActiveYards>()
            .add_systems(Update, tank_water_request_system);
        let yard_entity = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<CachedActiveYards>()
            .data
            .push((
                yard_entity,
                Yard {
                    min: Vec2::splat(-100.0),
                    max: Vec2::splat(100.0),
                },
            ));
        let spawn_tank = |app: &mut App, x: f32| {
            app.world_mut()
                .spawn((
                    Transform::from_xyz(x, 0.0, 0.0),
                    Stockpile {
                        capacity: 12,
                        resource_type: Some(ResourceType::Water),
                    },
                ))
                .id()
        };
        let pending = spawn_tank(&mut app, 0.0);
        let live = spawn_tank(&mut app, 20.0);
        let order = app.world_mut().spawn_empty().id();
        app.world_mut()
            .entity_mut(pending)
            .insert(DeconstructionPending { order });

        app.update();

        let mut requests = app.world_mut().query::<&TransportRequest>();
        let anchors = requests
            .iter(app.world())
            .filter(|request| request.kind == TransportRequestKind::GatherWaterToTank)
            .map(|request| request.anchor)
            .collect::<Vec<_>>();
        assert_eq!(anchors, vec![live]);
    }
}
