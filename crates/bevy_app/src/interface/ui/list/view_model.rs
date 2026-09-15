use super::{
    EntityListSnapshot, EntityListViewModel, FamiliarRowViewModel, SoulGender, SoulRowViewModel,
};
use crate::entities::damned_soul::{DamnedSoul, Gender, SoulIdentity};
use crate::entities::familiar::{Familiar, FamiliarOperation};
use crate::systems::familiar_ai::FamiliarAiState;
use crate::systems::jobs::WorkType;
use crate::systems::soul_ai::execute::task_execution::AssignedTask;
use bevy::prelude::*;
use hw_core::relationships::{CommandedBy, Commanding};
use hw_ui::components::{SectionFolded, UnassignedFolded, UnassignedSoulSection};
use hw_ui::list::search::EntityListSearchState;

fn matches_search(name: &str, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    name.contains(query)
}

fn filter_soul_rows(mut souls: Vec<SoulRowViewModel>, query: &str) -> Vec<SoulRowViewModel> {
    if query.is_empty() {
        return souls;
    }
    souls.retain(|row| matches_search(&row.name, query));
    souls
}

type FamiliarListQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Familiar,
        &'static FamiliarOperation,
        &'static FamiliarAiState,
        Option<&'static Commanding>,
    ),
>;

type AllSoulsQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static DamnedSoul,
        &'static AssignedTask,
        &'static SoulIdentity,
        Option<&'static CommandedBy>,
    ),
    Without<Familiar>,
>;

use super::{StressBucket, TaskVisual};

pub fn familiar_state_label(ai_state: &FamiliarAiState) -> &'static str {
    match ai_state {
        FamiliarAiState::Idle => "Idle",
        FamiliarAiState::SearchingTask => "Searching",
        FamiliarAiState::Scouting { .. } => "Scouting",
        FamiliarAiState::Supervising { .. } => "Supervising",
    }
}

