use bevy::prelude::*;

#[derive(Reflect, Clone, Debug, PartialEq)]
pub struct MovePlantData {
    pub task_entity: Entity,
    pub building: Entity,
    pub destination_grid: (i32, i32),
    pub destination_pos: Vec2,
    pub companion_anchor: Option<(i32, i32)>,
    pub phase: MovePlantPhase,
}

#[derive(Component, Reflect, Clone, Debug, PartialEq)]
#[reflect(Component)]
pub struct MovePlantTask {
    pub building: Entity,
    pub destination_grid: (i32, i32),
    pub destination_pos: Vec2,
    pub companion_anchor: Option<(i32, i32)>,
}

/// Runtime apply snapshot while a completed building move is being committed.
///
/// This lives beside the durable Move payload so every assignment producer can
/// enforce the same Move-vs-Deconstruct eligibility contract without depending
/// on the Soul executor crate.
#[derive(Component, Debug, Clone)]
pub struct PendingBuildingMove {
    pub old_occupied: Vec<(i32, i32)>,
    pub new_occupied: Vec<(i32, i32)>,
    pub companion_anchor: Option<(i32, i32)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Default)]
pub enum MovePlantPhase {
    #[default]
    GoToBuilding,
    Moving,
    Done,
}
