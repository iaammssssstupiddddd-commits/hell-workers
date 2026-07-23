use bevy::prelude::*;
use hw_core::logistics::ResourceType;
use hw_logistics::transport_request::{TransportRequestKind, WheelbarrowDestination};
use hw_logistics::{
    StockpilePolicyInput, StockpileTransferPhase, evaluate_stockpile_policy,
    stockpile_owner_accepts_item,
};

use super::capacity_helpers::check_stockpile_capacity;
use crate::familiar_ai::decide::task_management::{
    FamiliarTaskAssignmentQueries, IncomingDeliverySnapshot, ReservationShadow,
};

pub use super::water_resolver::{resolve_gather_water_inputs, resolve_haul_water_to_mixer_inputs};

fn choose_compatible_stockpile(
    candidates: impl Iterator<Item = (Entity, usize, Option<Entity>)>,
    fixed_source_owner: Option<Option<Entity>>,
) -> Option<(Entity, usize, Option<Entity>)> {
    candidates
        .filter(|(_, _, stockpile_owner)| {
            fixed_source_owner
                .is_none_or(|item_owner| stockpile_owner_accepts_item(item_owner, *stockpile_owner))
        })
        .min_by(|left, right| {
            left.1
                .cmp(&right.1)
                .then_with(|| left.0.index_u32().cmp(&right.0.index_u32()))
                .then_with(|| {
                    left.0
                        .generation()
                        .to_bits()
                        .cmp(&right.0.generation().to_bits())
                })
        })
}

pub struct ResolvedStockpileInputs {
    pub stockpile: Entity,
    pub resource_type: ResourceType,
    pub item_owner: Option<Entity>,
    pub fixed_source: Option<Entity>,
    pub available_amount: usize,
}

pub struct ResolvedConsolidationInputs {
    pub receiver: Entity,
    pub resource_type: ResourceType,
    pub donor_cells: Vec<Entity>,
    pub receiver_owner: Option<Entity>,
}

pub fn resolve_consolidation_inputs(
    task_entity: Entity,
    queries: &FamiliarTaskAssignmentQueries,
    shadow: &ReservationShadow,
    incoming_snapshot: &IncomingDeliverySnapshot,
) -> Option<ResolvedConsolidationInputs> {
    let req = queries.transport_requests.get(task_entity).ok()?;
    if req.kind != TransportRequestKind::ConsolidateStockpile {
        return None;
    }

    let receiver = req.anchor;
    let resource_type = req.resource_type;
    let expected_priority = queries.receiver_policy_tiers.get(task_entity).ok()?.0;
    check_stockpile_capacity(
        receiver,
        resource_type,
        queries,
        shadow,
        incoming_snapshot,
        Some(expected_priority),
    )?;
    let receiver_owner = queries
        .designation
        .belongs
        .get(receiver)
        .ok()
        .map(|belongs| belongs.0);
    let donor_cells: Vec<Entity> = req
        .stockpile_group
        .iter()
        .copied()
        .filter(|donor| *donor != receiver)
        .filter(|donor| {
            let donor_owner = queries
                .designation
                .belongs
                .get(*donor)
                .ok()
                .map(|belongs| belongs.0);
            if donor_owner != receiver_owner {
                return false;
            }
            let Ok((_, _, stockpile, stored)) = queries.storage.stockpiles.get(*donor) else {
                return false;
            };
            let Ok(policy) = queries.storage.stockpile_policies.get(*donor) else {
                return false;
            };
            let stored_amount = stored.map(|items| items.len()).unwrap_or(0);
            evaluate_stockpile_policy(StockpilePolicyInput {
                phase: StockpileTransferPhase::NewOutbound,
                policy: *policy,
                capacity: stockpile.capacity,
                stored_amount,
                stored_resource: stockpile.resource_type,
                transfer_resource: resource_type,
                requested_amount: 1,
                incoming_reserved: 0,
                incoming_reserved_other_resource: 0,
                cycle_reserved: 0,
                cycle_reserved_other_resource: 0,
            })
            .allowed_amount
                == 1
        })
        .collect();
    if donor_cells.is_empty() {
        return None;
    }

    Some(ResolvedConsolidationInputs {
        receiver,
        resource_type,
        donor_cells,
        receiver_owner,
    })
}

