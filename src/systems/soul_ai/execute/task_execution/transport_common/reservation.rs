//! 運搬タスクの予約解放・記録ヘルパ
//!
//! Release* / Record* の発火を共通API化し、失敗経路での解放漏れを防ぐ。

use crate::systems::logistics::ResourceType;
use crate::systems::soul_ai::execute::task_execution::context::TaskExecutionContext;
use bevy::prelude::*;

/// ストックパイル/ブループリントの目的地予約を解放
pub fn release_destination(_ctx: &mut TaskExecutionContext, _target: Entity) {
    // Relationship を利用するため、明示的な解放 Op は不要。
    // Soul の AssignedTask が変更されるか、アイテムから DeliveringTo が消えれば自動で減る。
}

/// ソース（アイテム）の予約を解放
pub fn release_source(ctx: &mut TaskExecutionContext, source: Entity, amount: usize) {
    ctx.queue_reservation(crate::events::ResourceReservationOp::ReleaseSource {
        source,
        amount,
    });
}

/// ミキサー目的地の予約を解放
pub fn release_mixer_destination(
    ctx: &mut TaskExecutionContext,
    target: Entity,
    resource_type: ResourceType,
) {
    ctx.queue_reservation(crate::events::ResourceReservationOp::ReleaseMixerDestination {
        target,
        resource_type,
    });
}

/// ソース取得を記録（Delta Update用）
pub fn record_picked_source(ctx: &mut TaskExecutionContext, source: Entity, amount: usize) {
    ctx.queue_reservation(crate::events::ResourceReservationOp::RecordPickedSource {
        source,
        amount,
    });
}

/// 目的地への格納を記録（Delta Update用）
pub fn record_stored_destination(_ctx: &mut TaskExecutionContext, _target: Entity) {
    // DeliveringTo を利用するため、ここでは何もしない。
    // 格納完了時にアイテムを despawn するか、リレーションシップを外せばよい。
}
