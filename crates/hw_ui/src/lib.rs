use bevy::prelude::*;

pub mod intents;
pub use intents::UiIntent;
pub mod components;
pub mod theme;
pub mod setup;
pub mod plugins;
pub mod interaction;
pub mod panels;
pub mod list;
pub mod models;

pub struct HwUiPlugin;

impl Plugin for HwUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<UiIntent>();
    }
}
pub mod selection;
pub mod camera;