pub fn resolve_haul_to_stockpile_inputs(
    task_entity: Entity,
    queries: &FamiliarTaskAssignmentQueries,
    shadow: &ReservationShadow,
    incoming_snapshot: &IncomingDeliverySnapshot,
) -> Option<ResolvedStockpileInputs> {
    let req = queries.transport_requests.get(task_entity).ok()?;
    if !matches!(req.kind, TransportRequestKind::DepositToStockpile) {
        return None;
    }

    let resource_type = req.resource_type;
    let fixed_source = queries
        .transport_request_fixed_sources
        .get(task_entity)
        .ok()
        .map(|source| source.0);
    let expected_priority = queries
        .receiver_policy_tiers
        .get(task_entity)
        .ok()
        .map(|tier| tier.0);
    if fixed_source.is_none() && expected_priority.is_none() {
        return None;
    }
    let fixed_source_owner = fixed_source.map(|source| {
        queries
            .designation
            .belongs
            .get(source)
            .ok()
            .map(|belongs| belongs.0)
    });
    let leased_stockpile = match queries.wheelbarrow_leases.get(task_entity) {
        Ok(lease) => match lease.destination {
            WheelbarrowDestination::Stockpile(cell) => Some(cell),
            _ => return None,
        },
        Err(_) => None,
    };
    let cells = if req.stockpile_group.is_empty() && fixed_source.is_some() {
        std::slice::from_ref(&req.anchor)
    } else if req.stockpile_group.is_empty() {
        return None;
    } else {
        req.stockpile_group.as_slice()
    };
    let (stockpile, available_amount, item_owner) = choose_compatible_stockpile(
        cells.iter().filter_map(|&cell| {
            if leased_stockpile.is_some_and(|leased| leased != cell) {
                return None;
            }
            let free = check_stockpile_capacity(
                cell,
                resource_type,
                queries,
                shadow,
                incoming_snapshot,
                expected_priority,
            )?;
            let owner = queries
                .designation
                .belongs
                .get(cell)
                .ok()
                .map(|belongs| belongs.0);
            Some((cell, free, owner))
        }),
        fixed_source_owner,
    )?;

    Some(ResolvedStockpileInputs {
        stockpile,
        resource_type,
        item_owner,
        fixed_source,
        available_amount,
    })
}

pub fn resolve_return_bucket_tank(
    task_entity: Entity,
    queries: &FamiliarTaskAssignmentQueries,
) -> Option<Entity> {
    let req = queries.transport_requests.get(task_entity).ok()?;
    if req.kind != TransportRequestKind::ReturnBucket {
        return None;
    }
    let tank = req.anchor;
    let (_, _, stockpile, _) = queries.storage.stockpiles.get(tank).ok()?;
    if stockpile.resource_type != Some(ResourceType::Water) {
        return None;
    }
    Some(tank)
}

pub fn resolve_return_wheelbarrow(
    task_entity: Entity,
    queries: &FamiliarTaskAssignmentQueries,
) -> Option<(Entity, Entity, Vec2)> {
    let req = queries.transport_requests.get(task_entity).ok()?;
    if req.kind != TransportRequestKind::ReturnWheelbarrow {
        return None;
    }

    let wheelbarrow = req.anchor;
    let parking_anchor = queries.designation.belongs.get(wheelbarrow).ok()?.0;
    let (_, wheelbarrow_transform) = queries.wheelbarrows.get(wheelbarrow).ok()?;

    Some((
        wheelbarrow,
        parking_anchor,
        wheelbarrow_transform.translation.truncate(),
    ))
}

pub fn resolve_haul_to_blueprint_inputs(
    task_entity: Entity,
    queries: &FamiliarTaskAssignmentQueries,
) -> Option<(Entity, ResourceType)> {
    let req = queries.transport_requests.get(task_entity).ok()?;
    if !matches!(req.kind, TransportRequestKind::DeliverToBlueprint) {
        return None;
    }
    let blueprint = req.anchor;
    Some((blueprint, req.resource_type))
}

pub fn resolve_haul_to_floor_construction_inputs(
    task_entity: Entity,
    queries: &FamiliarTaskAssignmentQueries,
) -> Option<(Entity, ResourceType)> {
    let req = queries.transport_requests.get(task_entity).ok()?;
    if !matches!(req.kind, TransportRequestKind::DeliverToFloorConstruction) {
        return None;
    }
    Some((req.anchor, req.resource_type))
}

pub fn resolve_haul_to_wall_construction_inputs(
    task_entity: Entity,
    queries: &FamiliarTaskAssignmentQueries,
) -> Option<(Entity, ResourceType)> {
    let req = queries.transport_requests.get(task_entity).ok()?;
    if !matches!(req.kind, TransportRequestKind::DeliverToWallConstruction) {
        return None;
    }
    Some((req.anchor, req.resource_type))
}

pub fn resolve_haul_to_provisional_wall_inputs(
    task_entity: Entity,
    queries: &FamiliarTaskAssignmentQueries,
) -> Option<(Entity, ResourceType)> {
    let req = queries.transport_requests.get(task_entity).ok()?;
    if !matches!(req.kind, TransportRequestKind::DeliverToProvisionalWall) {
        return None;
    }
    Some((req.anchor, req.resource_type))
}

