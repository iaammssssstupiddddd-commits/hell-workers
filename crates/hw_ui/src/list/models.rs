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

/// Named value nodes owned by a Soul row; independent from child layout order.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SoulRowNodes {
    pub gender_icon: Entity,
    pub name_text: Entity,
    pub fatigue_text: Entity,
    pub stress_text: Entity,
    pub dream_text: Entity,
    pub task_icon: Entity,
    pub task_label: Entity,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskVisual {
    Idle,
    Chop,
    Mine,
    GatherDefault,
    Haul,
    Build,
    HaulToBlueprint,
    Water,
    GeneratePower,
    Deconstruct,
    Move,
    Refine,
    CollectBone,
}

impl TaskVisual {
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "待機",
            Self::Chop => "伐採",
            Self::Mine => "採掘",
            Self::GatherDefault => "採取",
            Self::Haul => "運搬",
            Self::Build => "施工",
            Self::HaulToBlueprint => "搬入",
            Self::Water => "給水",
            Self::GeneratePower => "発電",
            Self::Deconstruct => "解体",
            Self::Move => "移設",
            Self::Refine => "精製",
            Self::CollectBone => "骨回収",
        }
    }
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
