use bevy::prelude::*;

#[derive(Resource, Default, Reflect)]
#[reflect(Resource)]
pub struct GameTime {
    pub seconds: f32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
}
