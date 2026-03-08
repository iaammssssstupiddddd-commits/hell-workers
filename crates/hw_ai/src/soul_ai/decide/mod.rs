pub mod escaping;
pub mod idle_behavior;
pub mod gathering_mgmt;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::events::{EscapeRequest, GatheringManagementRequest};

#[derive(SystemParam)]
pub struct SoulDecideOutput<'w> {
    pub escape_requests: MessageWriter<'w, EscapeRequest>,
    pub gathering_requests: MessageWriter<'w, GatheringManagementRequest>,
}
