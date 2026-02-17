use crate::entities::familiar::FamiliarCommand;
use crate::systems::familiar_ai::FamiliarDelegationPerfMetrics;
use crate::systems::familiar_ai::FamiliarTaskDelegationTimer;
use crate::systems::familiar_ai::decide::familiar_processor::{
    FamiliarDelegationContext, process_task_delegation_and_movement,
};
use crate::systems::familiar_ai::helpers::query_types::{FamiliarSoulQuery, FamiliarTaskQuery};
use crate::systems::spatial::{DesignationSpatialGrid, TransportRequestSpatialGrid};
use crate::world::map::WorldMap;
use crate::world::pathfinding::PathfindingContext;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use std::time::Instant;

/// 使い魔AIのタスク委譲に必要なSystemParam
#[derive(SystemParam)]
pub struct FamiliarAiTaskDelegationParams<'w, 's> {
    pub time: Res<'w, Time>,
    pub delegation_timer: ResMut<'w, FamiliarTaskDelegationTimer>,
    pub q_familiars: FamiliarTaskQuery<'w, 's>,
    pub q_souls: FamiliarSoulQuery<'w, 's>,
    pub task_queries:
        crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries<'w, 's>,
    pub designation_grid: Res<'w, DesignationSpatialGrid>,
    pub transport_request_grid: Res<'w, TransportRequestSpatialGrid>,
    pub world_map: Res<'w, WorldMap>,
    pub pf_context: Local<'s, PathfindingContext>,
    pub perf_metrics: ResMut<'w, FamiliarDelegationPerfMetrics>,
}

/// 使い魔AIのタスク委譲・移動システム（Decide Phase）
pub fn familiar_task_delegation_system(params: FamiliarAiTaskDelegationParams) {
    let started_at = Instant::now();
    let FamiliarAiTaskDelegationParams {
        time,
        mut delegation_timer,
        mut q_familiars,
        mut q_souls,
        mut task_queries,
        designation_grid,
        transport_request_grid,
        world_map,
        mut pf_context,
        mut perf_metrics,
        ..
    } = params;

    let timer_finished = delegation_timer.timer.tick(time.delta()).just_finished();
    let allow_task_delegation = !delegation_timer.first_run_done || timer_finished;
    delegation_timer.first_run_done = true;

    let mut reservation_shadow =
        crate::systems::familiar_ai::decide::task_management::ReservationShadow::default();
    let mut familiars_processed = 0u32;

    for (
        fam_entity,
        fam_transform,
        familiar_op,
        active_command,
        mut ai_state,
        mut fam_dest,
        mut fam_path,
        task_area_opt,
        commanding,
        managed_tasks_opt,
    ) in q_familiars.iter_mut()
    {
        if matches!(active_command.command, FamiliarCommand::Idle) {
            continue;
        }
        familiars_processed += 1;

        let state_changed = ai_state.is_changed();
        let default_tasks = crate::relationships::ManagedTasks::default();
        let managed_tasks = managed_tasks_opt.unwrap_or(&default_tasks);

        let initial_squad =
            crate::systems::familiar_ai::decide::squad::SquadManager::build_squad(commanding);
        let (squad_entities, _invalid_members) =
            crate::systems::familiar_ai::decide::squad::SquadManager::validate_squad(
                initial_squad,
                fam_entity,
                &mut q_souls,
            );

        let mut delegation_ctx = FamiliarDelegationContext {
            fam_entity,
            fam_transform,
            familiar_op,
            ai_state: &mut *ai_state,
            fam_dest: &mut *fam_dest,
            fam_path: &mut *fam_path,
            task_area_opt,
            squad_entities: &squad_entities,
            q_souls: &mut q_souls,
            task_queries: &mut task_queries,
            designation_grid: &designation_grid,
            transport_request_grid: &transport_request_grid,
            managed_tasks,
            world_map: &world_map,
            pf_context: &mut *pf_context,
            delta_secs: time.delta_secs(),
            allow_task_delegation,
            state_changed,
            reservation_shadow: &mut reservation_shadow,
        };
        process_task_delegation_and_movement(&mut delegation_ctx);
    }

    let (
        source_selector_calls,
        source_selector_cache_build_scanned_items,
        source_selector_candidate_scanned_items,
    ) = crate::systems::familiar_ai::decide::task_management::take_source_selector_scan_snapshot();
    let source_selector_scanned_items = source_selector_cache_build_scanned_items
        .saturating_add(source_selector_candidate_scanned_items);

    perf_metrics.latest_elapsed_ms = started_at.elapsed().as_secs_f32() * 1000.0;
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
    perf_metrics.familiars_processed = perf_metrics
        .familiars_processed
        .saturating_add(familiars_processed);
    perf_metrics.log_interval_secs += time.delta_secs();

    // 実測ログ出力は廃止。集計カウンタのみ定期リセットする。
    if perf_metrics.log_interval_secs >= 5.0 {
        perf_metrics.log_interval_secs = 0.0;
        perf_metrics.source_selector_calls = 0;
        perf_metrics.source_selector_cache_build_scanned_items = 0;
        perf_metrics.source_selector_candidate_scanned_items = 0;
        perf_metrics.source_selector_scanned_items = 0;
        perf_metrics.familiars_processed = 0;
    }
}
