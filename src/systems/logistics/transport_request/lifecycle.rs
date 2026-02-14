//! TransportRequest のライフサイクル管理
//!
//! Maintain フェーズ: アンカー消失時のリクエスト cleanup

use super::{
    ManualHaulPinnedSource, ManualTransportRequest, TransportRequest, TransportRequestFixedSource,
};
use crate::systems::jobs::Designation;
use crate::systems::logistics::{InStockpile, ResourceItem};
use bevy::prelude::*;

/// アンカー（搬入先）が消失した request を close する
///
/// 計画: "anchor 消失: request close（despawn）"
/// - standalone request エンティティ: despawn
/// - アイテムに付与された request: TransportRequest と Designation を remove
pub fn transport_request_anchor_cleanup_system(
    mut commands: Commands,
    q_requests: Query<(
        Entity,
        &TransportRequest,
        Option<&super::TransportDemand>,
        Option<&crate::relationships::TaskWorkers>,
        Option<&ManualTransportRequest>,
        Option<&TransportRequestFixedSource>,
    )>,
    q_any: Query<Entity>,
    q_items: Query<Entity, With<ResourceItem>>,
    q_familiars: Query<Entity, With<crate::entities::familiar::Familiar>>,
    q_in_stockpile: Query<(), With<InStockpile>>,
) {
    for (request_entity, req, demand_opt, workers_opt, manual_opt, fixed_source_opt) in
        q_requests.iter()
    {
        let mut should_close = false;
        let workers = workers_opt.map(|w| w.len()).unwrap_or(0);

        // 1. アンカー（搬入先）が消失した
        if q_any.get(req.anchor).is_err() {
            should_close = true;
        }

        // 2. 需要がゼロになり、かつ作業中のワーカーもいない
        if let Some(demand) = demand_opt {
            if demand.desired_slots == 0 && workers == 0 {
                should_close = true;
            }
        }

        // 3. 発行者（Familiar）が消失した
        if q_any.get(req.issued_by).is_err() || q_familiars.get(req.issued_by).is_err() {
            should_close = true;
        }

        // 4. manual request の固定 source が消失/搬送済みなら close
        if manual_opt.is_some() {
            match fixed_source_opt {
                Some(source) => {
                    if q_any.get(source.0).is_err() {
                        should_close = true;
                    }
                    if workers == 0 && q_in_stockpile.get(source.0).is_ok() {
                        should_close = true;
                    }
                }
                None => {
                    should_close = true;
                }
            }
        }

        if should_close {
            if let Some(source) = fixed_source_opt
                && q_any.get(source.0).is_ok()
            {
                commands.entity(source.0).remove::<ManualHaulPinnedSource>();
            }

            if q_items.get(request_entity).is_ok() {
                commands
                    .entity(request_entity)
                    .remove::<TransportRequest>()
                    .remove::<Designation>()
                    .remove::<super::TransportDemand>()
                    .remove::<super::TransportPolicy>()
                    .remove::<super::WheelbarrowLease>()
                    .remove::<super::WheelbarrowPendingSince>()
                    .remove::<ManualTransportRequest>()
                    .remove::<TransportRequestFixedSource>();
            } else {
                commands.entity(request_entity).despawn();
            }
        }
    }
}
