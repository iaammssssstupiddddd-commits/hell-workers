use super::AreaEditHistory;
use super::cancel::cancel_single_designation;
use super::geometry::in_selection_area;
use super::manual_haul::{find_manual_request_for_source, pick_manual_haul_stockpile_anchor};
use super::queries::DesignationTargetQuery;
use crate::entities::damned_soul::Destination;
use crate::entities::familiar::{ActiveCommand, Familiar, FamiliarCommand};
use crate::systems::command::{TaskArea, TaskMode};
use crate::systems::jobs::{Designation, PlayerIssuedDesignation, Priority, TaskSlots, WorkType};
use crate::systems::logistics::transport_request::{
    ManualHaulPinnedSource, ManualTransportRequest, TransportDemand, TransportPolicy,
    TransportPriority, TransportRequest, TransportRequestFixedSource, TransportRequestKind,
    TransportRequestState,
};
use bevy::prelude::*;
use hw_core::relationships::ManagedBy;
use hw_familiar_ai::AutoGatherDesignation;
use hw_world::zones::Site;
use std::collections::HashMap;

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
    q_sites: &Query<&Site>,
) {
    let clamped_area = super::geometry::clamp_area_to_site(new_area, q_sites);
    apply_task_area_to_familiar(familiar_entity, Some(&clamped_area), commands, q_familiars);
    area_edit_history.push(familiar_entity, before, Some(clamped_area));
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

    let mut manual_destination_shadow =
        HashMap::<Entity, HashMap<hw_logistics::ResourceType, usize>>::new();

    // Query/archetype order is not a gameplay contract. Manual haul consumes a shared capacity
    // shadow, so establish a durable source order before the first reservation is made.
    let mut ordered_targets: Vec<(Entity, Vec2)> = q_targets
        .iter()
        .filter_map(|(entity, transform, ..)| {
            let pos = transform.translation.truncate();
            in_selection_area(area, pos).then_some((entity, pos))
        })
        .collect();
    ordered_targets.sort_unstable_by(compare_designation_target_order);

    for (target_entity, _) in ordered_targets {
        let Ok((
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
            _stockpile_runtime,
        )) = q_targets.get(target_entity)
        else {
            continue;
        };
        let pos = transform.translation.truncate();

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
                let Some(anchor_stockpile) = pick_manual_haul_stockpile_anchor(
                    pos,
                    item_type,
                    item_owner,
                    &manual_destination_shadow,
                    q_targets,
                ) else {
                    debug!(
                        "MANUAL_HAUL: No stockpile anchor found for source {:?} ({:?})",
                        target_entity, item_type
                    );
                    continue;
                };
                manual_destination_shadow
                    .entry(anchor_stockpile)
                    .or_default()
                    .entry(item_type)
                    .and_modify(|amount| *amount = amount.saturating_add(1))
                    .or_insert(1);

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
                commands
                    .entity(target_entity)
                    .remove::<AutoGatherDesignation>()
                    .insert((
                        Designation { work_type: wt },
                        PlayerIssuedDesignation,
                        ManagedBy(issuer),
                        TaskSlots::new(1),
                        Priority(0),
                    ));
            } else {
                commands
                    .entity(target_entity)
                    .remove::<AutoGatherDesignation>()
                    .remove::<ManagedBy>()
                    .insert((
                        Designation { work_type: wt },
                        PlayerIssuedDesignation,
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

fn compare_designation_target_order(
    (left_entity, left_pos): &(Entity, Vec2),
    (right_entity, right_pos): &(Entity, Vec2),
) -> std::cmp::Ordering {
    left_pos
        .x
        .total_cmp(&right_pos.x)
        .then_with(|| left_pos.y.total_cmp(&right_pos.y))
        .then_with(|| left_entity.index_u32().cmp(&right_entity.index_u32()))
        .then_with(|| {
            left_entity
                .generation()
                .to_bits()
                .cmp(&right_entity.generation().to_bits())
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_logistics::{StockpileAcceptance, StockpilePolicy};

    #[derive(Resource)]
    struct ManualHaulApplyFixture {
        issuer: Entity,
        area: TaskArea,
    }

    fn apply_manual_haul_fixture(
        mut commands: Commands,
        fixture: Res<ManualHaulApplyFixture>,
        q_targets: DesignationTargetQuery,
    ) {
        apply_designation_in_area(
            &mut commands,
            TaskMode::DesignateHaul(None),
            &fixture.area,
            Some(fixture.issuer),
            &q_targets,
        );
    }

    fn entity(index: u32) -> Entity {
        Entity::from_raw_u32(index).expect("valid entity")
    }

    #[test]
    fn designation_targets_use_position_then_entity_instead_of_query_order() {
        let mut targets = vec![
            (entity(9), Vec2::new(1.0, 0.0)),
            (entity(4), Vec2::new(0.0, 1.0)),
            (entity(7), Vec2::new(0.0, 1.0)),
            (entity(2), Vec2::new(-1.0, 3.0)),
        ];

        targets.sort_unstable_by(compare_designation_target_order);

        assert_eq!(
            targets
                .into_iter()
                .map(|(entity, _)| entity)
                .collect::<Vec<_>>(),
            vec![entity(2), entity(4), entity(7), entity(9)]
        );
    }

    #[test]
    fn manual_area_apply_pins_source_to_a_policy_eligible_stockpile() {
        let mut app = App::new();
        let issuer = app.world_mut().spawn_empty().id();
        let source = app
            .world_mut()
            .spawn((
                Transform::default(),
                Visibility::Visible,
                hw_logistics::ResourceItem(hw_logistics::ResourceType::Wood),
            ))
            .id();
        let rejected = app
            .world_mut()
            .spawn((
                Transform::from_xyz(0.25, 0.0, 0.0),
                hw_logistics::Stockpile {
                    capacity: 2,
                    resource_type: None,
                },
                StockpilePolicy {
                    acceptance: StockpileAcceptance::Only(hw_logistics::ResourceType::Rock),
                    inbound_priority: TransportPriority::Critical,
                    target_amount: 2,
                    allow_export: true,
                },
                hw_logistics::BelongsTo(issuer),
            ))
            .id();
        let accepted = app
            .world_mut()
            .spawn((
                Transform::from_xyz(3.0, 0.0, 0.0),
                hw_logistics::Stockpile {
                    capacity: 2,
                    resource_type: None,
                },
                StockpilePolicy {
                    acceptance: StockpileAcceptance::Only(hw_logistics::ResourceType::Wood),
                    inbound_priority: TransportPriority::Low,
                    target_amount: 1,
                    allow_export: true,
                },
                hw_logistics::BelongsTo(issuer),
            ))
            .id();
        app.insert_resource(ManualHaulApplyFixture {
            issuer,
            area: TaskArea::from_points(Vec2::splat(-1.0), Vec2::splat(1.0)),
        })
        .add_systems(Update, apply_manual_haul_fixture);

        app.update();

        let mut requests = app.world_mut().query_filtered::<(
            &TransportRequest,
            &TransportRequestFixedSource,
            &ManagedBy,
        ), With<ManualTransportRequest>>();
        let requests: Vec<_> = requests.iter(app.world()).collect();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].0.anchor, accepted);
        assert_ne!(requests[0].0.anchor, rejected);
        assert_eq!(
            requests[0].0.resource_type,
            hw_logistics::ResourceType::Wood
        );
        assert_eq!(requests[0].0.priority, TransportPriority::Normal);
        assert_eq!(requests[0].1.0, source);
        assert_eq!(requests[0].2.0, issuer);
        assert!(
            app.world()
                .entity(source)
                .contains::<ManualHaulPinnedSource>()
        );
    }
}
