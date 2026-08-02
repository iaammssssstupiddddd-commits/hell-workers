//! Familiar AI タスク委譲システム（Decide Phase）。
//!
//! WorldMap / WalkabilityConnectivityCache / ConstructionSiteAccess / SpatialGrid など
//! 全ての依存型は leaf crate 由来であり、hw_familiar_ai から直接参照できる。

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::familiar::Familiar;
use hw_core::relationships::CommandedBy;
use hw_core::soul::{DamnedSoul, IdleState};
use hw_jobs::ConstructionSiteAccess;
use hw_jobs::TaskDiagnosticInputRevisions;
use hw_logistics::tile_index::TileSiteIndex;
use hw_spatial::{DesignationSpatialGrid, ResourceSpatialGrid, TransportRequestSpatialGrid};
use hw_world::{WalkabilityConnectivityCache, WorldMapRead};
#[cfg(feature = "profiling")]
use std::time::Instant;

use crate::familiar_ai::decide::delegation_context::{
    FamiliarDelegationContext, process_task_delegation_and_movement,
};
use crate::familiar_ai::decide::query_types::{FamiliarSoulQuery, FamiliarTaskQuery};
#[cfg(feature = "profiling")]
use crate::familiar_ai::decide::resources::FamiliarDelegationPerfMetrics;
use crate::familiar_ai::decide::resources::FamiliarTaskDelegationTimer;
use crate::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries;
use crate::familiar_ai::decide::task_management::{
    FamiliarEvaluatorDiagnostics, FamiliarTaskCandidateDiagnostics, FamiliarTaskDiagnosticCycle,
};

/// 使い魔AIのタスク委譲に必要なSystemParam
#[derive(SystemParam)]
pub struct FamiliarAiTaskDelegationParams<'w, 's> {
    pub time: Res<'w, Time>,
    pub delegation_timer: ResMut<'w, FamiliarTaskDelegationTimer>,
    pub q_familiars: FamiliarTaskQuery<'w, 's>,
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
    pub perf_metrics: ResMut<'w, FamiliarDelegationPerfMetrics>,
}

/// 使い魔AIのタスク委譲・移動システム（Decide Phase）
pub fn familiar_task_delegation_system(params: FamiliarAiTaskDelegationParams) {
    #[cfg(feature = "profiling")]
    let started_at = Instant::now();
    let FamiliarAiTaskDelegationParams {
        time,
        mut delegation_timer,
        mut q_familiars,
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
        mut perf_metrics,
        ..
    } = params;

    let allow_task_delegation = delegation_timer.advance(time.delta());
    let mut diagnostic_cycle = allow_task_delegation.then(|| {
        FamiliarTaskDiagnosticCycle::new(published_diagnostics.next_cycle(), &diagnostic_revisions)
    });

    let incoming_snapshot = if allow_task_delegation {
        crate::familiar_ai::decide::task_management::IncomingDeliverySnapshot::build(&task_queries)
    } else {
        crate::familiar_ai::decide::task_management::IncomingDeliverySnapshot::default()
    };

    let mut reservation_shadow =
        crate::familiar_ai::decide::task_management::ReservationShadow::default();
    #[cfg(feature = "profiling")]
    let mut familiars_processed = 0u32;

    for (
        fam_entity,
        fam_transform,
        familiar_op,
        familiar_policy,
        _active_command,
        mut ai_state,
        mut fam_dest,
        mut fam_path,
        task_area_opt,
        commanding,
        managed_tasks_opt,
    ) in q_familiars.iter_mut()
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

        let state_changed = ai_state.is_changed();
        let default_tasks = hw_core::relationships::ManagedTasks::default();
        let managed_tasks = managed_tasks_opt.unwrap_or(&default_tasks);

        // Delegation needs a validated squad only on its 0.5 s cycle. The
        // continuous supervising path also needs it to follow active workers;
        // idle/searching/scouting frames avoid rebuilding the Vec entirely.
        let needs_squad = allow_task_delegation
            || matches!(
                *ai_state,
                hw_core::familiar::FamiliarAiState::Supervising { .. }
            );
        let squad_entities = if needs_squad {
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

        let mut delegation_ctx = FamiliarDelegationContext {
            fam_entity,
            fam_transform,
            familiar_op,
            familiar_policy,
            ai_state: &mut ai_state,
            fam_dest: &mut fam_dest,
            fam_path: &mut fam_path,
            task_area_opt,
            squad_entities: &squad_entities,
            q_souls: &mut q_souls,
            task_queries: &mut task_queries,
            construction_sites: &construction_sites,
            designation_grid: &designation_grid,
            transport_request_grid: &transport_request_grid,
            resource_grid: &resource_grid,
            managed_tasks,
            world_map: &world_map,
            connectivity_cache: &mut connectivity_cache,
            delta_secs: time.delta_secs(),
            // Yard 共有タスクは候補集合に残す。Idle command を周期 gate の
            // 例外にはせず、最大 0.5 秒で同じ候補探索へ入る。
            allow_task_delegation,
            state_changed,
            reservation_shadow: &mut reservation_shadow,
            tile_site_index: &tile_site_index,
            incoming_snapshot: &incoming_snapshot,
            diagnostics: &mut evaluator_diagnostics,
            diagnostic_revisions: &diagnostic_revisions,
        };
        process_task_delegation_and_movement(&mut delegation_ctx);
        if let Some(cycle) = diagnostic_cycle.as_mut() {
            cycle.finish_evaluator(evaluator_diagnostics);
        }
    }

    if let Some(cycle) = diagnostic_cycle {
        published_diagnostics.publish(cycle);
    }

    #[cfg(feature = "profiling")]
    {
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
            .add_systems(Update, familiar_task_delegation_system);

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
