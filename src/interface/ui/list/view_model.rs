use super::{EntityListSnapshot, EntityListViewModel, FamiliarRowViewModel, SoulRowViewModel};
use crate::entities::damned_soul::{DamnedSoul, SoulIdentity};
use crate::entities::familiar::{Familiar, FamiliarOperation};
use crate::interface::ui::components::{SectionFolded, UnassignedFolded, UnassignedSoulSection};
use crate::relationships::{CommandedBy, Commanding};
use crate::systems::familiar_ai::FamiliarAiState;
use crate::systems::jobs::WorkType;
use crate::systems::soul_ai::execute::task_execution::AssignedTask;
use bevy::prelude::*;

use super::{StressBucket, TaskVisual};

pub fn familiar_state_label(ai_state: &FamiliarAiState) -> &'static str {
    match ai_state {
        FamiliarAiState::Idle => "Idle",
        FamiliarAiState::SearchingTask => "Searching",
        FamiliarAiState::Scouting { .. } => "Scouting",
        FamiliarAiState::Supervising { .. } => "Supervising",
    }
}

fn task_visual(task: &AssignedTask) -> TaskVisual {
    match task {
        AssignedTask::None => TaskVisual::Idle,
        AssignedTask::Gather(data) => match data.work_type {
            WorkType::Chop => TaskVisual::Chop,
            WorkType::Mine => TaskVisual::Mine,
            _ => TaskVisual::GatherDefault,
        },
        AssignedTask::Haul { .. } => TaskVisual::Haul,
        AssignedTask::Build { .. } => TaskVisual::Build,
        AssignedTask::HaulToBlueprint { .. } => TaskVisual::HaulToBlueprint,
        AssignedTask::GatherWater { .. } => TaskVisual::Water,
        AssignedTask::CollectSand { .. } => TaskVisual::GatherDefault,
        AssignedTask::CollectBone { .. } => TaskVisual::GatherDefault,
        AssignedTask::Refine { .. } => TaskVisual::Build,
        AssignedTask::HaulToMixer { .. } => TaskVisual::HaulToBlueprint,
        AssignedTask::HaulWaterToMixer { .. } => TaskVisual::Water,
        AssignedTask::HaulWithWheelbarrow { .. } => TaskVisual::Haul,
        AssignedTask::ReinforceFloorTile { .. } => TaskVisual::Build,
        AssignedTask::PourFloorTile { .. } => TaskVisual::Build,
        AssignedTask::CoatWall { .. } => TaskVisual::Build,
    }
}

fn stress_bucket(stress: f32) -> StressBucket {
    if stress > 0.8 {
        StressBucket::High
    } else if stress > 0.5 {
        StressBucket::Medium
    } else {
        StressBucket::Low
    }
}

fn build_soul_view_model(
    soul_entity: Entity,
    soul: &DamnedSoul,
    task: &AssignedTask,
    identity: &SoulIdentity,
) -> SoulRowViewModel {
    SoulRowViewModel {
        entity: soul_entity,
        name: identity.name.clone(),
        gender: identity.gender,
        fatigue_text: format!("{:.0}%", soul.fatigue * 100.0),
        stress_text: format!("{:.0}%", soul.stress * 100.0),
        stress_bucket: stress_bucket(soul.stress),
        task_visual: task_visual(task),
    }
}

fn build_familiar_row_view_model(
    fam_entity: Entity,
    familiar: &Familiar,
    op: &FamiliarOperation,
    ai_state: &FamiliarAiState,
    commanding_opt: Option<&Commanding>,
    is_folded: bool,
    q_all_souls: &Query<
        (
            Entity,
            &DamnedSoul,
            &AssignedTask,
            &SoulIdentity,
            Option<&CommandedBy>,
        ),
        Without<Familiar>,
    >,
) -> FamiliarRowViewModel {
    let squad_count = commanding_opt.map(|c| c.len()).unwrap_or(0);
    let mut souls = Vec::new();
    let mut show_empty = false;

    if !is_folded {
        if let Some(commanding) = commanding_opt {
            if commanding.is_empty() {
                show_empty = true;
            } else {
                for &soul_entity in commanding.iter() {
                    if let Ok((_, soul, task, identity, _)) = q_all_souls.get(soul_entity) {
                        souls.push(build_soul_view_model(soul_entity, soul, task, identity));
                    }
                }
                souls.sort_by_key(|vm| vm.entity.index());
            }
        }
    }

    FamiliarRowViewModel {
        entity: fam_entity,
        label: format!(
            "{} ({}/{}) [{}]",
            familiar.name,
            squad_count,
            op.max_controlled_soul,
            familiar_state_label(ai_state)
        ),
        is_folded,
        show_empty,
        souls,
    }
}

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
        familiars.push(build_familiar_row_view_model(
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
                unassigned.push(build_soul_view_model(soul_entity, soul, task, identity));
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
