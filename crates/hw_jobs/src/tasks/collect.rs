use bevy::prelude::*;

#[derive(Reflect, Clone, Debug, PartialEq)]
pub struct CollectBoneData {
    pub target: Entity,
    pub phase: CollectBonePhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Reflect, Default)]
pub enum CollectBonePhase {
    #[default]
    GoingToBone,
    Collecting {
        progress: f32,
    },
    Done,
}
