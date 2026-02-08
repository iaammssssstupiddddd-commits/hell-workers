use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::events::{
    EncouragementRequest, FamiliarAiStateChangedEvent, FamiliarIdleVisualRequest,
    FamiliarStateRequest, SquadManagementRequest,
};

pub mod encouragement;
pub mod familiar_processor;
pub mod following;
pub mod recruitment;
pub mod scouting;
pub mod squad;
pub mod state_decision;
pub mod state_handlers;
pub mod supervising;
pub mod task_delegation;
pub mod task_management;

/// Familiar Decide フェーズの共通出力チャネル
#[derive(SystemParam)]
pub struct FamiliarDecideOutput<'w> {
    pub state_changed_events: MessageWriter<'w, FamiliarAiStateChangedEvent>,
    pub state_requests: MessageWriter<'w, FamiliarStateRequest>,
    pub squad_requests: MessageWriter<'w, SquadManagementRequest>,
    pub encouragement_requests: MessageWriter<'w, EncouragementRequest>,
    pub idle_visual_requests: MessageWriter<'w, FamiliarIdleVisualRequest>,
}
