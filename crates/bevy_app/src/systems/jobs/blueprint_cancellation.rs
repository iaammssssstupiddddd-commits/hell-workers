//! Owner-controlled blueprint cancellation.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::events::SoulTaskUnassignRequest;
use hw_core::logistics::WheelbarrowDestination;
use hw_core::relationships::StoredIn;
use hw_core::soul::DamnedSoul;
use hw_jobs::{AssignedTask, Blueprint, BlueprintCancelRequested, TargetBlueprint};
use hw_logistics::transport_request::TransportRequest;
use hw_logistics::{
    PendingBelongsToBlueprint, ResourceItemVisualHandles, ResourceType, spawn_refund_items,
};

use crate::world::map::WorldMapWrite;

fn task_targets_blueprint(task: &AssignedTask, blueprint: Entity) -> bool {
    match task {
        AssignedTask::Build(data) => data.blueprint == blueprint,
        AssignedTask::HaulToBlueprint(data) => data.blueprint == blueprint,
        AssignedTask::HaulWithWheelbarrow(data) => {
            data.destination == WheelbarrowDestination::Blueprint(blueprint)
        }
        _ => false,
    }
}

/// Cancels blueprints marked by the UI action adapter.
///
/// Correctness is based on task payloads and request anchors, not only on
/// relationship targets that may be temporarily absent during load/rehydrate.
#[derive(SystemParam)]
pub struct BlueprintCancellationQueries<'w, 's> {
    blueprints: Query<
        'w,
        's,
        (Entity, &'static Transform, &'static Blueprint),
        With<BlueprintCancelRequested>,
    >,
    souls: Query<'w, 's, (Entity, &'static AssignedTask), With<DamnedSoul>>,
    requests: Query<
        'w,
        's,
        (
            Entity,
            &'static TransportRequest,
            Option<&'static TargetBlueprint>,
        ),
    >,
    pending: Query<'w, 's, (Entity, &'static PendingBelongsToBlueprint)>,
    stored_items: Query<'w, 's, (Entity, &'static StoredIn)>,
}

pub fn blueprint_cancellation_system(
    mut commands: Commands,
    queries: BlueprintCancellationQueries,
    mut world_map: WorldMapWrite,
    resource_item_handles: Res<ResourceItemVisualHandles>,
) {
    for (blueprint_entity, transform, blueprint) in &queries.blueprints {
        for (soul_entity, task) in &queries.souls {
            if task_targets_blueprint(task, blueprint_entity) {
                commands.write_message(SoulTaskUnassignRequest {
                    soul_entity,
                    emit_abandoned: true,
                });
            }
        }

        for (request_entity, request, target) in &queries.requests {
            if request.anchor == blueprint_entity
                || target.is_some_and(|target| target.0 == blueprint_entity)
            {
                commands.entity(request_entity).try_despawn();
            }
        }
        for (pending_entity, pending) in &queries.pending {
            if pending.0 == blueprint_entity {
                let grids: Vec<_> = world_map
                    .stockpile_entries()
                    .filter_map(|(&grid, &owner)| (owner == pending_entity).then_some(grid))
                    .collect();
                for grid in grids {
                    world_map.clear_stockpile_tile_if_owned(grid, pending_entity);
                }
                for (item_entity, stored_in) in &queries.stored_items {
                    if stored_in.0 == pending_entity {
                        commands
                            .entity(item_entity)
                            .remove::<StoredIn>()
                            .try_insert(Visibility::Visible);
                    }
                }
                commands.entity(pending_entity).try_despawn();
            }
        }

        let center = transform.translation.truncate();
        let mut delivered: Vec<_> = blueprint
            .delivered_materials
            .iter()
            .map(|(&resource_type, &amount)| (resource_type, amount))
            .collect();
        delivered.sort_unstable_by_key(|(resource_type, _)| resource_order(*resource_type));
        for (resource_type, amount) in delivered {
            spawn_refund_items(
                &mut commands,
                &resource_item_handles,
                center,
                resource_type,
                amount,
            );
        }

        for &grid in &blueprint.occupied_grids {
            world_map.clear_building_occupancy_if_owned(grid, blueprint_entity);
        }
        commands.entity(blueprint_entity).try_despawn();
    }
}

const fn resource_order(resource_type: ResourceType) -> u8 {
    match resource_type {
        ResourceType::Wood => 0,
        ResourceType::Rock => 1,
        ResourceType::Bone => 2,
        ResourceType::Sand => 3,
        ResourceType::StasisMud => 4,
        ResourceType::Water => 5,
        ResourceType::BucketEmpty => 6,
        ResourceType::BucketWater => 7,
        ResourceType::Wheelbarrow => 8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::schedule::ApplyDeferred;
    use hw_jobs::{
        BuildData, BuildPhase, BuildingType, HaulToBlueprintData, HaulToBpPhase,
        HaulWithWheelbarrowData, HaulWithWheelbarrowPhase,
    };
    use hw_logistics::transport_request::{TransportPriority, TransportRequestKind};
    use std::collections::HashMap;

    #[derive(Resource, Default)]
    struct UnassignReceipts(Vec<Entity>);

    fn collect_unassign(
        mut requests: MessageReader<SoulTaskUnassignRequest>,
        mut receipts: ResMut<UnassignReceipts>,
    ) {
        receipts
            .0
            .extend(requests.read().map(|request| request.soul_entity));
    }

    #[test]
    fn cancellation_uses_payload_and_refunds_delivered_materials_once() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<crate::world::map::WorldMap>()
            .insert_resource(ResourceItemVisualHandles {
                icon_bone_small: default(),
                icon_wood_small: default(),
                icon_rock_small: default(),
                icon_sand_small: default(),
                icon_stasis_mud_small: default(),
            })
            .init_resource::<UnassignReceipts>()
            .add_message::<SoulTaskUnassignRequest>()
            .add_systems(
                Update,
                (
                    blueprint_cancellation_system,
                    ApplyDeferred,
                    collect_unassign,
                )
                    .chain(),
            );

        let mut blueprint = Blueprint::new(BuildingType::Bridge, vec![(2, 3)]);
        blueprint.delivered_materials =
            HashMap::from([(ResourceType::Wood, 2), (ResourceType::Rock, 1)]);
        let blueprint_entity = app
            .world_mut()
            .spawn((Transform::default(), blueprint, BlueprintCancelRequested))
            .id();
        app.world_mut()
            .resource_mut::<crate::world::map::WorldMap>()
            .reserve_building_footprint(BuildingType::Bridge, blueprint_entity, [(2, 3)]);
        let pending_storage = app
            .world_mut()
            .spawn((
                PendingBelongsToBlueprint(blueprint_entity),
                crate::systems::logistics::Stockpile {
                    capacity: 1,
                    resource_type: Some(ResourceType::BucketEmpty),
                },
                Transform::from_xyz(128.0, 128.0, 0.0),
            ))
            .id();
        app.world_mut()
            .resource_mut::<crate::world::map::WorldMap>()
            .register_stockpile_tile((4, 4), pending_storage);
        let stored_item = app
            .world_mut()
            .spawn((
                hw_logistics::ResourceItem(ResourceType::BucketEmpty),
                StoredIn(pending_storage),
                Visibility::Hidden,
                Transform::from_xyz(128.0, 128.0, 0.0),
            ))
            .id();
        let soul = app
            .world_mut()
            .spawn((
                DamnedSoul::default(),
                AssignedTask::Build(BuildData {
                    blueprint: blueprint_entity,
                    phase: BuildPhase::GoingToBlueprint,
                }),
            ))
            .id();
        let haul_item = app.world_mut().spawn_empty().id();
        let haul_soul = app
            .world_mut()
            .spawn((
                DamnedSoul::default(),
                AssignedTask::HaulToBlueprint(HaulToBlueprintData {
                    item: haul_item,
                    blueprint: blueprint_entity,
                    phase: HaulToBpPhase::GoingToItem,
                }),
            ))
            .id();
        let wheelbarrow = app.world_mut().spawn_empty().id();
        let wheelbarrow_soul = app
            .world_mut()
            .spawn((
                DamnedSoul::default(),
                AssignedTask::HaulWithWheelbarrow(HaulWithWheelbarrowData {
                    wheelbarrow,
                    source_pos: Vec2::ZERO,
                    destination: WheelbarrowDestination::Blueprint(blueprint_entity),
                    collect_source: None,
                    collect_amount: 0,
                    collect_resource_type: None,
                    items: Vec::new(),
                    phase: HaulWithWheelbarrowPhase::GoingToParking,
                }),
            ))
            .id();
        let request_owner = app.world_mut().spawn_empty().id();
        let anchor_request = app
            .world_mut()
            .spawn(TransportRequest {
                kind: TransportRequestKind::DeliverToBlueprint,
                anchor: blueprint_entity,
                resource_type: ResourceType::Wood,
                issued_by: request_owner,
                priority: TransportPriority::Normal,
                stockpile_group: Vec::new(),
            })
            .id();
        let target_request = app
            .world_mut()
            .spawn((
                TransportRequest {
                    kind: TransportRequestKind::DeliverToBlueprint,
                    anchor: request_owner,
                    resource_type: ResourceType::Rock,
                    issued_by: request_owner,
                    priority: TransportPriority::Normal,
                    stockpile_group: Vec::new(),
                },
                TargetBlueprint(blueprint_entity),
            ))
            .id();

        app.update();

        assert!(app.world().get_entity(blueprint_entity).is_err());
        assert_eq!(
            app.world()
                .resource::<crate::world::map::WorldMap>()
                .building_entity((2, 3)),
            None
        );
        let unassigned: std::collections::HashSet<_> = app
            .world()
            .resource::<UnassignReceipts>()
            .0
            .iter()
            .copied()
            .collect();
        assert_eq!(unassigned.len(), 3);
        assert!(unassigned.contains(&soul));
        assert!(unassigned.contains(&haul_soul));
        assert!(unassigned.contains(&wheelbarrow_soul));
        assert!(app.world().get_entity(anchor_request).is_err());
        assert!(app.world().get_entity(target_request).is_err());
        assert!(app.world().get_entity(pending_storage).is_err());
        assert_eq!(
            app.world()
                .resource::<crate::world::map::WorldMap>()
                .stockpile_entity((4, 4)),
            None
        );
        assert!(app.world().get::<StoredIn>(stored_item).is_none());
        assert_eq!(
            app.world().get::<Visibility>(stored_item),
            Some(&Visibility::Visible)
        );
        let mut refunded = HashMap::<ResourceType, usize>::new();
        let mut resources = app.world_mut().query::<&hw_logistics::ResourceItem>();
        for item in resources.iter(app.world()) {
            *refunded.entry(item.0).or_default() += 1;
        }
        assert_eq!(refunded.get(&ResourceType::Wood), Some(&2));
        assert_eq!(refunded.get(&ResourceType::Rock), Some(&1));

        app.update();
        let mut resources = app.world_mut().query::<&hw_logistics::ResourceItem>();
        assert_eq!(resources.iter(app.world()).count(), 4);
    }
}
