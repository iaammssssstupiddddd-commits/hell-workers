//! Bounded read-only diagnosis, not an acceptance verdict or task producer.
use bevy::prelude::*;
use hw_core::relationships::{ManagedBy, TaskWorkers};
use hw_jobs::{ActiveTaskIdentity, AssignedTask, Designation, Rock};
use hw_logistics::transport_request::{TransportDemand, TransportRequest, TransportRequestKind};
use hw_world::{WalkabilityConnectivityCache, WorldMap, Yard};
use serde_json::{Value, json};

fn id(entity: Entity) -> String {
    format!("{entity:?}")
}

fn workers(world: &World, entity: Entity) -> Vec<String> {
    let rows: Vec<_> = world
        .get::<TaskWorkers>(entity)
        .into_iter()
        .flat_map(|w| w.iter())
        .take(129)
        .map(|&entity| id(entity))
        .collect();
    assert!(rows.len() <= 128, "supply diagnostic worker cap");
    rows
}

pub(super) fn snapshot(world: &mut World) -> Value {
    use hw_familiar_ai::familiar_ai::decide::auto_gather_for_blueprint::AutoGatherDesignation;
    let requests: Vec<_> = world
        .query::<(Entity, &TransportRequest)>()
        .iter(world)
        .filter(|(_, r)| r.kind == TransportRequestKind::DeliverToMixerSolid)
        .take(129)
        .map(|(entity, request)| (entity, request.clone()))
        .collect();
    assert!(requests.len() <= 128, "supply diagnostic request cap");
    let rocks: Vec<_> = world
        .query_filtered::<(Entity, &Transform), With<Rock>>()
        .iter(world)
        .take(129)
        .map(|(entity, pose)| (entity, pose.translation.truncate()))
        .collect();
    assert!(rocks.len() <= 128, "supply diagnostic source cap");
    let mut cache = WalkabilityConnectivityCache::default();
    let mut request_rows = Vec::new();
    for (entity, request) in requests {
        let yard = world.get::<Yard>(request.issued_by);
        let area = world.get::<hw_core::area::TaskArea>(request.issued_by);
        let active = world.get::<hw_core::familiar::ActiveCommand>(request.issued_by);
        let start_pos = yard.map(|y| (y.min + y.max) * 0.5).or_else(|| {
            world
                .get::<Transform>(request.issued_by)
                .map(|p| p.translation.truncate())
        });
        let map = world.resource::<WorldMap>();
        let start = start_pos.and_then(|p| map.get_nearest_walkable_grid(p));
        let sources: Vec<_> = rocks.iter().map(|&(source, pos)| {
            let grid = WorldMap::world_to_grid(pos);
            json!({"entity":id(source),"grid":grid,
                "reachable": start.map(|s| cache.can_reach_target(map,s,grid,map.is_walkable(grid.0,grid.1))),
                "designation":world.get::<Designation>(source).map(|d|format!("{:?}",d.work_type)),
                "workers":workers(world,source),
                "managed_by":world.get::<ManagedBy>(source).map(|m|id(m.0)),
                "auto_owner":world.get::<AutoGatherDesignation>(source).map(|a|id(a.owner))})
        }).collect();
        request_rows.push(json!({"entity":id(entity),"mixer":id(request.anchor),
            "resource":format!("{:?}",request.resource_type),"issued_by":id(request.issued_by),
            "demand":world.get::<TransportDemand>(entity).map(|d|json!({"desired":d.desired_slots,"inflight":d.inflight})),
            "workers":workers(world,entity),"yard":yard.map(|y|json!([y.min.to_array(),y.max.to_array()])),
            "area":area.map(|a|json!([a.bounds.min.to_array(),a.bounds.max.to_array()])),
            "command":active.map(|a|format!("{:?}",a.command)),"start":start,"rock_sources":sources}));
    }
    let actors: Vec<_> = world.query::<(Entity,&AssignedTask)>().iter(world)
        .filter(|(_, task)|matches!(task,AssignedTask::Gather(_) | AssignedTask::HaulToMixer(_) | AssignedTask::HaulWithWheelbarrow(_)))
        .take(129).map(|(entity,task)| {
            if let AssignedTask::HaulWithWheelbarrow(data) = task {
                assert!(data.items.len() <= 128, "supply diagnostic cargo cap");
            }
            json!({"entity":id(entity),"task":format!("{task:?}"),
                "identity":world.get::<ActiveTaskIdentity>(entity).map(|i|json!({"assignment":id(i.assignment_entity),"target":id(i.current_target_entity)})),
                "position":world.get::<Transform>(entity).map(|p|p.translation.to_array()),
                "destination":world.get::<hw_core::soul::Destination>(entity).map(|d|d.0.to_array()),
                "path":world.get::<hw_core::soul::Path>(entity).map(|p|json!({"index":p.current_index,"len":p.waypoints.len(),"planned":p.planned_destination.map(|v|v.to_array())}))})
        }).collect();
    assert!(actors.len() <= 128, "supply diagnostic actor cap");
    json!({"requests":request_rows,"actors":actors,"accepted":false})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_world_is_not_evidence_and_does_not_mutate_map() {
        let mut world = World::new();
        world.init_resource::<WorldMap>();
        let version = world.resource::<WorldMap>().obstacle_version;
        let count = world.entities().len();
        assert_eq!(
            snapshot(&mut world),
            json!({"requests":[],"actors":[],"accepted":false})
        );
        assert_eq!(world.entities().len(), count);
        assert_eq!(world.resource::<WorldMap>().obstacle_version, version);
    }

    #[test]
    fn actual_request_and_designation_are_reported_without_repair() {
        use hw_core::logistics::ResourceType;
        let mut world = World::new();
        world.init_resource::<WorldMap>();
        let owner = world
            .spawn(Yard {
                min: Vec2::ZERO,
                max: Vec2::splat(64.0),
            })
            .id();
        let mixer = world.spawn_empty().id();
        let request = world
            .spawn((
                TransportRequest {
                    kind: TransportRequestKind::DeliverToMixerSolid,
                    anchor: mixer,
                    resource_type: ResourceType::Rock,
                    issued_by: owner,
                    priority: Default::default(),
                    stockpile_group: vec![],
                },
                TransportDemand {
                    desired_slots: 5,
                    inflight: 0,
                },
            ))
            .id();
        let rock = world
            .spawn((
                Rock,
                Transform::default(),
                Designation {
                    work_type: hw_jobs::WorkType::Mine,
                },
                ManagedBy(owner),
            ))
            .id();
        let count = world.entities().len();
        let result = snapshot(&mut world);
        let row = &result["requests"][0];
        assert_eq!(row["entity"], id(request));
        assert_eq!(row["issued_by"], id(owner));
        assert_eq!(row["demand"], json!({"desired":5,"inflight":0}));
        assert_eq!(row["rock_sources"][0]["entity"], id(rock));
        assert_eq!(row["rock_sources"][0]["designation"], "Mine");
        assert_eq!(row["rock_sources"][0]["workers"], json!([]));
        assert_eq!(row["rock_sources"][0]["auto_owner"], Value::Null);
        assert!(world.get::<TaskWorkers>(rock).is_none());
        assert_eq!(world.entities().len(), count);
        world.despawn(owner);
        let missing = snapshot(&mut world);
        assert_eq!(missing["requests"][0]["yard"], Value::Null);
        assert_eq!(missing["requests"][0]["start"], Value::Null);
        assert_eq!(
            missing["requests"][0]["rock_sources"][0]["reachable"],
            Value::Null
        );
        assert_eq!(missing["accepted"], false);
    }

    #[test]
    #[should_panic(expected = "source cap")]
    fn overflow_is_not_silently_truncated() {
        let mut world = World::new();
        world.init_resource::<WorldMap>();
        for _ in 0..129 {
            world.spawn((Rock, Transform::default()));
        }
        snapshot(&mut world);
    }
}
