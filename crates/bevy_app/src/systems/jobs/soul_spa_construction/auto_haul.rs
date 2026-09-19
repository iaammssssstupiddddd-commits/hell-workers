use bevy::prelude::*;
use hw_core::area::TaskArea;
use hw_core::constants::FLOOR_CONSTRUCTION_PRIORITY;
use hw_core::familiar::{ActiveCommand, FamiliarCommand};
use hw_energy::{SoulSpaPhase, SoulSpaSite};
use hw_jobs::TargetSoulSpaSite;
use hw_logistics::ResourceType;
use hw_logistics::transport_request::TransportRequestKind;
use hw_logistics::transport_request::producer::{
    ExistingConstructionRequestQuery, RequestSyncSpec, collect_all_area_owners, find_owner,
    sync_construction_requests,
};
use hw_world::zones::{AreaBounds, Yard};
use std::collections::HashMap;

/// Constructing フェーズの SoulSpaSite に Bone の TransportRequest を自動生成。
pub fn soul_spa_auto_haul_system(
    mut commands: Commands,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea)>,
    q_yards: Query<(Entity, &Yard)>,
    q_sites: Query<(Entity, &Transform, &SoulSpaSite)>,
    q_existing: ExistingConstructionRequestQuery<TargetSoulSpaSite>,
) {
    let active_familiars: Vec<(Entity, AreaBounds)> = q_familiars
        .iter()
        .filter(|(_, active_cmd, _)| !matches!(active_cmd.command, FamiliarCommand::Idle))
        .map(|(e, _, area)| (e, area.bounds()))
        .collect();

    let active_yards: Vec<(Entity, Yard)> = q_yards.iter().map(|(e, y)| (e, y.clone())).collect();
    let all_owners = collect_all_area_owners(&active_familiars, &active_yards);

    if all_owners.is_empty() {
        return;
    }

    let mut desired_requests: HashMap<(Entity, ResourceType), (Entity, u32, Vec2)> = HashMap::new();

    for (site_entity, transform, site) in q_sites.iter() {
        if site.phase != SoulSpaPhase::Constructing {
            continue;
        }

        let site_pos = transform.translation.truncate();
        let Some((fam_entity, _)) = find_owner(site_pos, &all_owners) else {
            continue;
        };

        // Count in-flight deliveries for this site
        let in_flight: u32 = q_existing
            .iter()
            .filter(|(_, _, req, workers, _)| {
                req.anchor == site_entity
                    && req.kind == TransportRequestKind::DeliverToSoulSpa
                    && workers.map(|w| w.len()).unwrap_or(0) > 0
            })
            .map(|(_, _, _, workers, _)| workers.map(|w| w.len() as u32).unwrap_or(0))
            .sum();

        let remaining = site
            .bones_required
            .saturating_sub(site.bones_delivered)
            .saturating_sub(in_flight);

        if remaining == 0 {
            continue;
        }

        desired_requests.insert(
            (site_entity, ResourceType::Bone),
            (fam_entity, remaining.min(4), site_pos),
        );
    }

    sync_construction_requests(
        &mut commands,
        &q_existing,
        &desired_requests,
        RequestSyncSpec {
            expected_kind: TransportRequestKind::DeliverToSoulSpa,
            request_name: "TransportReq(SoulSpa Bone)",
            request_kind: TransportRequestKind::DeliverToSoulSpa,
        },
        |target: &TargetSoulSpaSite| target.0,
        TargetSoulSpaSite,
        |_| FLOOR_CONSTRUCTION_PRIORITY,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use hw_core::events::SoulTaskUnassignRequest;
    use hw_core::relationships::{TaskWorkers, WorkingOn};
    use hw_jobs::{Designation, Priority, TaskSlots};
    use hw_logistics::transport_request::{
        TransportDemand, TransportRequest, TransportRequestState,
        transport_request_anchor_cleanup_system,
    };

    fn fixture() -> (App, Entity, Entity, Entity) {
        let mut app = App::new();
        app.add_message::<SoulTaskUnassignRequest>()
            .add_systems(Update, soul_spa_auto_haul_system);
        let yard = app
            .world_mut()
            .spawn(Yard {
                min: Vec2::splat(-100.0),
                max: Vec2::splat(100.0),
            })
            .id();
        let site = app
            .world_mut()
            .spawn((
                Transform::default(),
                SoulSpaSite {
                    bones_required: 8,
                    bones_delivered: 0,
                    ..default()
                },
            ))
            .id();
        app.update();
        let request = app
            .world_mut()
            .query_filtered::<Entity, With<TargetSoulSpaSite>>()
            .single(app.world())
            .unwrap();
        (app, yard, site, request)
    }

    #[test]
    fn soul_spa_duplicate_workers_are_preserved_and_inflight_is_subtracted_once_test() {
        let (mut app, _, site, request) = fixture();
        assert_eq!(app.world().get::<TaskSlots>(request).unwrap().max, 4);
        let duplicate_request = app
            .world()
            .get::<TransportRequest>(request)
            .unwrap()
            .clone();
        let duplicate = app
            .world_mut()
            .spawn((
                duplicate_request.clone(),
                TargetSoulSpaSite(site),
                TaskSlots::new(8),
            ))
            .id();
        let empty_duplicate = app
            .world_mut()
            .spawn((duplicate_request, TargetSoulSpaSite(site)))
            .id();
        let workers = [
            app.world_mut().spawn(WorkingOn(request)).id(),
            app.world_mut().spawn(WorkingOn(request)).id(),
            app.world_mut().spawn(WorkingOn(duplicate)).id(),
        ];
        app.world_mut()
            .get_mut::<SoulSpaSite>(site)
            .unwrap()
            .bones_delivered = 2;
        app.update();
        // 8 required - 2 delivered - 3 workers = 3 total slots, not 3 + 2.
        assert_eq!(app.world().get::<TaskSlots>(request).unwrap().max, 3);
        let demand = app.world().get::<TransportDemand>(request).unwrap();
        assert_eq!((demand.desired_slots, demand.inflight), (3, 2));
        assert_eq!(app.world().get::<TaskSlots>(duplicate).unwrap().max, 1);
        assert_eq!(
            app.world().get::<TransportRequestState>(duplicate),
            Some(&TransportRequestState::Claimed)
        );
        assert!(app.world().get_entity(empty_duplicate).is_err());
        for (worker, target) in workers.into_iter().zip([request, request, duplicate]) {
            assert_eq!(app.world().get::<WorkingOn>(worker).unwrap().0, target);
        }
        assert_eq!(
            app.world().get::<TargetSoulSpaSite>(request).unwrap().0,
            site
        );
        assert_eq!(
            app.world().get::<Priority>(request).unwrap().0,
            FLOOR_CONSTRUCTION_PRIORITY
        );
        app.world_mut().clear_trackers();
        app.update();
        let entity = app.world().entity(request);
        assert!(!entity.get_ref::<TransportDemand>().unwrap().is_changed());
        assert!(!entity.get_ref::<TaskSlots>().unwrap().is_changed());
    }

    #[test]
    fn soul_spa_zero_demand_maintain_and_reactivation_test() {
        for has_worker in [false, true] {
            let (mut app, _, site, request) = fixture();
            let worker = has_worker.then(|| app.world_mut().spawn(WorkingOn(request)).id());
            app.world_mut()
                .get_mut::<SoulSpaSite>(site)
                .unwrap()
                .bones_delivered = 8;
            app.update();
            assert!(app.world().get::<Designation>(request).is_none());
            assert!(app.world().get::<TaskSlots>(request).is_none());
            let demand = app.world().get::<TransportDemand>(request).unwrap();
            assert_eq!(
                (demand.desired_slots, demand.inflight),
                (0, u32::from(has_worker))
            );
            app.world_mut()
                .run_system_once(transport_request_anchor_cleanup_system)
                .unwrap();
            assert_eq!(app.world().get_entity(request).is_ok(), has_worker);
            if let Some(worker) = worker {
                assert_eq!(app.world().get::<WorkingOn>(worker).unwrap().0, request);
                assert_eq!(app.world().get::<TaskWorkers>(request).unwrap().len(), 1);
            }
            app.world_mut()
                .get_mut::<SoulSpaSite>(site)
                .unwrap()
                .bones_delivered = 0;
            app.update();
            let renewed = app
                .world_mut()
                .query_filtered::<Entity, With<TargetSoulSpaSite>>()
                .single(app.world())
                .unwrap();
            assert_eq!(renewed == request, has_worker);
            assert_eq!(app.world().get::<TaskSlots>(renewed).unwrap().max, 4);
            assert!(app.world().get::<Designation>(renewed).is_some());
        }
    }

    #[test]
    fn soul_spa_no_owner_early_return_is_distinct_from_zero_demand_test() {
        let (mut app, yard, site, request) = fixture();
        app.world_mut().entity_mut(yard).remove::<Yard>();
        app.world_mut()
            .get_mut::<SoulSpaSite>(site)
            .unwrap()
            .bones_delivered = 8;
        app.world_mut().clear_trackers();
        app.update();
        let entity = app.world().entity(request);
        assert_eq!(entity.get::<TaskSlots>().unwrap().max, 4);
        assert!(!entity.get_ref::<TransportDemand>().unwrap().is_changed());
        // The lifecycle owner subsequently closes the invalid issuer.
        app.world_mut()
            .run_system_once(transport_request_anchor_cleanup_system)
            .unwrap();
        assert!(app.world().get_entity(request).is_err());
    }
}
