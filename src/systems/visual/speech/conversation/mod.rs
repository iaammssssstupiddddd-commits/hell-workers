pub mod components;
pub mod events;
pub mod systems;

use bevy::prelude::*;
use components::*;
use events::*;
use systems::*;

pub struct ConversationPlugin;

impl Plugin for ConversationPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<RequestConversation>()
            .add_message::<ConversationCompleted>()
            .register_type::<ConversationInitiator>()
            .register_type::<ConversationParticipant>()
            .register_type::<ConversationCooldown>()
            .add_systems(
                Update,
                (
                    check_conversation_triggers,
                    handle_conversation_requests,
                    process_conversation_logic,
                    apply_conversation_rewards,
                    update_conversation_cooldowns,
                ),
            );
    }
}
