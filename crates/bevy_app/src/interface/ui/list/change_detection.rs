use super::dirty::EntityListDirty;
use crate::entities::damned_soul::{DamnedSoul, SoulIdentity};
use crate::entities::familiar::{Familiar, FamiliarOperation};
use crate::systems::familiar_ai::FamiliarAiState;
use crate::systems::soul_ai::execute::task_execution::AssignedTask;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::relationships::{CommandedBy, Commanding};
use hw_ui::components::{SectionFolded, UnassignedFolded};

#[derive(SystemParam)]
pub struct StructureDetectors<'w, 's> {
    q_added_souls: Query<'w, 's, (), Added<DamnedSoul>>,
    q_added_familiars: Query<'w, 's, (), Added<Familiar>>,
    q_commanding: Query<'w, 's, (), Changed<Commanding>>,
    q_commanded_by: Query<'w, 's, (), Changed<CommandedBy>>,
    q_folded: Query<'w, 's, (), Changed<SectionFolded>>,
    q_unassigned_folded: Query<'w, 's, (), Changed<UnassignedFolded>>,
    removed_souls: RemovedComponents<'w, 's, DamnedSoul>,
    removed_familiars: RemovedComponents<'w, 's, Familiar>,
    removed_commanded_by: RemovedComponents<'w, 's, CommandedBy>,
}

#[derive(SystemParam)]
pub struct ValueDetectors<'w, 's> {
    q_souls: Query<'w, 's, (), Changed<DamnedSoul>>,
    q_tasks: Query<'w, 's, (), Changed<AssignedTask>>,
    q_identity: Query<'w, 's, (), Changed<SoulIdentity>>,
    q_familiars: Query<'w, 's, (), Changed<Familiar>>,
    q_familiar_ai: Query<'w, 's, (), Changed<FamiliarAiState>>,
    q_familiar_op: Query<'w, 's, (), Changed<FamiliarOperation>>,
}

pub fn detect_entity_list_changes(
    mut dirty: ResMut<EntityListDirty>,
    structure: StructureDetectors,
    value: ValueDetectors,
) {
    let StructureDetectors {
        q_added_souls,
        q_added_familiars,
        q_commanding,
        q_commanded_by,
        q_folded,
        q_unassigned_folded,
        mut removed_souls,
        mut removed_familiars,
        mut removed_commanded_by,
    } = structure;
    let ValueDetectors {
        q_souls,
        q_tasks,
        q_identity,
        q_familiars,
        q_familiar_ai,
        q_familiar_op,
    } = value;
    let structure_changed = !q_added_souls.is_empty()
        || !q_added_familiars.is_empty()
        || !q_commanding.is_empty()
        || !q_commanded_by.is_empty()
        || !q_folded.is_empty()
        || !q_unassigned_folded.is_empty()
        || removed_souls.read().next().is_some()
        || removed_familiars.read().next().is_some()
        || removed_commanded_by.read().next().is_some();

    if structure_changed {
        dirty.mark_structure();
    }

    let value_changed = !q_souls.is_empty()
        || !q_tasks.is_empty()
        || !q_identity.is_empty()
        || !q_familiars.is_empty()
        || !q_familiar_ai.is_empty()
        || !q_familiar_op.is_empty();

    if value_changed {
        dirty.mark_values();
    }
}
