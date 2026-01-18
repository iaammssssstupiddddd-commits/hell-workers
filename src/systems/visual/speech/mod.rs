pub mod components;
pub mod observers;
pub mod phrases;
pub mod spawn;
pub mod update;

use bevy::prelude::*;
use observers::*;
use update::update_speech_bubbles;

pub struct SpeechPlugin;

impl Plugin for SpeechPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, update_speech_bubbles);

        // Observers の登録
        app.add_observer(on_task_assigned);
        app.add_observer(on_task_completed);
        app.add_observer(on_soul_recruited);
        app.add_observer(on_exhausted);
        app.add_observer(on_stress_breakdown);
    }
}
