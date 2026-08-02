use bevy::prelude::*;
use hw_jobs::WorkType;

use super::super::builders::{
    issue_build, issue_collect_bone, issue_gather, issue_generate_power, issue_move, issue_refine,
};
use super::super::validator::can_reserve_source;
use crate::familiar_ai::decide::task_management::{
    AssignTaskContext, CandidateRejectReason, FamiliarTaskAssignmentQueries, ReservationShadow,
    TaskAssignmentAttempt,
};

pub(super) fn assign_gather(
    work_type: WorkType,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> TaskAssignmentAttempt {
    if !can_reserve_source(ctx.task_entity, queries, shadow) {
        return TaskAssignmentAttempt::Rejected(CandidateRejectReason::TemporaryContention);
    }
    issue_gather(work_type, task_pos, already_commanded, ctx, queries, shadow);
    TaskAssignmentAttempt::Submitted
}

pub(super) fn assign_build(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> TaskAssignmentAttempt {
    if let Ok((_, bp, _)) = queries.storage.blueprints.get(ctx.task_entity)
        && !bp.materials_complete()
    {
        debug!(
            "ASSIGN: Build target {:?} materials not complete",
            ctx.task_entity
        );
        return TaskAssignmentAttempt::Rejected(CandidateRejectReason::DependencyWaiting);
    }
    issue_build(task_pos, already_commanded, ctx, queries, shadow);
    TaskAssignmentAttempt::Submitted
}

pub(super) fn assign_move(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> TaskAssignmentAttempt {
    if issue_move(task_pos, already_commanded, ctx, queries, shadow) {
        TaskAssignmentAttempt::Submitted
    } else {
        TaskAssignmentAttempt::Rejected(CandidateRejectReason::MalformedTask)
    }
}

pub(super) fn assign_refine(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> TaskAssignmentAttempt {
    if !can_reserve_source(ctx.task_entity, queries, shadow) {
        return TaskAssignmentAttempt::Rejected(CandidateRejectReason::TemporaryContention);
    }
    issue_refine(task_pos, already_commanded, ctx, queries, shadow);
    TaskAssignmentAttempt::Submitted
}

pub(super) fn assign_collect_bone(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> TaskAssignmentAttempt {
    if !can_reserve_source(ctx.task_entity, queries, shadow) {
        return TaskAssignmentAttempt::Rejected(CandidateRejectReason::TemporaryContention);
    }
    issue_collect_bone(task_pos, already_commanded, ctx, queries, shadow);
    TaskAssignmentAttempt::Submitted
}

pub(super) fn assign_generate_power(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> TaskAssignmentAttempt {
    // タイルの parent_site を引き、active_slots ゲートを確認
    let Ok((tile, _)) = queries.soul_spa_tiles.get(ctx.task_entity) else {
        return TaskAssignmentAttempt::Rejected(CandidateRejectReason::MalformedTask);
    };
    let parent_site = tile.parent_site;
    let Ok(site) = queries.soul_spa_sites.get(parent_site) else {
        return TaskAssignmentAttempt::Rejected(CandidateRejectReason::MalformedTask);
    };
    if !can_reserve_source(ctx.task_entity, queries, shadow) {
        return TaskAssignmentAttempt::Rejected(CandidateRejectReason::TemporaryContention);
    }
    let occupied = queries
        .soul_spa_tiles
        .iter()
        .filter(|(t, w)| t.parent_site == parent_site && w.map(|w| !w.is_empty()).unwrap_or(false))
        .count() as u32;
    let occupied_or_pending =
        occupied.saturating_add(shadow.pending_soul_spa_assignments(parent_site));
    if !site.has_available_slot(occupied_or_pending) {
        return TaskAssignmentAttempt::Rejected(CandidateRejectReason::TemporaryContention);
    }
    issue_generate_power(task_pos, already_commanded, ctx, queries, shadow);
    shadow.reserve_soul_spa_assignment(parent_site);
    TaskAssignmentAttempt::Submitted
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_core::events::ResourceReservationRequest;
    use hw_energy::{SoulSpaPhase, SoulSpaSite, SoulSpaTile};
    use hw_jobs::events::TaskAssignmentRequest;
    use hw_logistics::SharedResourceCache;
    use hw_logistics::transport_request::WheelbarrowArbitrationDiagnostics;
    use hw_spatial::ResourceSpatialGrid;
    use hw_world::WorldMap;

    use crate::familiar_ai::decide::task_management::IncomingDeliverySnapshot;

    #[derive(Resource)]
    struct GeneratePowerFixture {
        attempts: Vec<Entity>,
        familiar: Entity,
        workers: Vec<Entity>,
    }

    #[derive(Resource, Default)]
    struct AssignmentProbe(Vec<TaskAssignmentAttempt>);

    fn probe_same_cycle_assignments(
        fixture: Res<GeneratePowerFixture>,
        mut queries: FamiliarTaskAssignmentQueries,
        resource_grid: Res<ResourceSpatialGrid>,
        tile_site_index: Res<hw_logistics::tile_index::TileSiteIndex>,
        mut probe: ResMut<AssignmentProbe>,
    ) {
        let incoming = IncomingDeliverySnapshot::default();
        let mut shadow = ReservationShadow::default();

        for (&task_entity, &worker_entity) in fixture.attempts.iter().zip(&fixture.workers) {
            probe.0.push(assign_generate_power(
                Vec2::ZERO,
                false,
                &AssignTaskContext {
                    fam_entity: fixture.familiar,
                    task_entity,
                    worker_entity,
                    fatigue_threshold: 1.0,
                    task_area_opt: None,
                    resource_grid: &resource_grid,
                    tile_site_index: &tile_site_index,
                    incoming_snapshot: &incoming,
                },
                &mut queries,
                &mut shadow,
            ));
        }
    }

    #[test]
    fn generate_power_counts_pending_assignments_in_the_same_cycle() {
        let mut app = App::new();
        app.init_resource::<WorldMap>()
            .init_resource::<SharedResourceCache>()
            .init_resource::<WheelbarrowArbitrationDiagnostics>()
            .init_resource::<ResourceSpatialGrid>()
            .init_resource::<hw_logistics::tile_index::TileSiteIndex>()
            .init_resource::<AssignmentProbe>()
            .add_message::<ResourceReservationRequest>()
            .add_message::<TaskAssignmentRequest>()
            .add_systems(Update, probe_same_cycle_assignments);

        let site = app
            .world_mut()
            .spawn(SoulSpaSite {
                phase: SoulSpaPhase::Operational,
                active_slots: 1,
                ..default()
            })
            .id();
        let tiles = [
            app.world_mut()
                .spawn(SoulSpaTile {
                    parent_site: site,
                    grid_pos: (0, 0),
                })
                .id(),
            app.world_mut()
                .spawn(SoulSpaTile {
                    parent_site: site,
                    grid_pos: (1, 0),
                })
                .id(),
        ];
        let familiar = app.world_mut().spawn_empty().id();
        let workers = [
            app.world_mut().spawn_empty().id(),
            app.world_mut().spawn_empty().id(),
        ];
        app.insert_resource(GeneratePowerFixture {
            attempts: tiles.to_vec(),
            familiar,
            workers: workers.to_vec(),
        });

        app.update();

        assert_eq!(
            app.world().resource::<AssignmentProbe>().0,
            vec![
                TaskAssignmentAttempt::Submitted,
                TaskAssignmentAttempt::Rejected(CandidateRejectReason::TemporaryContention),
            ]
        );
    }

    #[test]
    fn generate_power_rejects_a_same_cycle_tile_collision_without_consuming_another_site_slot() {
        let mut app = App::new();
        app.init_resource::<WorldMap>()
            .init_resource::<SharedResourceCache>()
            .init_resource::<WheelbarrowArbitrationDiagnostics>()
            .init_resource::<ResourceSpatialGrid>()
            .init_resource::<hw_logistics::tile_index::TileSiteIndex>()
            .init_resource::<AssignmentProbe>()
            .add_message::<ResourceReservationRequest>()
            .add_message::<TaskAssignmentRequest>()
            .add_systems(Update, probe_same_cycle_assignments);

        let site = app
            .world_mut()
            .spawn(SoulSpaSite {
                phase: SoulSpaPhase::Operational,
                active_slots: 2,
                ..default()
            })
            .id();
        let first_tile = app
            .world_mut()
            .spawn(SoulSpaTile {
                parent_site: site,
                grid_pos: (0, 0),
            })
            .id();
        let second_tile = app
            .world_mut()
            .spawn(SoulSpaTile {
                parent_site: site,
                grid_pos: (1, 0),
            })
            .id();
        let familiar = app.world_mut().spawn_empty().id();
        let workers = (0..3).map(|_| app.world_mut().spawn_empty().id()).collect();
        app.insert_resource(GeneratePowerFixture {
            attempts: vec![first_tile, first_tile, second_tile],
            familiar,
            workers,
        });

        app.update();

        assert_eq!(
            app.world().resource::<AssignmentProbe>().0,
            vec![
                TaskAssignmentAttempt::Submitted,
                TaskAssignmentAttempt::Rejected(CandidateRejectReason::TemporaryContention),
                TaskAssignmentAttempt::Submitted,
            ]
        );
    }
}
