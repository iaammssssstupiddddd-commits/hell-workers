//! TransportRequest のメトリクス集計

use std::collections::HashMap;

use bevy::prelude::*;

use crate::transport_request::{TransportRequest, TransportRequestKind, TransportRequestState};

/// TransportRequest の集計メトリクス
#[derive(Resource, Default, Debug)]
pub struct TransportRequestMetrics {
    pub by_kind: HashMap<TransportRequestKind, u32>,
    pub by_state: HashMap<TransportRequestState, u32>,
    pub total: u32,
    pub wheelbarrow_leases_active: u32,
    pub wheelbarrow_leases_granted_this_frame: u32,
    pub wheelbarrow_arb_eligible_requests: u32,
    pub wheelbarrow_arb_bucket_items_total: u32,
    pub wheelbarrow_arb_candidates_after_topk: u32,
    pub wheelbarrow_arb_items_deduped: u32,
    pub wheelbarrow_arb_candidates_dropped_by_dedup: u32,
    pub wheelbarrow_arb_avg_pending_secs: f32,
    pub wheelbarrow_arb_avg_lease_duration: f32,
    pub wheelbarrow_arb_elapsed_ms: f32,
    pub task_area_groups: u32,
    pub task_area_free_items_scanned: u32,
    pub task_area_items_matched: u32,
    pub task_area_elapsed_ms: f32,
    pub floor_material_sync_sites_processed: u32,
    pub floor_material_sync_resources_scanned: u32,
    pub floor_material_sync_tiles_scanned: u32,
    pub floor_material_sync_elapsed_ms: f32,
    pub wall_material_sync_sites_processed: u32,
    pub wall_material_sync_resources_scanned: u32,
    pub wall_material_sync_tiles_scanned: u32,
    pub wall_material_sync_elapsed_ms: f32,
}

impl TransportRequestMetrics {
    pub fn count_pending(&self) -> u32 {
        *self
            .by_state
            .get(&TransportRequestState::Pending)
            .unwrap_or(&0)
    }

    pub fn count_claimed(&self) -> u32 {
        *self
            .by_state
            .get(&TransportRequestState::Claimed)
            .unwrap_or(&0)
    }
}

/// Wheelbarrow arbitration の期間累積 work counter。
///
/// `TransportRequestMetrics` の arbitration fields は最新フレームの gauge であり、
/// fixed-step checkpoint や capture window の比較には使えない。この resource は
/// profiling build だけで登録し、計測境界でまとめて reset する。
#[cfg(feature = "profiling")]
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct WheelbarrowArbitrationPerfMetrics {
    pub rebuilds: u32,
    pub request_bucket_builds: u32,
    pub bucket_items_scanned: u32,
    pub candidates_after_top_k: u32,
}

pub fn transport_request_metrics_system(
    q_requests: Query<(Entity, &TransportRequest, Option<&TransportRequestState>)>,
    mut metrics: ResMut<TransportRequestMetrics>,
) {
    let mut by_kind = HashMap::new();
    let mut by_state = HashMap::new();
    let mut total = 0u32;

    for (_, req, state_opt) in q_requests.iter() {
        total = total.saturating_add(1);
        *by_kind.entry(req.kind).or_insert(0) += 1;
        let state = state_opt.copied().unwrap_or(TransportRequestState::Pending);
        *by_state.entry(state).or_insert(0) += 1;
    }

    metrics.by_kind = by_kind;
    metrics.by_state = by_state;
    metrics.total = total;
}
