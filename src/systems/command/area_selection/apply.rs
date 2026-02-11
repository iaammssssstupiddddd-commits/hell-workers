use super::geometry::in_selection_area;
use crate::entities::damned_soul::Destination;
use crate::entities::familiar::{ActiveCommand, Familiar, FamiliarCommand};
use crate::events::OnTaskAbandoned;
use crate::relationships::{ManagedBy, TaskWorkers, WorkingOn};
use crate::systems::command::{TaskArea, TaskMode};
use crate::systems::jobs::{Blueprint, Designation, Priority, Rock, TaskSlots, Tree, WorkType};
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

pub(super) fn cancel_single_designation(
    commands: &mut Commands,
    target_entity: Entity,
    task_workers: Option<&TaskWorkers>,
    is_blueprint: bool,
) {
    // 作業者への通知
    if let Some(workers) = task_workers {
        for &soul in workers.iter() {
            commands.entity(soul).remove::<WorkingOn>();
            commands.trigger(OnTaskAbandoned { entity: soul });
        }
    }

    if is_blueprint {
        // Blueprint はエンティティごと despawn する
        // WorldMap のクリーンアップは blueprint_cancel_cleanup_system が担当
        commands.entity(target_entity).despawn();
    } else {
        commands.entity(target_entity).remove::<Designation>();
        commands.entity(target_entity).remove::<TaskSlots>();
        commands.entity(target_entity).remove::<ManagedBy>();
    }
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
        Option<&Designation>,
        Option<&TaskWorkers>,
        Option<&Blueprint>,
    )>,
) {
    let work_type = match mode {
        TaskMode::DesignateChop(_) => Some(WorkType::Chop),
        TaskMode::DesignateMine(_) => Some(WorkType::Mine),
        TaskMode::DesignateHaul(_) => Some(WorkType::Haul),
        _ => None,
    };

    for (target_entity, transform, tree, rock, item, designation, task_workers, blueprint) in
        q_targets.iter()
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

        // キャンセルモード: Designation持ちのみキャンセル
        if designation.is_some() {
            cancel_single_designation(
                commands,
                target_entity,
                task_workers,
                blueprint.is_some(),
            );
        }
    }
}

/// Blueprint が despawn された時に WorldMap と PendingBelongsToBlueprint を掃除する
pub fn blueprint_cancel_cleanup_system(
    mut commands: Commands,
    mut world_map: ResMut<crate::world::map::WorldMap>,
    mut removed: RemovedComponents<Blueprint>,
    q_pending: Query<(Entity, &crate::systems::logistics::PendingBelongsToBlueprint)>,
) {
    for removed_entity in removed.read() {
        // WorldMap.buildings からこの Blueprint が占有していたグリッドを除去
        let grids_to_remove: Vec<(i32, i32)> = world_map
            .buildings
            .iter()
            .filter(|&(_, entity)| *entity == removed_entity)
            .map(|(&grid, _)| grid)
            .collect();
        for (gx, gy) in grids_to_remove {
            world_map.buildings.remove(&(gx, gy));
            world_map.remove_obstacle(gx, gy);
            info!(
                "BLUEPRINT_CANCEL: Cleaned up building grid ({}, {}) for {:?}",
                gx, gy, removed_entity
            );
        }

        // PendingBelongsToBlueprint のコンパニオンエンティティを除去
        for (companion_entity, pending) in q_pending.iter() {
            if pending.0 == removed_entity {
                // コンパニオンも Blueprint なので despawn すれば次フレームでこのシステムが再度クリーンアップ
                commands.entity(companion_entity).despawn();
                info!(
                    "BLUEPRINT_CANCEL: Despawned companion {:?} for {:?}",
                    companion_entity, removed_entity
                );
            }
        }
    }
}
