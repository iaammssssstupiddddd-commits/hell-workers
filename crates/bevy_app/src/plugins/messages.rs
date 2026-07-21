use bevy::prelude::*;

use crate::entities::damned_soul::DamnedSoulSpawnEvent;
use crate::entities::familiar::FamiliarSpawnEvent;
use crate::interface::ui::panels::task_list::TaskActionOutcome;
use crate::{
    DesignationRequest, EncouragementRequest, EscapeRequest, FamiliarAiStateChangedEvent,
    FamiliarIdleVisualRequest, FamiliarOperationMaxSoulChangedEvent, FamiliarStateRequest,
    GatheringManagementRequest, GatheringSpawnRequest, IdleBehaviorRequest,
    ResourceReservationRequest, SoulTaskUnassignRequest, SquadManagementRequest,
    TaskAssignmentRequest,
};
use hw_core::events::{
    DreamTransferredVisualMessage, OnGatheringJoined, OnGatheringParticipated,
    OnReleasedFromService, OnTaskAbandoned, OnTaskAssigned, SoulEncouragedVisualMessage,
    SoulExhaustedVisualMessage, SoulRecruitedVisualMessage, SoulStressBreakdownVisualMessage,
    TaskCompletedVisualMessage,
};
use hw_visual::speech::conversation::events::{
    ConversationCompleted, ConversationToneTriggered, RequestConversation,
};

macro_rules! root_message_types {
    ($callback:ident, $argument:expr) => {
        $callback!(
            $argument;
            DamnedSoulSpawnEvent,
            FamiliarSpawnEvent,
            FamiliarOperationMaxSoulChangedEvent,
            FamiliarAiStateChangedEvent,
            TaskAssignmentRequest,
            ResourceReservationRequest,
            SquadManagementRequest,
            IdleBehaviorRequest,
            EscapeRequest,
            GatheringManagementRequest,
            DesignationRequest,
            FamiliarStateRequest,
            EncouragementRequest,
            FamiliarIdleVisualRequest,
            RequestConversation,
            ConversationCompleted,
            ConversationToneTriggered,
            SoulRecruitedVisualMessage,
            SoulStressBreakdownVisualMessage,
            SoulExhaustedVisualMessage,
            TaskCompletedVisualMessage,
            SoulEncouragedVisualMessage,
            DreamTransferredVisualMessage,
            OnReleasedFromService,
            OnGatheringJoined,
            OnTaskAbandoned,
            OnGatheringParticipated,
            OnTaskAssigned,
            GatheringSpawnRequest,
            SoulTaskUnassignRequest,
            TaskActionOutcome,
        );
    };
}

macro_rules! add_root_messages {
    ($app:expr; $($message:ty),+ $(,)?) => {
        $(
            $app.add_message::<$message>();
        )+
    };
}

macro_rules! clear_root_messages_by_type {
    ($world:expr; $($message:ty),+ $(,)?) => {
        $(
            clear_message::<$message>($world);
        )+
    };
}

pub struct MessagesPlugin;

impl Plugin for MessagesPlugin {
    fn build(&self, app: &mut App) {
        root_message_types!(add_root_messages, app);
        crate::systems::save::register_load_reset_hook(app, "root-messages", clear_root_messages);
    }
}

/// Clears every root-owned message buffer before a persistent world is
/// replaced. The same type inventory initializes the buffers in
/// [`MessagesPlugin`], so new root message types cannot be registered without
/// also participating in this reset.
pub(crate) fn clear_root_messages(world: &mut World) {
    root_message_types!(clear_root_messages_by_type, world);
}

fn clear_message<T: Message>(world: &mut World) {
    if let Some(mut messages) = world.get_resource_mut::<Messages<T>>() {
        messages.clear();
    }
}

#[cfg(test)]
mod tests;
