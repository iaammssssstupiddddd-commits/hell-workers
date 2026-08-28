//! Familiar AI タスク委譲システム（Decide Phase）。
//!
//! WorldMap / WalkabilityConnectivityCache / ConstructionSiteAccess / SpatialGrid など
//! 全ての依存型は leaf crate 由来であり、hw_familiar_ai から直接参照できる。

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::familiar::Familiar;
use hw_core::relationships::CommandedBy;
use hw_core::soul::{DamnedSoul, IdleState};
use hw_jobs::{AssignedTask, ConstructionSiteAccess, TaskDiagnosticInputRevisions};
use hw_logistics::tile_index::TileSiteIndex;
use hw_spatial::{DesignationSpatialGrid, ResourceSpatialGrid, TransportRequestSpatialGrid};
use hw_world::{WalkabilityConnectivityCache, WorldMapRead};
use std::collections::HashSet;
#[cfg(feature = "profiling")]
use std::time::Instant;

use crate::familiar_ai::decide::delegation_context::{
    FamiliarDelegationContext, FamiliarMovementContext, process_supervision_movement,
    process_task_delegation,
};
use crate::familiar_ai::decide::query_types::{FamiliarSoulQuery, FamiliarTaskQuery};
#[cfg(feature = "profiling")]
use crate::familiar_ai::decide::resources::FamiliarDelegationPerfMetrics;
use crate::familiar_ai::decide::resources::FamiliarTaskDelegationTimer;
use crate::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries;
use crate::familiar_ai::decide::task_management::task_finder::DelegationCandidateSnapshot;
use crate::familiar_ai::decide::task_management::{
    FamiliarEvaluatorDiagnostics, FamiliarTaskCandidateDiagnostics, FamiliarTaskDiagnosticCycle,
};

/// 使い魔AIのタスク委譲に必要なSystemParam
type FamiliarDelegationQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static hw_core::familiar::FamiliarOperation,
        &'static hw_core::familiar::FamiliarPolicy,
        Option<&'static hw_core::area::TaskArea>,
        Option<&'static hw_core::relationships::Commanding>,
        Option<&'static hw_core::relationships::ManagedTasks>,
    ),
    With<Familiar>,
>;

/// Inputs scanned once and shared by every Familiar evaluated in this cycle.
struct DelegationCycleSnapshot {
    active_move_targets: HashSet<Entity>,
    incoming: crate::familiar_ai::decide::task_management::IncomingDeliverySnapshot,
    candidates: DelegationCandidateSnapshot,
}

#[derive(SystemParam)]
pub struct FamiliarAiTaskDelegationParams<'w, 's> {
    pub time: Res<'w, Time>,
    pub delegation_timer: ResMut<'w, FamiliarTaskDelegationTimer>,
    pub q_familiars: FamiliarDelegationQuery<'w, 's>,
    pub q_souls: FamiliarSoulQuery<'w, 's>,
    pub task_queries: FamiliarTaskAssignmentQueries<'w, 's>,
    pub construction_sites: ConstructionSiteAccess<'w, 's>,
    pub designation_grid: Res<'w, DesignationSpatialGrid>,
    pub transport_request_grid: Res<'w, TransportRequestSpatialGrid>,
    pub resource_grid: Res<'w, ResourceSpatialGrid>,
    pub tile_site_index: Res<'w, TileSiteIndex>,
    pub world_map: WorldMapRead<'w>,
    pub connectivity_cache: ResMut<'w, WalkabilityConnectivityCache>,
    pub diagnostic_revisions: Res<'w, TaskDiagnosticInputRevisions>,
    pub published_diagnostics: ResMut<'w, FamiliarTaskCandidateDiagnostics>,
    #[cfg(feature = "profiling")]
    pub perf_metrics: Option<ResMut<'w, FamiliarDelegationPerfMetrics>>,
}

