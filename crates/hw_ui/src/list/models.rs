use bevy::prelude::*;
use std::collections::HashMap;

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
    pub entity: Entity,
    pub label: String,
    pub is_folded: bool,
    pub show_empty: bool,
    pub souls: Vec<SoulRowViewModel>,
}

#[derive(Clone, PartialEq)]
pub struct SoulRowViewModel {
    pub entity: Entity,
    pub name: String,
    pub gender: SoulGender,
    pub fatigue_text: String,
    pub stress_text: String,
    pub stress_bucket: StressBucket,
    pub dream_text: String,
    pub dream_empty: bool,
    pub task_visual: TaskVisual,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SoulGender {
    Male,
    Female,
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

/// エンティティリストUI ノードの参照インデックス（差分同期用）
#[derive(Resource, Default)]
pub struct EntityListNodeIndex {
    pub familiar_sections: HashMap<Entity, FamiliarSectionNodes>,
    pub familiar_member_rows: HashMap<Entity, HashMap<Entity, Entity>>,
    pub familiar_empty_rows: HashMap<Entity, Entity>,
    pub unassigned_rows: HashMap<Entity, Entity>,
}

/// 使い魔セクション内の主要UIノードへの参照
#[derive(Clone, Copy)]
pub struct FamiliarSectionNodes {
    pub root: Entity,
    pub header_text: Entity,
    pub fold_icon: Entity,
    pub members_container: Entity,
}
