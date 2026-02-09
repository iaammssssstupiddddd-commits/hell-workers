use bevy::prelude::*;

use crate::entities::damned_soul::DamnedSoulSpawnEvent;
use crate::entities::familiar::FamiliarSpawnEvent;
use crate::events::{
    DesignationRequest, EncouragementRequest, EscapeRequest, FamiliarAiStateChangedEvent,
    FamiliarIdleVisualRequest, FamiliarOperationMaxSoulChangedEvent, FamiliarStateRequest,
    GatheringManagementRequest, IdleBehaviorRequest, OnExhausted, OnGatheringParticipated,
    ResourceReservationRequest, SquadManagementRequest, TaskAssignmentRequest,
};
use crate::systems::visual::speech::conversation::events::{
    ConversationCompleted, ConversationToneTriggered, RequestConversation,
};

pub struct MessagesPlugin;

impl Plugin for MessagesPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<DamnedSoulSpawnEvent>()
            .add_message::<FamiliarSpawnEvent>()
            .add_message::<FamiliarOperationMaxSoulChangedEvent>()
            .add_message::<FamiliarAiStateChangedEvent>()
            .add_message::<TaskAssignmentRequest>()
            .add_message::<ResourceReservationRequest>()
            .add_message::<SquadManagementRequest>()
            .add_message::<IdleBehaviorRequest>()
            .add_message::<EscapeRequest>()
            .add_message::<GatheringManagementRequest>()
            .add_message::<DesignationRequest>()
            .add_message::<FamiliarStateRequest>()
            .add_message::<EncouragementRequest>()
            .add_message::<FamiliarIdleVisualRequest>()
            .add_message::<RequestConversation>()
            .add_message::<ConversationCompleted>()
            .add_message::<ConversationToneTriggered>()
            .add_message::<OnExhausted>()
            .add_message::<OnGatheringParticipated>();
    }
}
