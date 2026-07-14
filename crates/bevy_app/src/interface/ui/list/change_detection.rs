use super::dirty::EntityListDirty;
use crate::entities::damned_soul::{DamnedSoul, SoulIdentity};
use crate::entities::familiar::{Familiar, FamiliarOperation};
use crate::systems::familiar_ai::FamiliarAiState;
use crate::systems::soul_ai::execute::task_execution::AssignedTask;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::ecs::drain_removed;
use hw_core::relationships::{CommandedBy, Commanding};
use hw_ui::components::{SectionFolded, UnassignedFolded};
use hw_ui::list::search::EntityListSearchState;

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
    search_state: Res<EntityListSearchState>,
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
    let removed_souls = drain_removed(&mut removed_souls);
    let removed_familiars = drain_removed(&mut removed_familiars);
    let removed_commanded_by = drain_removed(&mut removed_commanded_by);
    let structure_changed = !q_added_souls.is_empty()
        || !q_added_familiars.is_empty()
        || !q_commanding.is_empty()
        || !q_commanded_by.is_empty()
        || !q_folded.is_empty()
        || !q_unassigned_folded.is_empty()
        || removed_souls
        || removed_familiars
        || removed_commanded_by;

    if structure_changed {
        dirty.mark_structure();
    }

    let search_active = !search_state.normalized().is_empty();
    if search_active && !q_identity.is_empty() {
        dirty.mark_structure();
    }

    let value_changed = !q_souls.is_empty()
        || !q_tasks.is_empty()
        || (!q_identity.is_empty() && !search_active)
        || !q_familiars.is_empty()
        || !q_familiar_ai.is_empty()
        || !q_familiar_op.is_empty();

    if value_changed {
        dirty.mark_values();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::minimal_app;

    #[test]
    fn consumes_all_structure_removal_readers_in_one_update() {
        let mut app = minimal_app();
        app.init_resource::<EntityListDirty>();
        app.init_resource::<EntityListSearchState>();
        app.add_systems(Update, detect_entity_list_changes);
        app.update();

        let commander = app.world_mut().spawn_empty().id();
        let soul = app.world_mut().spawn(DamnedSoul::default()).id();
        let familiar = app.world_mut().spawn(Familiar::default()).id();
        let commanded = app.world_mut().spawn(CommandedBy(commander)).id();
        app.update();
        app.world_mut()
            .resource_mut::<EntityListDirty>()
            .clear_all();

        app.world_mut().entity_mut(soul).remove::<DamnedSoul>();
        app.world_mut().entity_mut(familiar).remove::<Familiar>();
        app.world_mut()
            .entity_mut(commanded)
            .remove::<CommandedBy>();
        app.update();
        assert!(
            app.world()
                .resource::<EntityListDirty>()
                .needs_structure_sync()
        );

        app.world_mut()
            .resource_mut::<EntityListDirty>()
            .clear_all();
        app.update();
        assert!(
            !app.world()
                .resource::<EntityListDirty>()
                .needs_structure_sync()
        );
    }
}
