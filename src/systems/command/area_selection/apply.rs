use super::geometry::in_selection_area;
use crate::entities::damned_soul::Destination;
use crate::entities::familiar::{ActiveCommand, Familiar, FamiliarCommand};
use crate::relationships::ManagedBy;
use crate::systems::command::{TaskArea, TaskMode};
use crate::systems::jobs::{Designation, Priority, Rock, TaskSlots, Tree, WorkType};
use crate::systems::logistics::ResourceItem;
use bevy::prelude::*;

pub(super) fn apply_task_area_to_familiar(
    familiar_entity: Entity,
    area: Option<&TaskArea>,
    commands: &mut Commands,
    q_familiars: &mut Query<(&mut ActiveCommand, &mut Destination), With<Familiar>>,
) {
    if let Some(area) = area {
        commands.entity(familiar_entity).insert(area.clone());
        if let Ok((mut active_command, mut familiar_dest)) = q_familiars.get_mut(familiar_entity) {
            familiar_dest.0 = area.center();
            active_command.command = FamiliarCommand::Patrol;
        }
    } else {
        commands.entity(familiar_entity).remove::<TaskArea>();
        if let Ok((mut active_command, _)) = q_familiars.get_mut(familiar_entity) {
            active_command.command = FamiliarCommand::Idle;
        }
    }
}

pub(super) fn assign_unassigned_tasks_in_area(
    commands: &mut Commands,
    familiar_entity: Entity,
    area: &TaskArea,
    q_unassigned: &Query<(Entity, &Transform, &Designation), Without<ManagedBy>>,
) -> usize {
    let mut assigned_count = 0;

    for (task_entity, task_transform, _) in q_unassigned.iter() {
        let pos = task_transform.translation.truncate();
        if !in_selection_area(area, pos) {
            continue;
        }

        commands
            .entity(task_entity)
            .insert((ManagedBy(familiar_entity), Priority(0)));
        assigned_count += 1;
    }

    assigned_count
}

pub(super) fn apply_designation_in_area(
    commands: &mut Commands,
    mode: TaskMode,
    area: &TaskArea,
    issued_by: Option<Entity>,
    q_targets: &Query<(
        Entity,
        &Transform,
        Option<&Tree>,
        Option<&Rock>,
        Option<&ResourceItem>,
    )>,
) {
    let work_type = match mode {
        TaskMode::DesignateChop(_) => Some(WorkType::Chop),
        TaskMode::DesignateMine(_) => Some(WorkType::Mine),
        TaskMode::DesignateHaul(_) => Some(WorkType::Haul),
        _ => None,
    };

    for (target_entity, transform, tree, rock, item) in q_targets.iter() {
        let pos = transform.translation.truncate();
        if !in_selection_area(area, pos) {
            continue;
        }

        if let Some(wt) = work_type {
            let match_found = match wt {
                WorkType::Chop => tree.is_some(),
                WorkType::Mine => rock.is_some(),
                WorkType::Haul => item.is_some(),
                _ => false,
            };
            if !match_found {
                continue;
            }

            if let Some(issuer) = issued_by {
                commands.entity(target_entity).insert((
                    Designation { work_type: wt },
                    ManagedBy(issuer),
                    TaskSlots::new(1),
                    Priority(0),
                ));
                info!(
                    "DESIGNATION: Created {:?} for {:?} (assigned to {:?})",
                    wt, target_entity, issuer
                );
            } else {
                commands.entity(target_entity).insert((
                    Designation { work_type: wt },
                    TaskSlots::new(1),
                    Priority(0),
                ));
                info!(
                    "DESIGNATION: Created {:?} for {:?} (unassigned)",
                    wt, target_entity
                );
            }
            continue;
        }

        commands.entity(target_entity).remove::<Designation>();
        commands.entity(target_entity).remove::<TaskSlots>();
        commands.entity(target_entity).remove::<ManagedBy>();
    }
}
