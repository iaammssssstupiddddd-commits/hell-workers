pub mod animation;
pub mod components;
pub mod conversation;
pub mod cooldown;
pub mod observers;
pub mod periodic;
pub mod phrases;
pub mod spawn;
pub mod typewriter;
pub mod update;

use bevy::prelude::*;
use conversation::ConversationPlugin;
use observers::*;

pub struct SpeechPlugin;

impl Plugin for SpeechPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<cooldown::SpeechHistory>();
        app.init_resource::<periodic::PeriodicEmotionFrameCounter>();
        app.add_plugins(ConversationPlugin);
        app.add_systems(
            Update,
            (
                update::update_bubble_stacking, // 追従の前にオフセットを確定させる
                update::update_speech_bubbles,
                animation::animate_speech_bubbles,
                typewriter::update_typewriter,
                periodic::periodic_emotion_system,
                observers::reaction_delay_system,
            )
                .chain(),
        );

        // Observers の登録
        app.add_observer(on_task_assigned);
        app.add_observer(on_task_completed);
        app.add_observer(on_soul_recruited);
        app.add_observer(on_exhausted);
        app.add_observer(on_stress_breakdown);
        app.add_observer(on_released_from_service);
        app.add_observer(on_gathering_joined);
        app.add_observer(on_task_abandoned);
        app.add_observer(on_encouraged);
    }
}
