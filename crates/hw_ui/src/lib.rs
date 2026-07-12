use bevy::prelude::*;

pub mod area_edit;
pub mod intents;
pub use intents::UiIntent;
pub mod components;
pub mod interaction;
pub mod list;
pub mod models;
pub mod panels;
pub mod plugins;
pub mod setup;
pub mod text_input_intents;
pub use text_input_intents::TextInputIntent;
pub mod theme;
pub mod widgets;

pub struct HwUiPlugin;

impl Plugin for HwUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<UiIntent>()
            .add_message::<TextInputIntent>();
    }
}
pub mod camera;
pub mod selection;
