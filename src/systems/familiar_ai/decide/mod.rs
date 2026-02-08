use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::events::{
    EncouragementRequest, FamiliarAiStateChangedEvent, FamiliarStateRequest, SquadManagementRequest,
};

pub mod encouragement;
pub mod following;
pub mod state_decision;
pub mod task_delegation;

/// Familiar Decide フェーズの共通出力チャネル
#[derive(SystemParam)]
pub struct FamiliarDecideOutput<'w> {
    pub state_changed_events: MessageWriter<'w, FamiliarAiStateChangedEvent>,
    pub state_requests: MessageWriter<'w, FamiliarStateRequest>,
    pub squad_requests: MessageWriter<'w, SquadManagementRequest>,
    pub encouragement_requests: MessageWriter<'w, EncouragementRequest>,
}
