use super::geometry::in_selection_area;
use super::queries::DesignationTargetQuery;
use super::state::AreaEditHistory;
use crate::entities::damned_soul::Destination;
use crate::entities::familiar::{ActiveCommand, Familiar, FamiliarCommand};
use crate::relationships::ManagedBy;
use crate::systems::command::{TaskArea, TaskMode};
use crate::systems::jobs::{Designation, Priority, TaskSlots, WorkType};
use super::cancel::cancel_single_designation;
use super::manual_haul::{find_manual_request_for_source, pick_manual_haul_stockpile_anchor};
use crate::systems::logistics::transport_request::{
    ManualHaulPinnedSource, ManualTransportRequest, TransportDemand, TransportPolicy,
    TransportPriority, TransportRequest, TransportRequestFixedSource, TransportRequestKind,
    TransportRequestState,
};
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

/// エリア適用 + 履歴記録。input.rs と shortcuts.rs で共有。
pub(super) fn apply_area_and_record_history(
    familiar_entity: Entity,
    new_area: &TaskArea,
    before: Option<TaskArea>,
    commands: &mut Commands,
    q_familiars: &mut Query<(&mut ActiveCommand, &mut Destination), With<Familiar>>,
    area_edit_history: &mut AreaEditHistory,
) {
    apply_task_area_to_familiar(familiar_entity, Some(new_area), commands, q_familiars);
    area_edit_history.push(familiar_entity, before, Some(new_area.clone()));
}

pub(super) fn apply_designation_in_area(
    commands: &mut Commands,
    mode: TaskMode,
    area: &TaskArea,
    issued_by: Option<Entity>,
    q_targets: &DesignationTargetQuery,
) {
    let work_type = match mode {
        TaskMode::DesignateChop(_) => Some(WorkType::Chop),
        TaskMode::DesignateMine(_) => Some(WorkType::Mine),
        TaskMode::DesignateHaul(_) => Some(WorkType::Haul),
        _ => None,
    };

    for (
        target_entity,
        transform,
        tree,
        rock,
        item,
        designation,
        task_workers,
        blueprint,
        belongs_to,
        transport_request,
        fixed_source,
        _stockpile,
        _stored_items,
        _bucket_storage,
        _manual_request,
    ) in q_targets.iter()
    {
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

            if wt == WorkType::Haul {
                let Some(issuer) = issued_by else {
                    warn!(
                        "MANUAL_HAUL: Skipped source {:?} because no familiar is selected",
                        target_entity
                    );
                    continue;
                };
                let Some(item_type) = item.map(|it| it.0) else {
                    continue;
                };

                let item_owner = belongs_to.map(|belongs| belongs.0);
                let Some(anchor_stockpile) =
                    pick_manual_haul_stockpile_anchor(pos, item_type, item_owner, q_targets)
                else {
                    debug!(
                        "MANUAL_HAUL: No stockpile anchor found for source {:?} ({:?})",
                        target_entity, item_type
                    );
                    continue;
                };

                if designation.is_some()
                    && transport_request.is_none()
                    && designation.is_some_and(|d| d.work_type == WorkType::Haul)
                {
                    commands
                        .entity(target_entity)
                        .remove::<Designation>()
                        .remove::<TaskSlots>()
                        .remove::<ManagedBy>()
                        .remove::<Priority>();
                }

                commands
                    .entity(target_entity)
                    .insert(ManualHaulPinnedSource);

                let request_entity = find_manual_request_for_source(target_entity, q_targets);
                let mut request_cmd = if let Some(existing) = request_entity {
                    commands.entity(existing)
                } else {
                    commands.spawn_empty()
                };

                request_cmd.insert((
                    Name::new("TransportRequest::ManualDesignateHaul"),
                    Transform::from_xyz(pos.x, pos.y, 0.0),
                    Visibility::Inherited,
                    Designation {
                        work_type: WorkType::Haul,
                    },
                    ManagedBy(issuer),
                    TaskSlots::new(1),
                    Priority(0),
                    TransportRequest {
                        kind: TransportRequestKind::DepositToStockpile,
                        anchor: anchor_stockpile,
                        resource_type: item_type,
                        issued_by: issuer,
                        priority: TransportPriority::Normal,
                        stockpile_group: vec![],
                    },
                    TransportDemand {
                        desired_slots: 1,
                        inflight: 0,
                    },
                    TransportRequestState::Pending,
                    TransportPolicy::default(),
                    ManualTransportRequest,
                    TransportRequestFixedSource(target_entity),
                ));

                continue;
            }

            if let Some(issuer) = issued_by {
                commands.entity(target_entity).insert((
                    Designation { work_type: wt },
                    ManagedBy(issuer),
                    TaskSlots::new(1),
                    Priority(0),
                ));
            } else {
                commands.entity(target_entity).insert((
                    Designation { work_type: wt },
                    TaskSlots::new(1),
                    Priority(0),
                ));
            }
            continue;
        }

        if designation.is_some() {
            cancel_single_designation(
                commands,
                target_entity,
                task_workers,
                blueprint.is_some(),
                transport_request.is_some(),
                fixed_source.map(|source| source.0),
            );
        }
    }
}
