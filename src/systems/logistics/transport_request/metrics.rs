//! TransportRequest のメトリクスとデバッグ観測
//!
//! M0: 計画の観測基盤。request 数・種別・状態の集計とデバッグログを提供する。
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
    /// 前回ログ出力からの経過秒数（デバッグ間隔制御用）
    pub _log_interval: f32,
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

/// Phase 0: ベースライン比較用の固定メトリクス項目
/// 変更前後の比較のために、以下の項目を固定して記録する:
/// - total: 総 request 数
/// - by_kind: 種別ごとの request 数
/// - by_state: 状態ごとの request 数（Pending, Claimed, InFlight）
/// - wheelbarrow_arb_elapsed_ms: 仲裁システム実行時間
const LOG_INTERVAL: f32 = 5.0;

impl TransportRequestKind {
    fn as_str(&self) -> &'static str {
        match self {
            Self::DepositToStockpile => "DepositToStockpile",
            Self::DeliverToBlueprint => "DeliverToBlueprint",
            Self::DeliverToMixerSolid => "DeliverToMixerSolid",
            Self::DeliverWaterToMixer => "DeliverWaterToMixer",
            Self::GatherWaterToTank => "GatherWaterToTank",
            Self::ReturnBucket => "ReturnBucket",
            Self::BatchWheelbarrow => "BatchWheelbarrow",
            Self::ConsolidateStockpile => "ConsolidateStockpile",
        }
    }
}

/// Perceive フェーズ: メトリクスを再集計し、間隔ごとにデバッグログを出力
pub fn transport_request_metrics_system(
    time: Res<Time>,
    q_requests: Query<(Entity, &TransportRequest, Option<&TransportRequestState>)>,
    mut metrics: ResMut<TransportRequestMetrics>,
) {
    let delta = time.delta_secs();
    metrics._log_interval += delta;

    // 集計
    let mut by_kind = HashMap::new();
    let mut by_state = HashMap::new();

    for (entity, req, state_opt) in q_requests.iter() {
        *by_kind.entry(req.kind).or_insert(0) += 1;
        let state = state_opt.copied().unwrap_or(TransportRequestState::Pending);
        *by_state.entry(state).or_insert(0) += 1;

        // デバッグ: trace レベルで request id / anchor を出力（間隔制御下）
        if metrics._log_interval >= 4.9 {
            trace!(
                "TransportRequest {:?} kind={} anchor={:?} issued_by={:?}",
                entity,
                req.kind.as_str(),
                req.anchor,
                req.issued_by
            );
        }
    }

    let total = q_requests.iter().count() as u32;
    metrics.by_kind = by_kind;
    metrics.by_state = by_state;
    metrics.total = total;

    // デバッグログ: 固定間隔で summary（ベースライン比較用）
    if metrics._log_interval >= LOG_INTERVAL {
        metrics._log_interval = 0.0;
        if total > 0 {
            let mut parts = Vec::new();
            for (kind, count) in &metrics.by_kind {
                parts.push(format!("{}={}", kind.as_str(), count));
            }
            debug!(
                "TransportRequest: total={} [{}] wb_leases={} wb_arb(eligible={}, bucket={}, topk={}, ms={:.3}) task_area(groups={}, scanned={}, matched={}, ms={:.3})",
                total,
                parts.join(", "),
                metrics.wheelbarrow_leases_active,
                metrics.wheelbarrow_arb_eligible_requests,
                metrics.wheelbarrow_arb_bucket_items_total,
                metrics.wheelbarrow_arb_candidates_after_topk,
                metrics.wheelbarrow_arb_elapsed_ms,
                metrics.task_area_groups,
                metrics.task_area_free_items_scanned,
                metrics.task_area_items_matched,
                metrics.task_area_elapsed_ms,
            );
        }
    }
}
