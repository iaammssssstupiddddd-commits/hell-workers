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
        #[cfg(feature = "profiling")]
        mut perf_metrics,
        ..
    } = params;

    let allow_task_delegation = delegation_timer.advance(time.delta());

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
        _active_command,
        mut ai_state,
        mut fam_dest,
        mut fam_path,
        task_area_opt,
        commanding,
        managed_tasks_opt,
    ) in q_familiars.iter_mut()
    {
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
        };
        process_task_delegation_and_movement(&mut delegation_ctx);
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
    }
}
