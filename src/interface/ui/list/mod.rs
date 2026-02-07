//! エンティティリストの動的更新システム

use bevy::prelude::*;
use std::collections::HashMap;

mod helpers;
mod interaction;
mod sync;
mod view_model;

pub use interaction::{
    entity_list_interaction_system, entity_list_scroll_hint_visibility_system,
    entity_list_scroll_system, entity_list_tab_focus_system, entity_list_visual_feedback_system,
    update_unassigned_arrow_icon_system,
};
pub use sync::sync_entity_list_from_view_model_system;
pub use view_model::build_entity_list_view_model_system;

#[derive(Resource, Default, Clone, PartialEq)]
pub struct EntityListViewModel {
    pub(super) current: EntityListSnapshot,
    pub(super) previous: EntityListSnapshot,
}

#[derive(Default, Clone, PartialEq)]
pub(super) struct EntityListSnapshot {
    pub(super) familiars: Vec<FamiliarRowViewModel>,
    pub(super) unassigned: Vec<SoulRowViewModel>,
    pub(super) unassigned_folded: bool,
}

#[derive(Clone, PartialEq)]
pub(super) struct FamiliarRowViewModel {
    pub(super) entity: Entity,
    pub(super) label: String,
    pub(super) is_folded: bool,
    pub(super) show_empty: bool,
    pub(super) souls: Vec<SoulRowViewModel>,
}

#[derive(Clone, PartialEq)]
pub(super) struct SoulRowViewModel {
    pub(super) entity: Entity,
    pub(super) name: String,
    pub(super) gender: crate::entities::damned_soul::Gender,
    pub(super) fatigue_text: String,
    pub(super) stress_text: String,
    pub(super) stress_bucket: StressBucket,
    pub(super) task_visual: TaskVisual,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum StressBucket {
    Low,
    Medium,
    High,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum TaskVisual {
    Idle,
    Chop,
    Mine,
    GatherDefault,
    Haul,
    Build,
    HaulToBlueprint,
    Water,
}

#[derive(Resource, Default)]
pub struct EntityListNodeIndex {
    pub(super) familiar_sections: HashMap<Entity, FamiliarSectionNodes>,
    pub(super) unassigned_rows: HashMap<Entity, Entity>,
}

#[derive(Clone, Copy)]
pub(super) struct FamiliarSectionNodes {
    pub(super) root: Entity,
    pub(super) header_text: Entity,
    pub(super) fold_icon: Entity,
    pub(super) members_container: Entity,
}
