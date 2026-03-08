use bevy::prelude::*;

pub mod intents;
pub use intents::UiIntent;

pub struct HwUiPlugin;

impl Plugin for HwUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<UiIntent>();
    }
}
