use crate::entities::damned_soul::DreamQuality;
use crate::systems::utils::floating_text::FloatingText;
use bevy::prelude::*;

#[derive(Component)]
pub struct DreamParticle {
    pub owner: Entity,
    pub quality: DreamQuality,
    pub lifetime: f32,
    pub max_lifetime: f32,
    pub velocity: Vec2,
    pub phase: f32,
}

#[derive(Component, Default)]
pub struct DreamVisualState {
    pub particle_cooldown: f32,
    pub popup_accumulated: f32,
    pub active_particles: u8,
}

#[derive(Component, Clone)]
pub struct DreamGainPopup {
    pub floating_text: FloatingText,
}

#[derive(Component)]
pub struct DreamGainUiParticle {
    pub time_alive: f32,
    pub target_pos: Vec2,
    pub start_pos: Vec2,
    pub velocity: Vec2,
    pub phase: f32,
    pub noise_direction: Vec2,
    pub noise_timer: f32,
    pub merge_count: u8,
    pub merging_into: Option<Entity>,
    pub merge_timer: f32,
    pub trail_cooldown: f32,
    pub prev_pos: Vec2,
    pub mass: f32,
}

#[derive(Component)]
pub struct DreamTrailGhost {
    pub lifetime: f32,
    pub max_lifetime: f32,
}

#[derive(Component, Default)]
pub struct DreamIconAbsorb {
    pub timer: f32,
    pub pulse_count: u8,
}

