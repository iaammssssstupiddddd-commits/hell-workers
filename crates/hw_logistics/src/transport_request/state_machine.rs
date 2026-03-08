//! Transport Request State Machine
//!
//! TaskWorkers の有無に基づいて TransportRequestState を自動更新します。

use bevy::prelude::*;
use hw_core::relationships::TaskWorkers;

use crate::transport_request::{TransportRequest, TransportRequestState};

pub fn transport_request_state_sync_system(
    mut q_requests: Query<
        (&TaskWorkers, &mut TransportRequestState),
        (With<TransportRequest>, Changed<TaskWorkers>),
    >,
) {
    for (workers, mut state) in q_requests.iter_mut() {
        if workers.is_empty() {
            if *state != TransportRequestState::Pending {
                *state = TransportRequestState::Pending;
            }
        } else if *state == TransportRequestState::Pending {
            *state = TransportRequestState::Claimed;
        }
    }
}
