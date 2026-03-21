use bevy::prelude::*;
use hw_core::logistics::ResourceType;

#[derive(Reflect, Clone, Debug, PartialEq)]
pub struct RefineData {
    pub mixer: Entity,
    pub phase: RefinePhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Reflect, Default)]
pub enum RefinePhase {
    #[default]
    GoingToMixer,
    Refining {
        progress: f32,
    },
    Done,
}

#[derive(Reflect, Clone, Debug, PartialEq, Eq)]
pub struct HaulToMixerData {
    pub item: Entity,
    pub mixer: Entity,
    pub resource_type: ResourceType,
    pub phase: HaulToMixerPhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Default)]
pub enum HaulToMixerPhase {
    #[default]
    GoingToItem,
    GoingToMixer,
    Delivering,
}
