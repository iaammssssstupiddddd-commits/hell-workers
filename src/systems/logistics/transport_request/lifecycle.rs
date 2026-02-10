//! TransportRequest のライフサイクル管理
//!
//! Maintain フェーズ: アンカー消失時のリクエスト cleanup

use super::TransportRequest;
use crate::systems::jobs::Designation;
use crate::systems::logistics::ResourceItem;
use bevy::prelude::*;

/// アンカー（搬入先）が消失した request を close する
///
/// 計画: "anchor 消失: request close（despawn）"
/// - standalone request エンティティ: despawn
/// - アイテムに付与された request: TransportRequest と Designation を remove
pub fn transport_request_anchor_cleanup_system(
    mut commands: Commands,
    q_requests: Query<(Entity, &TransportRequest)>,
    q_any: Query<Entity>,
    q_items: Query<Entity, With<ResourceItem>>,
) {
    for (request_entity, req) in q_requests.iter() {
        if q_any.get(req.anchor).is_err() {
            if q_items.get(request_entity).is_ok() {
                commands
                    .entity(request_entity)
                    .remove::<TransportRequest>()
                    .remove::<Designation>();
            } else {
                commands.entity(request_entity).despawn();
            }
        }
    }
}
