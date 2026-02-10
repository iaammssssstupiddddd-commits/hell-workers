//! Transport Request State Machine
//!
//! TaskWorkers の有無に基づいて TransportRequestState を自動更新します。

use bevy::prelude::*;
use crate::relationships::TaskWorkers;
use super::components::{TransportRequest, TransportRequestState};

/// TransportRequest エンティティの状態を、アサイン状況 (TaskWorkers) に基づいて同期する
pub fn transport_request_state_sync_system(
    mut q_requests: Query<(&TaskWorkers, &mut TransportRequestState), (With<TransportRequest>, Changed<TaskWorkers>)>,
) {
    for (workers, mut state) in q_requests.iter_mut() {
        if workers.is_empty() {
            // ワーカーが不在なら Pending に戻す
            if *state != TransportRequestState::Pending {
                *state = TransportRequestState::Pending;
            }
        } else {
            // ワーカーが存在する場合
            // 現在は単純化のため、1人でもいれば Claimed とする。
            // 本来は AssignedTask の状態を見て InFlight に遷移させるべきだが、
            // 現状の assignment/builders 構成では Claimed と InFlight の区別が難しいため、
            // まずは Claimed に統一する。
            if *state == TransportRequestState::Pending {
                *state = TransportRequestState::Claimed;
            }
        }
    }
}