pub(super) fn familiar_label(
    familiar: &Familiar,
    op: &FamiliarOperation,
    ai_state: &FamiliarAiState,
    squad_count: usize,
) -> String {
    format!(
        "{} ({}/{}) [{}]",
        familiar.name,
        squad_count,
        op.max_controlled_soul,
        familiar_state_label(ai_state)
    )
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
        AssignedTask::MovePlant { .. } => TaskVisual::Move,
        AssignedTask::HaulToBlueprint { .. } => TaskVisual::HaulToBlueprint,
        AssignedTask::CollectBone { .. } => TaskVisual::CollectBone,
        AssignedTask::Refine { .. } => TaskVisual::Refine,
        AssignedTask::HaulToMixer { .. } => TaskVisual::HaulToBlueprint,
        AssignedTask::HaulWithWheelbarrow { .. } => TaskVisual::Haul,
        AssignedTask::ReinforceFloorTile { .. } => TaskVisual::Build,
        AssignedTask::PourFloorTile { .. } => TaskVisual::Build,
        AssignedTask::FrameWallTile { .. } => TaskVisual::Build,
        AssignedTask::CoatWall { .. } => TaskVisual::Build,
        AssignedTask::BucketTransport(_) => TaskVisual::Water,
        AssignedTask::GeneratePower(_) => TaskVisual::GeneratePower,
        AssignedTask::Deconstruct(_) => TaskVisual::Deconstruct,
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

pub(super) fn build_soul_view_model(
    soul_entity: Entity,
    soul: &DamnedSoul,
    task: &AssignedTask,
    identity: &SoulIdentity,
) -> SoulRowViewModel {
    SoulRowViewModel {
        entity: soul_entity,
        name: identity.name.clone(),
        gender: match identity.gender {
            Gender::Male => SoulGender::Male,
            Gender::Female => SoulGender::Female,
        },
        fatigue_text: format!("{:.0}%", soul.fatigue * 100.0),
        stress_text: format!("{:.0}%", soul.stress * 100.0),
        stress_bucket: stress_bucket(soul.stress),
        dream_text: format!("{:.0}", soul.dream),
        dream_empty: soul.dream <= 0.0,
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
    q_all_souls: &AllSoulsQuery<'_, '_>,
) -> FamiliarRowViewModel {
    let squad_count = commanding_opt.map(|c| c.len()).unwrap_or(0);
    let mut souls = Vec::new();
    let mut show_empty = false;

    if !is_folded && let Some(commanding) = commanding_opt {
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

    FamiliarRowViewModel {
        entity: fam_entity,
        label: familiar_label(familiar, op, ai_state, squad_count),
        is_folded,
        show_empty,
        souls,
    }
}

pub fn build_entity_list_view_model_system(
    dirty: Res<super::dirty::EntityListDirty>,
    search_state: Res<EntityListSearchState>,
    mut view_model: ResMut<EntityListViewModel>,
    q_familiars: FamiliarListQuery<'_, '_>,
    q_all_souls: AllSoulsQuery<'_, '_>,
    q_folded: Query<Has<SectionFolded>>,
    unassigned_folded_query: Query<Has<UnassignedFolded>, With<UnassignedSoulSection>>,
) {
    if !dirty.needs_structure_sync() && !dirty.needs_value_sync_only() {
        return;
    }

    view_model.previous = std::mem::take(&mut view_model.current);

    let query = search_state.normalized();
    let searching = !query.is_empty();
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
            is_folded && !searching,
            &q_all_souls,
        ));
    }
    familiars.sort_by_key(|vm| vm.entity.index());

    let mut unassigned = Vec::new();
    if !unassigned_folded || searching {
        for (soul_entity, soul, task, identity, under_command) in q_all_souls.iter() {
            if under_command.is_none() {
                unassigned.push(build_soul_view_model(soul_entity, soul, task, identity));
            }
        }
    }
    unassigned.sort_by_key(|vm| vm.entity.index());

    let familiars = familiars
        .into_iter()
        .map(|mut row| {
            row.souls = filter_soul_rows(row.souls, query);
            if searching {
                row.is_folded = row.souls.is_empty() && q_folded.get(row.entity).unwrap_or(false);
                row.show_empty = false;
            }
            row
        })
        .collect();
    let unassigned = filter_soul_rows(unassigned, query);
    let unassigned_folded = unassigned_folded && (!searching || unassigned.is_empty());

    view_model.current = EntityListSnapshot {
        familiars,
        unassigned,
        unassigned_folded,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn power_and_deconstruction_never_fall_back_to_water() {
        use hw_jobs::tasks::{DeconstructData, GeneratePowerData};
        let power = AssignedTask::GeneratePower(GeneratePowerData {
            tile: Entity::PLACEHOLDER,
            tile_pos: Vec2::ZERO,
            phase: default(),
        });
        let deconstruct = AssignedTask::Deconstruct(DeconstructData {
            order: Entity::PLACEHOLDER,
            target: Entity::PLACEHOLDER,
            phase: default(),
        });
        assert_eq!(task_visual(&power), TaskVisual::GeneratePower);
        assert_eq!(task_visual(&deconstruct), TaskVisual::Deconstruct);
        assert_eq!(task_visual(&power).label(), "発電");
        assert_eq!(task_visual(&deconstruct).label(), "解体");
    }

    #[test]
    fn search_finds_folded_souls_and_restores_original_folds() {
        let mut app = App::new();
        app.init_resource::<super::super::dirty::EntityListDirty>()
            .init_resource::<EntityListSearchState>()
            .init_resource::<EntityListViewModel>()
            .add_systems(Update, build_entity_list_view_model_system);
        let familiar = app
            .world_mut()
            .spawn((
                Familiar::default(),
                FamiliarOperation::default(),
                FamiliarAiState::Idle,
                SectionFolded,
            ))
            .id();
        let section = app
            .world_mut()
            .spawn((UnassignedSoulSection, UnassignedFolded))
            .id();
        let mut souls = Vec::new();
        for assigned in [true, false] {
            let mut entity = app.world_mut().spawn((
                DamnedSoul::default(),
                AssignedTask::None,
                SoulIdentity {
                    name: "検索対象".into(),
                    gender: Gender::Male,
                },
            ));
            if assigned {
                entity.insert(CommandedBy(familiar));
            }
            souls.push(entity.id());
        }
        app.world_mut()
            .resource_mut::<super::super::dirty::EntityListDirty>()
            .mark_structure();
        app.update();
        assert!(
            app.world()
                .resource::<EntityListViewModel>()
                .current
                .familiars[0]
                .souls
                .is_empty()
        );
        for query in [" 対象 ", "存在しない", ""] {
            app.world_mut()
                .resource_mut::<EntityListSearchState>()
                .query = query.into();
            app.update();
            let snapshot = &app.world().resource::<EntityListViewModel>().current;
            if query.trim() == "対象" {
                assert_eq!(snapshot.familiars[0].souls[0].entity, souls[0]);
                assert_eq!(snapshot.unassigned[0].entity, souls[1]);
                assert!(!snapshot.familiars[0].is_folded);
                assert!(!snapshot.unassigned_folded);
            } else {
                assert!(snapshot.familiars[0].souls.is_empty());
                assert!(snapshot.unassigned.is_empty());
                assert!(snapshot.familiars[0].is_folded);
                assert!(snapshot.unassigned_folded);
            }
            assert!(app.world().get::<SectionFolded>(familiar).is_some());
            assert!(app.world().get::<UnassignedFolded>(section).is_some());
        }
    }

    #[test]
    fn familiar_label_reflects_current_roster_and_operation_max() {
        let familiar = Familiar {
            name: "A".to_string(),
            ..default()
        };
        let operation = FamiliarOperation {
            max_controlled_soul: 1,
            ..default()
        };

        assert_eq!(
            familiar_label(&familiar, &operation, &FamiliarAiState::Idle, 3),
            "A (3/1) [Idle]"
        );
    }
}
