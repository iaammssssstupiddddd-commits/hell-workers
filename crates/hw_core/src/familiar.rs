use bevy::prelude::*;

#[derive(Component, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
pub enum FamiliarAiState {
    Idle,
    SearchingTask,
    Scouting { target_soul: Entity },
    Supervising {
        target: Option<Entity>,
        timer: f32,
    },
}

impl Default for FamiliarAiState {
    fn default() -> Self {
        Self::Idle
    }
}
