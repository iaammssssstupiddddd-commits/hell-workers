//! TransportRequest のメトリクス集計
//!
//! M0: 計画の観測基盤。request 数・種別・状態の集計を提供する。
//! Phase 0: 比較可能なメトリクス項目を固定し、ベースライン観測を整備。

use super::{TransportRequest, TransportRequestKind, TransportRequestState};
use bevy::prelude::*;
use std::collections::HashMap;

/// TransportRequest の集計メトリクス
#[derive(Resource, Default, Debug)]
pub struct TransportRequestMetrics {
    /// 種別ごとの request 数
    pub by_kind: HashMap<TransportRequestKind, u32>,
    /// 状態ごとの request 数
    pub by_state: HashMap<TransportRequestState, u32>,
    /// 総 request 数
    pub total: u32,
    /// 手押し車リースのアクティブ数
    pub wheelbarrow_leases_active: u32,
    /// このフレームで付与された手押し車リース数
    pub wheelbarrow_leases_granted_this_frame: u32,
    /// 仲裁対象として評価した request 数
    pub wheelbarrow_arb_eligible_requests: u32,
    /// 仲裁時に request が参照したバケット候補数（Top-K 前）
    pub wheelbarrow_arb_bucket_items_total: u32,
    /// 仲裁時に Top-K 抽出後に残った候補数
    pub wheelbarrow_arb_candidates_after_topk: u32,
    /// 候補間重複除去で除外された item 数
    pub wheelbarrow_arb_items_deduped: u32,
    /// 重複除去で hard_min 未満となりスキップされた候補数
    pub wheelbarrow_arb_candidates_dropped_by_dedup: u32,
    /// 仲裁対象 request の平均 pending 時間（秒）
    pub wheelbarrow_arb_avg_pending_secs: f32,
    /// このフレームで付与した lease の平均期間（秒）
    pub wheelbarrow_arb_avg_lease_duration: f32,
    /// 仲裁システムの実行時間（ms）
    pub wheelbarrow_arb_elapsed_ms: f32,
    /// task area producer が評価した Stockpile グループ数
    pub task_area_groups: u32,
    /// task area producer が走査した free item 数
    pub task_area_free_items_scanned: u32,
    /// task area producer で条件一致した item 数
    pub task_area_items_matched: u32,
    /// task area producer システムの実行時間（ms）
    pub task_area_elapsed_ms: f32,
    /// floor material sync が処理した Site 数
    pub floor_material_sync_sites_processed: u32,
    /// floor material sync が走査した resource 数
    pub floor_material_sync_resources_scanned: u32,
    /// floor material sync が走査した tile 数
    pub floor_material_sync_tiles_scanned: u32,
    /// floor material sync システムの実行時間（ms）
    pub floor_material_sync_elapsed_ms: f32,
    /// wall material sync が処理した Site 数
    pub wall_material_sync_sites_processed: u32,
    /// wall material sync が走査した resource 数
    pub wall_material_sync_resources_scanned: u32,
    /// wall material sync が走査した tile 数
    pub wall_material_sync_tiles_scanned: u32,
    /// wall material sync システムの実行時間（ms）
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

/// Perceive フェーズ: メトリクスを再集計する
pub fn transport_request_metrics_system(
    q_requests: Query<(Entity, &TransportRequest, Option<&TransportRequestState>)>,
    mut metrics: ResMut<TransportRequestMetrics>,
) {
    // 集計
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
