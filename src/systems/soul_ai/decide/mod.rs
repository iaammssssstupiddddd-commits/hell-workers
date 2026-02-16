use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::events::{EscapeRequest, GatheringManagementRequest};

pub mod escaping;
pub mod drifting;
pub mod gathering_mgmt;
pub mod idle_behavior;
pub mod separation;
pub mod work;

/// Soul Decide フェーズの共通出力チャネル
#[derive(SystemParam)]
pub struct SoulDecideOutput<'w> {
    pub escape_requests: MessageWriter<'w, EscapeRequest>,
    pub gathering_requests: MessageWriter<'w, GatheringManagementRequest>,
}
