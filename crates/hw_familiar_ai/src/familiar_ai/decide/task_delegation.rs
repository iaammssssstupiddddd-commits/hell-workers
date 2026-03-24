//! Familiar AI タスク委譲システム（Decide Phase）。
//!
//! WorldMap / PathfindingContext / ConstructionSiteAccess / SpatialGrid など
//! 全ての依存型は leaf crate 由来であり、hw_familiar_ai から直接参照できる。

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::familiar::{Familiar, FamiliarCommand};
use hw_core::relationships::CommandedBy;
use hw_core::soul::{DamnedSoul, IdleState};
use hw_jobs::ConstructionSiteAccess;
use hw_logistics::tile_index::TileSiteIndex;
use hw_spatial::{DesignationSpatialGrid, ResourceSpatialGrid, TransportRequestSpatialGrid};
use hw_world::WorldMapRead;
use hw_world::pathfinding::PathfindingContext;
use std::time::Instant;

use crate::familiar_ai::decide::delegation_context::{
    FamiliarDelegationContext, process_task_delegation_and_movement,
};
use crate::familiar_ai::decide::query_types::{FamiliarSoulQuery, FamiliarTaskQuery};
use crate::familiar_ai::decide::resources::{
    FamiliarDelegationPerfMetrics, FamiliarTaskDelegationTimer, ReachabilityFrameCache,
};
use crate::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries;

const REACHABILITY_CACHE_SAFETY_CLEAR_INTERVAL_FRAMES: u32 = 60;

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
    pub pf_context: Local<'s, PathfindingContext>,
    pub reachability_frame_cache: ResMut<'w, ReachabilityFrameCache>,
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
        construction_sites,
        designation_grid,
        transport_request_grid,
        resource_grid,
        tile_site_index,
        world_map,
        mut pf_context,
        mut reachability_frame_cache,
        mut perf_metrics,
        ..
    } = params;

    if world_map.is_changed() {
        reachability_frame_cache.cache.clear();
        reachability_frame_cache.age = 0;
    } else {
        reachability_frame_cache.age = reachability_frame_cache.age.saturating_add(1);
        if reachability_frame_cache.age >= REACHABILITY_CACHE_SAFETY_CLEAR_INTERVAL_FRAMES {
            reachability_frame_cache.cache.clear();
            reachability_frame_cache.age = 0;
        }
    }

    let timer_finished = delegation_timer.timer.tick(time.delta()).just_finished();
    let allow_task_delegation = !delegation_timer.first_run_done || timer_finished;
    delegation_timer.first_run_done = true;

    let mut reservation_shadow =
        crate::familiar_ai::decide::task_management::ReservationShadow::default();
    let incoming_snapshot =
        crate::familiar_ai::decide::task_management::IncomingDeliverySnapshot::build(&task_queries);
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
        let is_idle_command = matches!(active_command.command, FamiliarCommand::Idle);
        familiars_processed += 1;

        let state_changed = ai_state.is_changed();
        let default_tasks = hw_core::relationships::ManagedTasks::default();
        let managed_tasks = managed_tasks_opt.unwrap_or(&default_tasks);

        let (squad_entities, _invalid_members) = {
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
        };

        let mut delegation_ctx = FamiliarDelegationContext {
            fam_entity,
            fam_transform,
            familiar_op,
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
            pf_context: &mut pf_context,
            delta_secs: time.delta_secs(),
            // Yard 共有タスクは TaskArea 非依存で拾える要件のため、
            // Idle command でも委譲処理自体は実行する。
            allow_task_delegation: allow_task_delegation || is_idle_command,
            state_changed,
            reservation_shadow: &mut reservation_shadow,
            reachability_frame_cache: &mut reachability_frame_cache.cache,
            tile_site_index: &tile_site_index,
            incoming_snapshot: &incoming_snapshot,
        };
        process_task_delegation_and_movement(&mut delegation_ctx);
    }

    let (
        source_selector_calls,
        source_selector_cache_build_scanned_items,
        source_selector_candidate_scanned_items,
    ) = crate::familiar_ai::decide::task_management::take_source_selector_scan_snapshot();
    let source_selector_scanned_items = source_selector_cache_build_scanned_items
        .saturating_add(source_selector_candidate_scanned_items);
    let reachable_with_cache_calls =
        crate::familiar_ai::decide::task_management::take_reachable_with_cache_calls();

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
    perf_metrics.reachable_with_cache_calls = perf_metrics
        .reachable_with_cache_calls
        .saturating_add(reachable_with_cache_calls);
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
        perf_metrics.reachable_with_cache_calls = 0;
        perf_metrics.familiars_processed = 0;
    }
}
