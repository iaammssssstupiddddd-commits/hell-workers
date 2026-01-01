use bevy::prelude::*;

#[derive(Resource)]
pub struct GameAssets {
    pub grass: Handle<Image>,
    pub dirt: Handle<Image>,
    pub stone: Handle<Image>,
    pub colonist: Handle<Image>,
    pub wall: Handle<Image>,
    pub wood: Handle<Image>,
}
