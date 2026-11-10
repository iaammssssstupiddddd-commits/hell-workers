use bevy::prelude::*;
use hw_core::area::TaskArea;
use hw_core::logistics::ResourceType;
use hw_logistics::water::tank_can_accept_new_bucket;

use super::finder::{find_nearest_bucket_for_tank, find_nearest_water_bucket_for_tank};
use crate::familiar_ai::decide::task_management::{
    FamiliarTaskAssignmentQueries, ReservationShadow,
};

pub fn resolve_gather_water_inputs(
    task_entity: Entity,
    task_pos: Vec2,
    _task_area_opt: Option<&TaskArea>,
    queries: &FamiliarTaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> Option<(Entity, Entity)> {
    use hw_logistics::transport_request::TransportRequestKind;
    let req = queries.transport_requests.get(task_entity).ok()?;
    if req.kind != TransportRequestKind::GatherWaterToTank {
        return None;
    }

    let tank_entity = req.anchor;
    let Ok((_, _, tank_stock, stored_opt)) = queries.storage.stockpiles.get(tank_entity) else {
        return None;
    };
    if tank_stock.resource_type != Some(ResourceType::Water) {
        return None;
    }
    let current_water = stored_opt.map(|s| s.len()).unwrap_or(0);
    let incoming_buckets = queries
        .reservation
        .incoming_deliveries_query
        .get(tank_entity)
        .map(|(_, inc)| inc.len())
        .unwrap_or(0);
    if !tank_can_accept_new_bucket(current_water, incoming_buckets, tank_stock.capacity) {
        return None;
    }

    let (bucket_entity, _) = find_nearest_bucket_for_tank(tank_entity, task_pos, queries, shadow)?;
    Some((bucket_entity, tank_entity))
}

pub fn resolve_haul_water_to_mixer_inputs(
    task_entity: Entity,
    task_pos: Vec2,
    task_area_opt: Option<&TaskArea>,
    queries: &FamiliarTaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> Option<(Entity, Entity, Entity)> {
    use hw_logistics::transport_request::TransportRequestKind;
    let req = queries.transport_requests.get(task_entity).ok()?;
    if req.kind != TransportRequestKind::DeliverWaterToMixer {
        return None;
    }
    let mixer_entity = req.anchor;
    queries.storage.mixers.get(mixer_entity).ok()?;
    let (_, _, stockpile, stored) = queries.storage.stockpiles.get(mixer_entity).ok()?;
    let reserved = queries
        .reservation
        .resource_cache
        .get_mixer_destination_reservation(mixer_entity, ResourceType::Water)
        .saturating_add(shadow.mixer_reserved(mixer_entity, ResourceType::Water));
    if stockpile.resource_type != Some(ResourceType::Water)
        || !tank_can_accept_new_bucket(
            stored.map_or(0, |items| items.len()),
            reserved,
            stockpile.capacity,
        )
    {
        return None;
    }

    find_tank_bucket_for_water_mixer(mixer_entity, task_pos, task_area_opt, queries, shadow)
        .map(|(tank, bucket)| (mixer_entity, tank, bucket))
}

fn find_tank_bucket_for_water_mixer(
    _mixer_entity: Entity,
    mixer_pos: Vec2,
    task_area_opt: Option<&TaskArea>,
    queries: &FamiliarTaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> Option<(Entity, Entity)> {
    let yard_opt = task_area_opt
        .and_then(|area| {
            let center = area.center();
            queries.yards.iter().find(|yard| yard.contains(center))
        })
        .or_else(|| queries.yards.iter().next());

    let mut tank_candidates: Vec<(Entity, f32)> = queries
        .storage
        .stockpiles
        .iter()
        .filter(|(_s_entity, s_transform, stock, _stored)| {
            if stock.resource_type != Some(ResourceType::Water) {
                return false;
            }
            let tank_pos = s_transform.translation.truncate();
            let in_task_area = task_area_opt.is_some_and(|area| area.contains(tank_pos));
            let in_yard = yard_opt.is_some_and(|yard| yard.contains(tank_pos));
            if (task_area_opt.is_some() || yard_opt.is_some()) && !in_task_area && !in_yard {
                return false;
            }
            true
        })
        .map(|(e, t, _, _)| {
            let d = t.translation.truncate().distance_squared(mixer_pos);
            (e, d)
        })
        .collect();
    tank_candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    for (tank_entity, _) in tank_candidates {
        if let Some((bucket_entity, _)) =
            find_nearest_water_bucket_for_tank(tank_entity, mixer_pos, queries, shadow)
        {
            return Some((tank_entity, bucket_entity));
        }

        let has_stored_water = queries
            .storage
            .stockpiles
            .get(tank_entity)
            .ok()
            .is_some_and(|(_, _, stock, stored)| {
                stock.resource_type == Some(ResourceType::Water)
                    && stored.map(|s| s.len()).unwrap_or(0)
                        >= hw_core::constants::BUCKET_CAPACITY as usize
            });
        if !has_stored_water {
            continue;
        }

        if let Some((bucket_entity, _)) =
            find_nearest_bucket_for_tank(tank_entity, mixer_pos, queries, shadow)
        {
            return Some((tank_entity, bucket_entity));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_core::events::{ResourceReservationOp, ResourceReservationRequest};
    use hw_core::relationships::StoredIn;
    use hw_jobs::{events::TaskAssignmentRequest, mud_mixer::MudMixerStorage};
    use hw_logistics::transport_request::{
        TransportPriority, TransportRequest, TransportRequestKind,
        WheelbarrowArbitrationDiagnostics,
    };
    use hw_logistics::{BelongsTo, ResourceItem, SharedResourceCache, Stockpile};
    use hw_world::WorldMap;

    #[derive(Resource)]
    struct Probe {
        request: Entity,
        shadow: usize,
        result: Option<(Entity, Entity, Entity)>,
    }

    fn probe(mut state: ResMut<Probe>, queries: FamiliarTaskAssignmentQueries) {
        let mixer = queries
            .transport_requests
            .get(state.request)
            .unwrap()
            .anchor;
        let mut shadow = ReservationShadow::default();
        for _ in 0..state.shadow {
            shadow.apply_reserve_ops(&[ResourceReservationOp::ReserveMixerDestination {
                target: mixer,
                resource_type: ResourceType::Water,
            }]);
        }
        state.result =
            resolve_haul_water_to_mixer_inputs(state.request, Vec2::ZERO, None, &queries, &shadow);
    }

    fn admitted(
        current: usize,
        capacity: usize,
        tank_count: usize,
        full: bool,
        cached: usize,
        shadow: usize,
    ) -> bool {
        let mut app = App::new();
        app.init_resource::<WorldMap>()
            .init_resource::<SharedResourceCache>()
            .init_resource::<WheelbarrowArbitrationDiagnostics>()
            .add_message::<ResourceReservationRequest>()
            .add_message::<TaskAssignmentRequest>()
            .add_systems(Update, probe);
        let tank = app
            .world_mut()
            .spawn((
                Transform::from_xyz(-32., 0., 0.),
                Stockpile {
                    capacity: 50,
                    resource_type: Some(ResourceType::Water),
                },
            ))
            .id();
        let mixer = app
            .world_mut()
            .spawn((
                Transform::default(),
                MudMixerStorage::default(),
                Stockpile {
                    capacity,
                    resource_type: Some(ResourceType::Water),
                },
            ))
            .id();
        for (owner, count) in [(tank, tank_count), (mixer, current)] {
            for _ in 0..count {
                app.world_mut()
                    .spawn((ResourceItem(ResourceType::Water), StoredIn(owner)));
            }
        }
        app.world_mut().spawn((
            Transform::default(),
            Visibility::Visible,
            BelongsTo(tank),
            ResourceItem(if full {
                ResourceType::BucketWater
            } else {
                ResourceType::BucketEmpty
            }),
        ));
        let request = app
            .world_mut()
            .spawn(TransportRequest {
                kind: TransportRequestKind::DeliverWaterToMixer,
                anchor: mixer,
                resource_type: ResourceType::Water,
                issued_by: tank,
                priority: TransportPriority::Normal,
                stockpile_group: vec![],
            })
            .id();
        for _ in 0..cached {
            app.world_mut()
                .resource_mut::<SharedResourceCache>()
                .reserve_mixer_destination(mixer, ResourceType::Water);
        }
        app.insert_resource(Probe {
            request,
            shadow,
            result: None,
        });
        app.update();
        app.world().resource::<Probe>().result.is_some()
    }

    #[test]
    fn water_assignment_requires_whole_source_and_destination_buckets() {
        for current in 0..=5 {
            for tank in [0, 1, 4, 5] {
                for full in [false, true] {
                    assert_eq!(
                        admitted(current, 5, tank, full, 0, 0),
                        current == 0 && (full || tank == 5),
                        "current={current} tank={tank} full={full}"
                    );
                }
            }
        }
    }

    #[test]
    fn water_assignment_counts_cache_and_cycle_shadow_as_buckets_once() {
        for (cached, shadow) in [(1, 0), (0, 1), (1, 1)] {
            assert!(!admitted(0, 5, 5, false, cached, shadow));
        }
        assert!(!admitted(0, 10, 5, false, 1, 1));
        assert!(admitted(0, 15, 5, false, 1, 1));
    }
}
