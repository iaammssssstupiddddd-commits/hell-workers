use super::dirty::EntityListDirty;
use crate::entities::damned_soul::{DamnedSoul, SoulIdentity};
use crate::entities::familiar::{Familiar, FamiliarOperation};
use crate::interface::ui::components::{SectionFolded, UnassignedFolded};
use crate::relationships::{CommandedBy, Commanding};
use crate::systems::familiar_ai::FamiliarAiState;
use crate::systems::soul_ai::execute::task_execution::AssignedTask;
use bevy::prelude::*;

pub fn detect_entity_list_changes(
    mut dirty: ResMut<EntityListDirty>,
    q_souls: Query<(), Changed<DamnedSoul>>,
    q_added_souls: Query<(), Added<DamnedSoul>>,
    q_tasks: Query<(), Changed<AssignedTask>>,
    q_identity: Query<(), Changed<SoulIdentity>>,
    q_familiars: Query<(), Changed<Familiar>>,
    q_added_familiars: Query<(), Added<Familiar>>,
    q_familiar_ai: Query<(), Changed<FamiliarAiState>>,
    q_familiar_op: Query<(), Changed<FamiliarOperation>>,
    q_commanding: Query<(), Changed<Commanding>>,
    q_commanded_by: Query<(), Changed<CommandedBy>>,
    q_folded: Query<(), Changed<SectionFolded>>,
    q_unassigned_folded: Query<(), Changed<UnassignedFolded>>,
    mut removed_souls: RemovedComponents<DamnedSoul>,
    mut removed_familiars: RemovedComponents<Familiar>,
    mut removed_commanded_by: RemovedComponents<CommandedBy>,
) {
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
