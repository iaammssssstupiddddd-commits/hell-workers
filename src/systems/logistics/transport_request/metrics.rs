//! TransportRequest のメトリクスとデバッグ観測
//!
//! M0: 計画の観測基盤。request 数・種別・状態の集計とデバッグログを提供する。

use super::{TransportLease, TransportRequest, TransportRequestKind, TransportRequestState};
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
}

impl TransportRequestMetrics {
    pub fn count_pending(&self) -> u32 {
        *self.by_state.get(&TransportRequestState::Pending).unwrap_or(&0)
    }

    pub fn count_claimed(&self) -> u32 {
        *self.by_state.get(&TransportRequestState::Claimed).unwrap_or(&0)
    }

    pub fn count_in_flight(&self) -> u32 {
        *self.by_state.get(&TransportRequestState::InFlight).unwrap_or(&0)
    }
}

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
        }
    }
}

/// Perceive フェーズ: メトリクスを再集計し、間隔ごとにデバッグログを出力
pub fn transport_request_metrics_system(
    time: Res<Time>,
    q_requests: Query<
        (
            Entity,
            &TransportRequest,
            Option<&TransportRequestState>,
            Option<&TransportLease>,
        ),
    >,
    mut metrics: ResMut<TransportRequestMetrics>,
) {
    let delta = time.delta_secs();
    metrics._log_interval += delta;

    // 集計
    let mut by_kind = HashMap::new();
    let mut by_state = HashMap::new();

    for (entity, req, state_opt, lease_opt) in q_requests.iter() {
        *by_kind.entry(req.kind).or_insert(0) += 1;
        let state = state_opt.copied().unwrap_or(TransportRequestState::Pending);
        *by_state.entry(state).or_insert(0) += 1;

        // デバッグ: trace レベルで request id / anchor / worker を出力（間隔制御下）
        if metrics._log_interval >= 4.9 {
            let worker = lease_opt.map(|l| l.claimed_by_worker);
            trace!(
                "TransportRequest {:?} kind={} anchor={:?} issued_by={:?} worker={:?}",
                entity,
                req.kind.as_str(),
                req.anchor,
                req.issued_by,
                worker
            );
        }
    }

    let total = q_requests.iter().count() as u32;
    metrics.by_kind = by_kind;
    metrics.by_state = by_state;
    metrics.total = total;

    // デバッグログ: 5秒間隔で summary
    const LOG_INTERVAL: f32 = 5.0;
    if metrics._log_interval >= LOG_INTERVAL {
        metrics._log_interval = 0.0;
        if total > 0 {
            let mut parts = Vec::new();
            for (kind, count) in &metrics.by_kind {
                parts.push(format!("{}={}", kind.as_str(), count));
            }
            debug!(
                "TransportRequest: total={} [{}] wb_leases={}",
                total,
                parts.join(", "),
                metrics.wheelbarrow_leases_active,
            );
        }
    }
}
