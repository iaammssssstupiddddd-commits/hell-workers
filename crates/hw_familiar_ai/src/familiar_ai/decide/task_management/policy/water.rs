use bevy::prelude::*;
use hw_core::logistics::ResourceType;

use super::super::builders::{WaterHaulSpec, issue_gather_water, issue_haul_water_to_mixer};
use super::super::validator::{
    resolve_gather_water_inputs, resolve_haul_water_to_mixer_inputs, source_not_reserved,
};
use crate::familiar_ai::decide::task_management::{
    AssignTaskContext, CandidateRejectReason, FamiliarTaskAssignmentQueries, ReservationShadow,
    TaskAssignmentAttempt,
};

pub(super) fn assign_gather_water(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> TaskAssignmentAttempt {
    let Some((bucket_entity, tank_entity)) = resolve_gather_water_inputs(
        ctx.task_entity,
        task_pos,
        ctx.task_area_opt,
        queries,
        shadow,
    ) else {
        debug!(
            "ASSIGN: No suitable bucket/tank found for GatherWater task {:?}",
            ctx.task_entity
        );
        return TaskAssignmentAttempt::Rejected(CandidateRejectReason::MissingResourceOrSource);
    };

    if !source_not_reserved(bucket_entity, queries, shadow) {
        return TaskAssignmentAttempt::Rejected(CandidateRejectReason::TemporaryContention);
    }

    issue_gather_water(
        bucket_entity,
        tank_entity,
        task_pos,
        already_commanded,
        ctx,
        queries,
        shadow,
    );
    TaskAssignmentAttempt::Submitted
}

pub(super) fn assign_haul_water_to_mixer(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> TaskAssignmentAttempt {
    let Some((mixer_entity, tank_entity, bucket_entity)) = resolve_haul_water_to_mixer_inputs(
        ctx.task_entity,
        task_pos,
        ctx.task_area_opt,
        queries,
        shadow,
    ) else {
        debug!(
            "ASSIGN: HaulWaterToMixer task {:?} has no TargetMixer or no available tank/bucket",
            ctx.task_entity
        );
        return TaskAssignmentAttempt::Rejected(CandidateRejectReason::MissingResourceOrSource);
    };

    let bucket_is_full = queries
        .items
        .get(bucket_entity)
        .ok()
        .is_some_and(|(item, _)| item.0 == ResourceType::BucketWater)
        || queries
            .designation
            .targets
            .get(bucket_entity)
            .ok()
            .and_then(|(_, _, _, _, resource_item_opt, _, _)| resource_item_opt.map(|res| res.0))
            .is_some_and(|resource_type| resource_type == ResourceType::BucketWater);

    if !source_not_reserved(bucket_entity, queries, shadow) {
        return TaskAssignmentAttempt::Rejected(CandidateRejectReason::TemporaryContention);
    }
    let needs_tank_fill = !bucket_is_full;
    if needs_tank_fill && !source_not_reserved(tank_entity, queries, shadow) {
        return TaskAssignmentAttempt::Rejected(CandidateRejectReason::TemporaryContention);
    }

    issue_haul_water_to_mixer(
        WaterHaulSpec {
            bucket: bucket_entity,
            mixer: mixer_entity,
            tank: tank_entity,
            needs_tank_fill,
        },
        task_pos,
        already_commanded,
        ctx,
        queries,
        shadow,
    );
    TaskAssignmentAttempt::Submitted
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_core::events::ResourceReservationRequest;
    use hw_jobs::events::TaskAssignmentRequest;
    use hw_logistics::SharedResourceCache;
    use hw_logistics::transport_request::{
        TransportPriority, TransportRequest, TransportRequestKind,
        WheelbarrowArbitrationDiagnostics,
    };
    use hw_logistics::zone::Stockpile;
    use hw_spatial::ResourceSpatialGrid;
    use hw_world::WorldMap;

    use crate::familiar_ai::decide::task_management::{
        IncomingDeliverySnapshot, ReservationShadow,
    };

    #[derive(Resource)]
    struct MissingBucketFixture {
        request: Entity,
        familiar: Entity,
        worker: Entity,
    }

    #[derive(Resource, Default)]
    struct AssignmentProbe(Option<TaskAssignmentAttempt>);

    fn probe_missing_bucket_assignment(
        fixture: Res<MissingBucketFixture>,
        mut queries: FamiliarTaskAssignmentQueries,
        resource_grid: Res<ResourceSpatialGrid>,
        tile_site_index: Res<hw_logistics::tile_index::TileSiteIndex>,
        mut probe: ResMut<AssignmentProbe>,
    ) {
        let incoming = IncomingDeliverySnapshot::default();
        let mut shadow = ReservationShadow::default();
        probe.0 = Some(assign_gather_water(
            Vec2::ZERO,
            false,
            &AssignTaskContext {
                fam_entity: fixture.familiar,
                task_entity: fixture.request,
                worker_entity: fixture.worker,
                fatigue_threshold: 0.0,
                task_area_opt: None,
                resource_grid: &resource_grid,
                tile_site_index: &tile_site_index,
                incoming_snapshot: &incoming,
            },
            &mut queries,
            &mut shadow,
        ));
    }

    #[test]
    fn gather_water_without_a_bucket_is_missing_resource_not_contention() {
        let mut app = App::new();
        app.init_resource::<WorldMap>()
            .init_resource::<SharedResourceCache>()
            .init_resource::<WheelbarrowArbitrationDiagnostics>()
            .init_resource::<ResourceSpatialGrid>()
            .init_resource::<hw_logistics::tile_index::TileSiteIndex>()
            .init_resource::<AssignmentProbe>()
            .add_message::<ResourceReservationRequest>()
            .add_message::<TaskAssignmentRequest>()
            .add_systems(Update, probe_missing_bucket_assignment);

        let tank = app
            .world_mut()
            .spawn((
                Transform::default(),
                Stockpile {
                    capacity: 4,
                    resource_type: Some(ResourceType::Water),
                },
            ))
            .id();
        let familiar = app.world_mut().spawn_empty().id();
        let worker = app.world_mut().spawn_empty().id();
        let request = app
            .world_mut()
            .spawn(TransportRequest {
                kind: TransportRequestKind::GatherWaterToTank,
                anchor: tank,
                resource_type: ResourceType::BucketWater,
                issued_by: familiar,
                priority: TransportPriority::Normal,
                stockpile_group: Vec::new(),
            })
            .id();
        app.insert_resource(MissingBucketFixture {
            request,
            familiar,
            worker,
        });

        app.update();

        assert_eq!(
            app.world().resource::<AssignmentProbe>().0,
            Some(TaskAssignmentAttempt::Rejected(
                CandidateRejectReason::MissingResourceOrSource
            ))
        );
    }
}
