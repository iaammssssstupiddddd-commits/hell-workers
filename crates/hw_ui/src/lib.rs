use bevy::prelude::*;

pub mod intents;
pub use intents::UiIntent;
pub mod components;
pub mod interaction;
pub mod list;
pub mod models;
pub mod panels;
pub mod plugins;
pub mod setup;
pub mod theme;

pub struct HwUiPlugin;

impl Plugin for HwUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<UiIntent>();
    }
}
pub mod camera;
pub mod selection;