pub fn resolve_haul_to_mixer_inputs(
    task_entity: Entity,
    queries: &FamiliarTaskAssignmentQueries,
) -> Option<(Entity, ResourceType)> {
    let req = queries.transport_requests.get(task_entity).ok()?;
    if !matches!(req.kind, TransportRequestKind::DeliverToMixerSolid) {
        return None;
    }
    let mixer_entity = req.anchor;
    Some((mixer_entity, req.resource_type))
}

pub fn resolve_haul_to_soul_spa_inputs(
    task_entity: Entity,
    queries: &FamiliarTaskAssignmentQueries,
) -> Option<(Entity, ResourceType)> {
    let req = queries.transport_requests.get(task_entity).ok()?;
    if !matches!(req.kind, TransportRequestKind::DeliverToSoulSpa) {
        return None;
    }
    Some((req.anchor, req.resource_type))
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_core::events::ResourceReservationRequest;
    use hw_core::relationships::DeliveringTo;
    use hw_jobs::events::TaskAssignmentRequest;
    use hw_logistics::SharedResourceCache;
    use hw_logistics::transport_request::{
        ReceiverPolicyTier, TransportPriority, TransportRequest, WheelbarrowArbitrationDiagnostics,
        WheelbarrowLease,
    };
    use hw_logistics::zone::{Stockpile, StockpileAcceptance, StockpilePolicy};
    use hw_world::WorldMap;

    #[derive(Resource)]
    struct ResolverProbe {
        task: Entity,
        selected: Option<Entity>,
    }

    #[derive(Resource)]
    struct ConsolidationResolverProbe {
        task: Entity,
        resolved: bool,
    }

    fn resolve_stockpile_probe(
        mut probe: ResMut<ResolverProbe>,
        queries: FamiliarTaskAssignmentQueries,
    ) {
        let incoming = IncomingDeliverySnapshot::build(&queries);
        probe.selected = resolve_haul_to_stockpile_inputs(
            probe.task,
            &queries,
            &ReservationShadow::default(),
            &incoming,
        )
        .map(|resolved| resolved.stockpile);
    }

    fn resolve_consolidation_probe(
        mut probe: ResMut<ConsolidationResolverProbe>,
        queries: FamiliarTaskAssignmentQueries,
    ) {
        let incoming = IncomingDeliverySnapshot::build(&queries);
        probe.resolved = resolve_consolidation_inputs(
            probe.task,
            &queries,
            &ReservationShadow::default(),
            &incoming,
        )
        .is_some();
    }

    fn resolver_base_app() -> App {
        let mut app = App::new();
        app.insert_resource(WorldMap::default())
            .init_resource::<SharedResourceCache>()
            .init_resource::<WheelbarrowArbitrationDiagnostics>()
            .add_message::<ResourceReservationRequest>()
            .add_message::<TaskAssignmentRequest>();
        app
    }

    fn resolver_test_app() -> App {
        let mut app = resolver_base_app();
        app.add_systems(Update, resolve_stockpile_probe);
        app
    }

    fn spawn_stockpile(app: &mut App, capacity: usize) -> Entity {
        app.world_mut()
            .spawn((
                Transform::default(),
                Stockpile {
                    capacity,
                    resource_type: None,
                },
                StockpilePolicy::for_capacity(capacity),
            ))
            .id()
    }

    fn spawn_request(app: &mut App, anchor: Entity, cells: Vec<Entity>) -> Entity {
        let issued_by = app.world_mut().spawn_empty().id();
        app.world_mut()
            .spawn((
                TransportRequest {
                    kind: TransportRequestKind::DepositToStockpile,
                    anchor,
                    resource_type: ResourceType::Wood,
                    issued_by,
                    priority: TransportPriority::Normal,
                    stockpile_group: cells,
                },
                ReceiverPolicyTier(TransportPriority::Normal),
            ))
            .id()
    }

    fn entity(index: u32) -> Entity {
        Entity::from_raw_u32(index).expect("valid entity")
    }

    #[test]
    fn selected_destination_owner_drives_the_source_owner_filter() {
        let owner_a = entity(100);
        let owner_b = entity(101);
        let cell_a = entity(1);
        let cell_b = entity(2);

        assert_eq!(
            choose_compatible_stockpile(
                [(cell_a, 4, Some(owner_a)), (cell_b, 3, Some(owner_b))].into_iter(),
                None,
            ),
            Some((cell_b, 3, Some(owner_b)))
        );
    }

    #[test]
    fn fixed_owned_source_excludes_a_smaller_other_owner_cell() {
        let owner_a = entity(100);
        let owner_b = entity(101);
        let cell_a = entity(1);
        let cell_b = entity(2);

        assert_eq!(
            choose_compatible_stockpile(
                [(cell_a, 4, Some(owner_a)), (cell_b, 3, Some(owner_b))].into_iter(),
                Some(Some(owner_a)),
            ),
            Some((cell_a, 4, Some(owner_a)))
        );
    }

    #[test]
    fn live_policy_change_stops_a_new_stockpile_assignment() {
        let mut app = resolver_test_app();
        let stockpile = spawn_stockpile(&mut app, 4);
        let request = spawn_request(&mut app, stockpile, vec![stockpile]);
        app.insert_resource(ResolverProbe {
            task: request,
            selected: None,
        });

        app.update();
        assert_eq!(
            app.world().resource::<ResolverProbe>().selected,
            Some(stockpile)
        );

        *app.world_mut()
            .get_mut::<StockpilePolicy>(stockpile)
            .expect("stockpile policy") = StockpilePolicy {
            acceptance: StockpileAcceptance::Only(ResourceType::Rock),
            inbound_priority: TransportPriority::Normal,
            target_amount: 0,
            allow_export: true,
        }
        .normalized_for_capacity(4);

        app.update();
        assert_eq!(app.world().resource::<ResolverProbe>().selected, None);
    }

    #[test]
    fn consolidation_is_revalidated_after_production_and_before_assignment() {
        let mut app = resolver_base_app();
        app.add_systems(Update, resolve_consolidation_probe);
        let owner = app.world_mut().spawn_empty().id();
        let receiver = app
            .world_mut()
            .spawn((
                Transform::default(),
                Stockpile {
                    capacity: 2,
                    resource_type: Some(ResourceType::Wood),
                },
                StockpilePolicy::for_capacity(2),
                hw_logistics::BelongsTo(owner),
            ))
            .id();
        let donor = app
            .world_mut()
            .spawn((
                Transform::from_xyz(1.0, 0.0, 0.0),
                Stockpile {
                    capacity: 2,
                    resource_type: Some(ResourceType::Wood),
                },
                StockpilePolicy {
                    acceptance: StockpileAcceptance::Only(ResourceType::Rock),
                    inbound_priority: TransportPriority::Normal,
                    target_amount: 2,
                    allow_export: false,
                },
                hw_logistics::BelongsTo(owner),
            ))
            .id();
        app.world_mut().spawn((
            hw_logistics::ResourceItem(ResourceType::Wood),
            hw_core::relationships::StoredIn(donor),
        ));
        let request = app
            .world_mut()
            .spawn((
                TransportRequest {
                    kind: TransportRequestKind::ConsolidateStockpile,
                    anchor: receiver,
                    resource_type: ResourceType::Wood,
                    issued_by: owner,
                    priority: TransportPriority::Low,
                    stockpile_group: vec![receiver, donor],
                },
                ReceiverPolicyTier(TransportPriority::Normal),
            ))
            .id();
        app.insert_resource(ConsolidationResolverProbe {
            task: request,
            resolved: false,
        });

        app.update();
        assert!(
            app.world()
                .resource::<ConsolidationResolverProbe>()
                .resolved
        );

        app.world_mut()
            .get_mut::<StockpilePolicy>(receiver)
            .expect("receiver policy")
            .target_amount = 0;
        app.update();
        assert!(
            !app.world()
                .resource::<ConsolidationResolverProbe>()
                .resolved
        );
    }

    #[test]
    fn unreadable_incoming_item_still_reserves_physical_capacity() {
        let mut app = resolver_test_app();
        let stockpile = spawn_stockpile(&mut app, 1);
        let request = spawn_request(&mut app, stockpile, vec![stockpile]);
        app.world_mut().spawn(DeliveringTo(stockpile));
        app.insert_resource(ResolverProbe {
            task: request,
            selected: None,
        });

        app.update();

        assert_eq!(app.world().resource::<ResolverProbe>().selected, None);
    }

    #[test]
    fn wheelbarrow_lease_pins_the_destination_without_fallback() {
        let mut app = resolver_test_app();
        let preferred_without_lease = spawn_stockpile(&mut app, 2);
        let leased_destination = spawn_stockpile(&mut app, 8);
        let request = spawn_request(
            &mut app,
            preferred_without_lease,
            vec![preferred_without_lease, leased_destination],
        );
        let wheelbarrow = app.world_mut().spawn_empty().id();
        app.world_mut()
            .entity_mut(request)
            .insert(WheelbarrowLease {
                wheelbarrow,
                items: Vec::new(),
                source_pos: Vec2::ZERO,
                destination: WheelbarrowDestination::Stockpile(leased_destination),
                lease_until: 1.0,
            });
        app.insert_resource(ResolverProbe {
            task: request,
            selected: None,
        });

        app.update();
        assert_eq!(
            app.world().resource::<ResolverProbe>().selected,
            Some(leased_destination)
        );

        app.world_mut()
            .get_mut::<StockpilePolicy>(leased_destination)
            .expect("stockpile policy")
            .target_amount = 0;

        app.update();
        assert_eq!(app.world().resource::<ResolverProbe>().selected, None);
    }
}
