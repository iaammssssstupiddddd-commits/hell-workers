use crate::entities::damned_soul::SoulIdentity;
use bevy::prelude::*;
use hw_ui::TextInputIntent;

pub fn handle_text_input_intents_system(
    mut intents: MessageReader<TextInputIntent>,
    mut q_identity: Query<&mut SoulIdentity>,
) {
    for intent in intents.read() {
        let TextInputIntent::RenameSoul { entity, name } = intent;
        let trimmed = name.trim();
        if trimmed.is_empty() || trimmed.chars().count() > 32 {
            continue;
        }
        if let Ok(mut identity) = q_identity.get_mut(*entity) {
            identity.name = trimmed.to_string();
        }
    }
}
