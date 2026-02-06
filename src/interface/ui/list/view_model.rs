use super::{EntityListSnapshot, EntityListViewModel};
use crate::entities::damned_soul::{DamnedSoul, SoulIdentity};
use crate::entities::familiar::{Familiar, FamiliarOperation};
use crate::interface::ui::components::{SectionFolded, UnassignedFolded, UnassignedSoulSection};
use crate::relationships::{CommandedBy, Commanding};
use crate::systems::familiar_ai::FamiliarAiState;
use crate::systems::soul_ai::task_execution::AssignedTask;
use bevy::prelude::*;

pub fn build_entity_list_view_model_system(
    mut view_model: ResMut<EntityListViewModel>,
    q_familiars: Query<(
        Entity,
        &Familiar,
        &FamiliarOperation,
        &FamiliarAiState,
        Option<&Commanding>,
    )>,
    q_all_souls: Query<
        (
            Entity,
            &DamnedSoul,
            &AssignedTask,
            &SoulIdentity,
            Option<&CommandedBy>,
        ),
        Without<Familiar>,
    >,
    q_folded: Query<Has<SectionFolded>>,
    unassigned_folded_query: Query<Has<UnassignedFolded>, With<UnassignedSoulSection>>,
) {
    view_model.previous = std::mem::take(&mut view_model.current);

    let unassigned_folded = unassigned_folded_query.iter().next().unwrap_or(false);
    let mut familiars = Vec::new();

    for (fam_entity, familiar, op, ai_state, commanding_opt) in q_familiars.iter() {
        let is_folded = q_folded.get(fam_entity).unwrap_or(false);
        familiars.push(super::helpers::build_familiar_row_view_model(
            fam_entity,
            familiar,
            op,
            ai_state,
            commanding_opt,
            is_folded,
            &q_all_souls,
        ));
    }
    familiars.sort_by_key(|vm| vm.entity.index());

    let mut unassigned = Vec::new();
    if !unassigned_folded {
        for (soul_entity, soul, task, identity, under_command) in q_all_souls.iter() {
            if under_command.is_none() {
                unassigned.push(super::helpers::build_soul_view_model(
                    soul_entity,
                    soul,
                    task,
                    identity,
                ));
            }
        }
    }
    unassigned.sort_by_key(|vm| vm.entity.index());

    view_model.current = EntityListSnapshot {
        familiars,
        unassigned,
        unassigned_folded,
    };
}
