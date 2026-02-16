use super::geometry::in_selection_area;
use crate::entities::damned_soul::Destination;
use crate::entities::familiar::{ActiveCommand, Familiar, FamiliarCommand};
use crate::events::OnTaskAbandoned;
use crate::relationships::{ManagedBy, StoredItems, TaskWorkers, WorkingOn};
use crate::systems::command::{TaskArea, TaskMode};
use crate::systems::jobs::{Blueprint, Designation, Priority, Rock, TaskSlots, Tree, WorkType};
use crate::systems::logistics::transport_request::{
    ManualHaulPinnedSource, ManualTransportRequest, TransportDemand, TransportPolicy,
    TransportPriority, TransportRequest, TransportRequestFixedSource, TransportRequestKind,
    TransportRequestState,
};
use crate::systems::logistics::{BelongsTo, BucketStorage, ResourceItem, ResourceType, Stockpile};
use bevy::prelude::*;

fn pick_manual_haul_stockpile_anchor(
    source_pos: Vec2,
    resource_type: ResourceType,
    item_owner: Option<Entity>,
    q_targets: &Query<(
        Entity,
        &Transform,
        Option<&Tree>,
        Option<&Rock>,
        Option<&ResourceItem>,
        Option<&Designation>,
        Option<&TaskWorkers>,
        Option<&Blueprint>,
        Option<&BelongsTo>,
        Option<&TransportRequest>,
        Option<&TransportRequestFixedSource>,
        Option<&Stockpile>,
        Option<&StoredItems>,
        Option<&BucketStorage>,
        Option<&ManualTransportRequest>,
    )>,
) -> Option<Entity> {
    let is_bucket = matches!(
        resource_type,
        ResourceType::BucketEmpty | ResourceType::BucketWater
    );

    let mut best_with_capacity: Option<(Entity, f32)> = None;
    let mut best_any_capacity: Option<(Entity, f32)> = None;

    for (
        stock_entity,
        stock_transform,
        _,
        _,
        _,
        _,
        _,
        _,
        stock_owner_opt,
        _,
        _,
        stockpile_opt,
        stored_opt,
        bucket_opt,
        _,
    ) in q_targets.iter()
    {
        let Some(stockpile) = stockpile_opt else {
            continue;
        };
        let stock_owner = stock_owner_opt.map(|belongs| belongs.0);
        if stock_owner != item_owner {
            continue;
        }

        let is_bucket_storage = bucket_opt.is_some();
        if is_bucket_storage && !is_bucket {
            continue;
        }

        let is_dedicated = stock_owner.is_some();
        let type_match = if is_dedicated && is_bucket {
            true
        } else {
            stockpile.resource_type.is_none() || stockpile.resource_type == Some(resource_type)
        };
        if !type_match {
            continue;
        }

        let dist_sq = stock_transform
            .translation
            .truncate()
            .distance_squared(source_pos);
        match best_any_capacity {
            Some((_, best_dist_sq)) if best_dist_sq <= dist_sq => {}
            _ => best_any_capacity = Some((stock_entity, dist_sq)),
        }

        let current = stored_opt.map(|stored| stored.len()).unwrap_or(0);
        if current >= stockpile.capacity {
            continue;
        }
        match best_with_capacity {
            Some((_, best_dist_sq)) if best_dist_sq <= dist_sq => {}
            _ => best_with_capacity = Some((stock_entity, dist_sq)),
        }
    }

    best_with_capacity
        .or(best_any_capacity)
        .map(|(entity, _)| entity)
}

fn find_manual_request_for_source(
    source_entity: Entity,
    q_targets: &Query<(
        Entity,
        &Transform,
        Option<&Tree>,
        Option<&Rock>,
        Option<&ResourceItem>,
        Option<&Designation>,
        Option<&TaskWorkers>,
        Option<&Blueprint>,
        Option<&BelongsTo>,
        Option<&TransportRequest>,
        Option<&TransportRequestFixedSource>,
        Option<&Stockpile>,
        Option<&StoredItems>,
        Option<&BucketStorage>,
        Option<&ManualTransportRequest>,
    )>,
) -> Option<Entity> {
    q_targets.iter().find_map(
        |(
            request_entity,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            transport_request_opt,
            fixed_source_opt,
            _,
            _,
            _,
            manual_opt,
        )| {
            (manual_opt.is_some()
                && transport_request_opt.is_some()
                && fixed_source_opt.map(|source| source.0) == Some(source_entity))
            .then_some(request_entity)
        },
    )
}

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
    is_transport_request: bool,
    fixed_source: Option<Entity>,
) {
    fn trigger_task_abandoned_if_alive(commands: &mut Commands, soul: Entity) {
        commands.queue(move |world: &mut World| {
            if world.get_entity(soul).is_ok() {
                world.trigger(OnTaskAbandoned { entity: soul });
            }
        });
    }

    // 作業者への通知
    if let Some(workers) = task_workers {
        for &soul in workers.iter() {
            commands.entity(soul).try_remove::<WorkingOn>();
            trigger_task_abandoned_if_alive(commands, soul);
        }
    }

    if let Some(source_entity) = fixed_source {
        commands
            .entity(source_entity)
            .try_remove::<ManualHaulPinnedSource>();
    }

    if is_blueprint || is_transport_request {
        // Blueprint はエンティティごと despawn する
        // WorldMap のクリーンアップは blueprint_cancel_cleanup_system が担当
        commands.entity(target_entity).try_despawn();
    } else {
        commands
            .entity(target_entity)
            .try_remove::<(Designation, TaskSlots, ManagedBy)>();
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
        Option<&BelongsTo>,
        Option<&TransportRequest>,
        Option<&TransportRequestFixedSource>,
        Option<&Stockpile>,
        Option<&StoredItems>,
        Option<&BucketStorage>,
        Option<&ManualTransportRequest>,
    )>,
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

                info!(
                    "MANUAL_HAUL: Upserted request for source {:?} -> stockpile {:?}",
                    target_entity, anchor_stockpile
                );
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
                transport_request.is_some(),
                fixed_source.map(|source| source.0),
            );
        }
    }
}

/// Blueprint が despawn された時に WorldMap と PendingBelongsToBlueprint を掃除する
pub fn blueprint_cancel_cleanup_system(
    mut commands: Commands,
    mut world_map: ResMut<crate::world::map::WorldMap>,
    mut removed: RemovedComponents<Blueprint>,
    q_pending: Query<(
        Entity,
        &crate::systems::logistics::PendingBelongsToBlueprint,
    )>,
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
                commands.entity(companion_entity).try_despawn();
                info!(
                    "BLUEPRINT_CANCEL: Despawned companion {:?} for {:?}",
                    companion_entity, removed_entity
                );
            }
        }
    }
}