/// 0.5-second Familiar task-delegation cycle (Decide Phase).
pub fn familiar_task_delegation_cycle_system(params: FamiliarAiTaskDelegationParams) {
    #[cfg(feature = "profiling")]
    let started_at = Instant::now();
    let FamiliarAiTaskDelegationParams {
        time,
        mut delegation_timer,
        q_familiars,
        mut q_souls,
        mut task_queries,
        construction_sites,
        designation_grid,
        transport_request_grid,
        resource_grid,
        tile_site_index,
        world_map,
        mut connectivity_cache,
        diagnostic_revisions,
        mut published_diagnostics,
        #[cfg(feature = "profiling")]
        perf_metrics,
        ..
    } = params;

    let allow_task_delegation = delegation_timer.advance(time.delta());
    if !allow_task_delegation {
        return;
    }
    let active_move_targets = {
        let mut assignments = q_souls.transmute_lens_filtered::<&AssignedTask, Without<Familiar>>();
        assignments
            .query()
            .iter()
            .filter_map(|task| match task {
                AssignedTask::MovePlant(data) => Some(data.building),
                _ => None,
            })
            .collect::<HashSet<_>>()
    };
    let cycle_snapshot = DelegationCycleSnapshot {
        active_move_targets,
        incoming: crate::familiar_ai::decide::task_management::IncomingDeliverySnapshot::build(
            &task_queries,
        ),
        candidates: DelegationCandidateSnapshot::build(&task_queries),
    };
    let mut diagnostic_cycle = Some(FamiliarTaskDiagnosticCycle::new(
        published_diagnostics.next_cycle(),
        &diagnostic_revisions,
    ));

    let mut reservation_shadow =
        crate::familiar_ai::decide::task_management::ReservationShadow::default();
    #[cfg(feature = "profiling")]
    let mut familiars_processed = 0u32;

    for (
        fam_entity,
        fam_transform,
        familiar_op,
        familiar_policy,
        task_area_opt,
        commanding,
        managed_tasks_opt,
    ) in q_familiars.iter()
    {
        let mut evaluator_diagnostics = FamiliarEvaluatorDiagnostics::new(0);
        if let Some(cycle) = diagnostic_cycle.as_mut() {
            cycle.begin_evaluator();
        }
        #[cfg(feature = "profiling")]
        {
            if allow_task_delegation {
                familiars_processed += 1;
            }
        }

        let default_tasks = hw_core::relationships::ManagedTasks::default();
        let managed_tasks = managed_tasks_opt.unwrap_or(&default_tasks);

        let squad_entities = {
            let mut q_squad_lens = q_souls.transmute_lens_filtered::<
                (Entity, &DamnedSoul, &IdleState, Option<&CommandedBy>),
                Without<Familiar>,
            >();
            let q_squad = q_squad_lens.query();
            let initial_squad =
                crate::familiar_ai::decide::squad::SquadManager::build_squad(commanding);
            crate::familiar_ai::decide::squad::SquadManager::validate_squad(
                initial_squad,
                fam_entity,
                &q_squad,
            )
            .0
        };

        let mut delegation_ctx = FamiliarDelegationContext {
            fam_entity,
            fam_transform,
            familiar_op,
            familiar_policy,
            task_area_opt,
            squad_entities: &squad_entities,
            active_move_targets: &cycle_snapshot.active_move_targets,
            candidate_snapshot: &cycle_snapshot.candidates,
            q_souls: &mut q_souls,
            task_queries: &mut task_queries,
            construction_sites: &construction_sites,
            designation_grid: &designation_grid,
            transport_request_grid: &transport_request_grid,
            resource_grid: &resource_grid,
            managed_tasks,
            world_map: &world_map,
            connectivity_cache: &mut connectivity_cache,
            // Yard 共有タスクは候補集合に残す。Idle command を周期 gate の
            // 例外にはせず、最大 0.5 秒で同じ候補探索へ入る。
            allow_task_delegation,
            reservation_shadow: &mut reservation_shadow,
            tile_site_index: &tile_site_index,
            incoming_snapshot: &cycle_snapshot.incoming,
            diagnostics: &mut evaluator_diagnostics,
            diagnostic_revisions: &diagnostic_revisions,
        };
        process_task_delegation(&mut delegation_ctx);
        if let Some(cycle) = diagnostic_cycle.as_mut() {
            cycle.finish_evaluator(evaluator_diagnostics);
        }
    }

    if let Some(cycle) = diagnostic_cycle {
        published_diagnostics.publish(cycle);
    }

    #[cfg(feature = "profiling")]
    if let Some(mut perf_metrics) = perf_metrics {
        let (
            source_selector_calls,
            source_selector_cache_build_scanned_items,
            source_selector_candidate_scanned_items,
        ) = crate::familiar_ai::decide::task_management::take_source_selector_scan_snapshot();
        let source_selector_scanned_items = source_selector_cache_build_scanned_items
            .saturating_add(source_selector_candidate_scanned_items);
        let reachable_with_cache_calls =
            crate::familiar_ai::decide::task_management::take_reachable_with_cache_calls();
        let candidate_metrics =
            crate::familiar_ai::decide::task_management::take_candidate_pipeline_perf_snapshot();

        perf_metrics.latest_elapsed_ms = started_at.elapsed().as_secs_f32() * 1000.0;
        if allow_task_delegation {
            perf_metrics.delegation_cycles = perf_metrics.delegation_cycles.saturating_add(1);
            perf_metrics.incoming_snapshot_builds =
                perf_metrics.incoming_snapshot_builds.saturating_add(1);
        }
        perf_metrics.source_selector_calls = perf_metrics
            .source_selector_calls
            .saturating_add(source_selector_calls);
        perf_metrics.source_selector_cache_build_scanned_items = perf_metrics
            .source_selector_cache_build_scanned_items
            .saturating_add(source_selector_cache_build_scanned_items);
        perf_metrics.source_selector_candidate_scanned_items = perf_metrics
            .source_selector_candidate_scanned_items
            .saturating_add(source_selector_candidate_scanned_items);
        perf_metrics.source_selector_scanned_items = perf_metrics
            .source_selector_scanned_items
            .saturating_add(source_selector_scanned_items);
        perf_metrics.reachable_with_cache_calls = perf_metrics
            .reachable_with_cache_calls
            .saturating_add(reachable_with_cache_calls);
        perf_metrics.familiars_processed = perf_metrics
            .familiars_processed
            .saturating_add(familiars_processed);
        perf_metrics.candidate_membership_checks = perf_metrics
            .candidate_membership_checks
            .saturating_add(candidate_metrics.membership_checks);
        perf_metrics.policy_disabled_rejections = perf_metrics
            .policy_disabled_rejections
            .saturating_add(candidate_metrics.policy_disabled_rejections);
        perf_metrics.candidate_snapshot_attempts = perf_metrics
            .candidate_snapshot_attempts
            .saturating_add(candidate_metrics.snapshot_attempts);
        perf_metrics.candidate_score_attempts = perf_metrics
            .candidate_score_attempts
            .saturating_add(candidate_metrics.score_attempts);
        perf_metrics.worker_score_attempts = perf_metrics
            .worker_score_attempts
            .saturating_add(candidate_metrics.worker_score_attempts);
        perf_metrics.top_k_partition_runs = perf_metrics
            .top_k_partition_runs
            .saturating_add(candidate_metrics.top_k_partition_runs);
        perf_metrics.top_k_retained_candidates = perf_metrics
            .top_k_retained_candidates
            .saturating_add(candidate_metrics.top_k_retained_candidates);
        perf_metrics.top_k_fallback_candidates = perf_metrics
            .top_k_fallback_candidates
            .saturating_add(candidate_metrics.top_k_fallback_candidates);
    }
}

