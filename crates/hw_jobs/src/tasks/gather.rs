use bevy::prelude::*;
use hw_core::jobs::WorkType;

#[derive(Reflect, Clone, Debug, PartialEq)]
pub struct GatherData {
    pub target: Entity,
    pub work_type: WorkType,
    pub phase: GatherPhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Reflect, Default)]
pub enum GatherPhase {
    #[default]
    GoingToResource,
    Collecting {
        progress: f32,
    },
    Done,
}
