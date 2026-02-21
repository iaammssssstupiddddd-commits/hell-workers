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
    pub lifetime: f32,
    pub max_lifetime: f32,
    pub start_pos: Vec2,
    pub target_pos: Vec2,
    pub control_point_1: Vec2,
    pub control_point_2: Vec2,
    pub control_point_3: Vec2,
    pub phase: f32,
    pub merge_count: u8,
    pub merging_into: Option<Entity>,
    pub merge_timer: f32,
    pub trail_cooldown: f32,
    pub prev_pos: Vec2,
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

