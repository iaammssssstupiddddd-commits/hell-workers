pub mod animation;
pub mod components;
pub mod conversation;
pub mod cooldown;
pub mod emitter;
pub mod idle_visual;
pub mod max_soul_visual;
pub mod observers;
pub mod periodic;
pub mod phrases;
pub mod spawn;
pub mod squad_visual;
pub mod typewriter;
pub mod update;
pub mod voice;

pub use idle_visual::familiar_idle_visual_apply_system;
pub use max_soul_visual::max_soul_visual_system;
pub use squad_visual::squad_visual_system;
pub use voice::FamiliarVoice;

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
                .chain()
                .in_set(hw_core::system_sets::GameSystemSet::Visual),
        );
        app.add_systems(
            Update,
            (
                observers::speech_on_task_assigned_system,
                observers::speech_on_task_completed_system,
            )
                .in_set(hw_core::system_sets::GameSystemSet::Visual),
        );

        // Observers の登録
        app.add_observer(on_soul_recruited);
        app.add_observer(on_exhausted);
        app.add_observer(on_stress_breakdown);
        app.add_observer(on_released_from_service);
        app.add_observer(on_gathering_joined);
        app.add_observer(on_task_abandoned);
        app.add_observer(on_encouraged);
    }
}