/// Per-frame Familiar movement for Supervising and SearchingTask states.
pub fn familiar_supervision_movement_system(
    time: Res<Time>,
    mut q_familiars: FamiliarTaskQuery,
    mut q_souls: FamiliarSoulQuery,
) {
    for (
        fam_entity,
        fam_transform,
        _familiar_op,
        _familiar_policy,
        _active_command,
        mut ai_state,
        mut fam_dest,
        mut fam_path,
        task_area_opt,
        commanding,
        _managed_tasks_opt,
    ) in q_familiars.iter_mut()
    {
        let state_changed = ai_state.is_changed();
        let squad_entities = if matches!(
            *ai_state,
            hw_core::familiar::FamiliarAiState::Supervising { .. }
        ) {
            let mut q_squad_lens = q_souls.transmute_lens_filtered::<
                (Entity, &DamnedSoul, &IdleState, Option<&CommandedBy>),
                Without<Familiar>,
            >();
            let q_squad = q_squad_lens.query();
            let initial_squad =
                crate::familiar_ai::decide::squad::SquadManager::build_squad(commanding);
            crate::familiar_ai::decide::squad::SquadManager::validate_squad(
                initial_squad,
                fam_entity,
                &q_squad,
            )
            .0
        } else {
            Vec::new()
        };

        process_supervision_movement(&mut FamiliarMovementContext {
            fam_entity,
            fam_transform,
            ai_state: &mut ai_state,
            fam_dest: &mut fam_dest,
            fam_path: &mut fam_path,
            task_area_opt,
            squad_entities: &squad_entities,
            q_souls: &mut q_souls,
            delta_secs: time.delta_secs(),
            state_changed,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_core::events::ResourceReservationRequest;
    use hw_core::familiar::{
        ActiveCommand, FamiliarAiState, FamiliarCommand, FamiliarOperation, FamiliarPolicy,
    };
    use hw_core::relationships::{CommandedBy, ManagedBy};
    use hw_core::soul::{DamnedSoul, Destination, IdleState, Path};
    use hw_jobs::events::TaskAssignmentRequest;
    use hw_jobs::{AssignedTask, Designation, Priority, Rock, TaskSlots, WorkType};
    use hw_logistics::SharedResourceCache;
    use hw_logistics::tile_index::TileSiteIndex;
    use hw_logistics::transport_request::WheelbarrowArbitrationDiagnostics;
    use hw_spatial::{DesignationSpatialGrid, ResourceSpatialGrid, TransportRequestSpatialGrid};
    use hw_world::{WalkabilityConnectivityCache, WorldMap};

    #[test]
    fn one_delegation_cycle_submits_two_owned_mines_to_two_idle_souls() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<FamiliarTaskDelegationTimer>()
            .init_resource::<DesignationSpatialGrid>()
            .init_resource::<TransportRequestSpatialGrid>()
            .init_resource::<ResourceSpatialGrid>()
            .init_resource::<TileSiteIndex>()
            .init_resource::<WorldMap>()
            .init_resource::<WalkabilityConnectivityCache>()
            .init_resource::<SharedResourceCache>()
            .init_resource::<WheelbarrowArbitrationDiagnostics>()
            .init_resource::<TaskDiagnosticInputRevisions>()
            .init_resource::<FamiliarTaskCandidateDiagnostics>()
            .add_message::<ResourceReservationRequest>()
            .add_message::<TaskAssignmentRequest>()
            .add_systems(
                Update,
                (
                    familiar_task_delegation_cycle_system,
                    familiar_supervision_movement_system,
                )
                    .chain(),
            );

        let familiar = app
            .world_mut()
            .spawn((
                Familiar::default(),
                Transform::default(),
                FamiliarOperation {
                    max_controlled_soul: 2,
                    ..default()
                },
                FamiliarPolicy::default(),
                ActiveCommand {
                    command: FamiliarCommand::Patrol,
                },
                FamiliarAiState::SearchingTask,
                Destination(Vec2::ZERO),
                Path::default(),
            ))
            .id();
        let souls = [
            app.world_mut()
                .spawn((
                    Transform::from_xyz(-16.0, 0.0, 0.0),
                    DamnedSoul::default(),
                    AssignedTask::None,
                    Destination(Vec2::ZERO),
                    Path::default(),
                    IdleState::default(),
                    CommandedBy(familiar),
                ))
                .id(),
            app.world_mut()
                .spawn((
                    Transform::from_xyz(16.0, 0.0, 0.0),
                    DamnedSoul::default(),
                    AssignedTask::None,
                    Destination(Vec2::ZERO),
                    Path::default(),
                    IdleState::default(),
                    CommandedBy(familiar),
                ))
                .id(),
        ];
        let mine_positions = [Vec2::new(-32.0, 32.0), Vec2::new(32.0, 32.0)];
        {
            let mut world_map = app.world_mut().resource_mut::<WorldMap>();
            for pos in mine_positions {
                world_map.add_grid_obstacle(WorldMap::world_to_grid(pos));
            }
        }
        let mines = mine_positions.map(|pos| {
            app.world_mut()
                .spawn((
                    Transform::from_translation(pos.extend(0.0)),
                    Designation {
                        work_type: WorkType::Mine,
                    },
                    ManagedBy(familiar),
                    TaskSlots::new(1),
                    Priority::default(),
                    Rock,
                ))
                .id()
        });
        app.world_mut().flush();

        app.update();

        let requests = app
            .world()
            .resource::<Messages<TaskAssignmentRequest>>()
            .iter_current_update_messages()
            .collect::<Vec<_>>();
        assert_eq!(requests.len(), 2);
        assert!(
            requests
                .iter()
                .all(|request| request.work_type == WorkType::Mine)
        );
        assert!(souls.iter().all(|soul| {
            requests
                .iter()
                .any(|request| request.worker_entity == *soul)
        }));
        assert!(
            mines
                .iter()
                .all(|mine| { requests.iter().any(|request| request.task_entity == *mine) })
        );
    }
}
