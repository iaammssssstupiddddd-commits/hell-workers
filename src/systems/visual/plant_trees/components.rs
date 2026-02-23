use bevy::prelude::*;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlantTreeVisualPhase {
    MagicCircle,
    Growth,
    LifeSpark,
}

#[derive(Component, Debug)]
pub struct PlantTreeVisualState {
    pub phase: PlantTreeVisualPhase,
    pub phase_elapsed: f32,
}

impl Default for PlantTreeVisualState {
    fn default() -> Self {
        Self {
            phase: PlantTreeVisualPhase::MagicCircle,
            phase_elapsed: 0.0,
        }
    }
}

#[derive(Component, Debug)]
pub struct PlantTreeMagicCircle {
    pub elapsed: f32,
}

#[derive(Component, Debug)]
pub struct PlantTreeLifeSpark {
    pub velocity: Vec2,
    pub lifetime: f32,
    pub max_lifetime: f32,
}
