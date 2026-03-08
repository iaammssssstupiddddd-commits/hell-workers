use bevy::prelude::Resource;

#[derive(Default, Clone, PartialEq, Resource)]
pub struct EntityListViewModel {
    pub current: EntityListSnapshot,
    pub previous: EntityListSnapshot,
}

#[derive(Default, Clone, PartialEq)]
pub struct EntityListSnapshot {
    pub familiars: Vec<FamiliarRowViewModel>,
    pub unassigned: Vec<SoulRowViewModel>,
    pub unassigned_folded: bool,
}

#[derive(Clone, PartialEq)]
pub struct FamiliarRowViewModel {
    pub entity: bevy::prelude::Entity,
    pub label: String,
    pub is_folded: bool,
    pub show_empty: bool,
    pub souls: Vec<SoulRowViewModel>,
}

#[derive(Clone, PartialEq)]
pub struct SoulRowViewModel {
    pub entity: bevy::prelude::Entity,
    pub name: String,
    pub gender: i8,
    pub fatigue_text: String,
    pub stress_text: String,
    pub stress_bucket: StressBucket,
    pub dream_text: String,
    pub dream_empty: bool,
    pub task_visual: TaskVisual,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StressBucket {
    Low,
    Medium,
    High,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TaskVisual {
    Idle,
    Chop,
    Mine,
    GatherDefault,
    Haul,
    Build,
    HaulToBlueprint,
    Water,
}
