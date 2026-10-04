//! Shared transaction primitives for floor and wall construction cancellation.

use bevy::prelude::*;
use hw_core::relationships::WorkingOn;
use hw_logistics::transport_request::{TransportRequest, TransportRequestKind};
use hw_soul_ai::unassign_task;

use crate::entities::damned_soul::{DamnedSoul, Path};
use crate::systems::jobs::{ResourceItemVisualHandles, spawn_refund_items};
use crate::systems::logistics::{Inventory, ResourceType};
use crate::systems::soul_ai::execute::task_execution::context::TaskQueries;
use crate::systems::soul_ai::execute::task_execution::types::AssignedTask;
use crate::world::map::WorldMap;

pub(super) type ConstructionCancellationSoulQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static mut AssignedTask,
        &'static mut Path,
        &'static mut Inventory,
        Option<&'static WorkingOn>,
    ),
    With<DamnedSoul>,
>;

#[derive(Debug, Clone, Copy)]
pub(super) struct ConstructionCancellationPolicy {
    request_kind: TransportRequestKind,
    refund_primary: ResourceType,
    refund_secondary: ResourceType,
}

impl ConstructionCancellationPolicy {
    pub(super) const FLOOR: Self = Self {
        request_kind: TransportRequestKind::DeliverToFloorConstruction,
        refund_primary: ResourceType::Bone,
        refund_secondary: ResourceType::StasisMud,
    };

    pub(super) const WALL: Self = Self {
        request_kind: TransportRequestKind::DeliverToWallConstruction,
        refund_primary: ResourceType::Wood,
        refund_secondary: ResourceType::StasisMud,
    };
}

pub(super) fn collect_site_requests(
    requests: &Query<(Entity, &TransportRequest)>,
    site: Entity,
    policy: ConstructionCancellationPolicy,
) -> Vec<Entity> {
    requests
        .iter()
        .filter_map(|(entity, request)| {
            (request.kind == policy.request_kind && request.anchor == site).then_some(entity)
        })
        .collect()
}

pub(super) fn release_matching_workers(
    commands: &mut Commands,
    q_souls: &mut ConstructionCancellationSoulQuery<'_, '_>,
    reservation_queries: &mut TaskQueries<'_, '_>,
    world_map: &WorldMap,
    mut matches: impl FnMut(&AssignedTask, Option<&WorkingOn>) -> bool,
) -> usize {
    let mut released = 0;
    for (soul, transform, mut task, mut path, mut inventory, working_on) in q_souls.iter_mut() {
        if !matches(&task, working_on) {
            continue;
        }
        unassign_task(
            commands,
            hw_soul_ai::SoulDropCtx {
                soul_entity: soul,
                drop_pos: transform.translation.truncate(),
                inventory: Some(&mut inventory),
                dropped_item_res: None,
            },
            &mut task,
            &mut path,
            reservation_queries,
            world_map,
            true,
        );
        released += 1;
    }
    released
}

pub(super) fn spawn_construction_refunds(
    commands: &mut Commands,
    handles: &ResourceItemVisualHandles,
    position: Vec2,
    policy: ConstructionCancellationPolicy,
    primary_amount: u32,
    secondary_amount: u32,
) {
    spawn_refund_items(
        commands,
        handles,
        position,
        policy.refund_primary,
        primary_amount,
    );
    spawn_refund_items(
        commands,
        handles,
        position,
        policy.refund_secondary,
        secondary_amount,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_core::{
        area::TaskArea,
        events::{OnTaskAbandoned, ResourceReservationOp, ResourceReservationRequest},
        relationships::DeliveringTo,
    };
    use hw_jobs::construction::{
        FloorConstructionCancelRequested, WallConstructionCancelRequested,
    };
    use hw_jobs::{FloorConstructionSite, HaulData, HaulPhase, WallConstructionSite};
    use hw_logistics::{SharedResourceCache, tile_index::TileSiteIndex};

    #[test]
    fn construction_cancellation_releases_secondary_destination_test() {
        for floor in [false, true] {
            for with_working_on in [false, true] {
                let mut app = App::new();
                app.add_plugins(MinimalPlugins)
                    .init_resource::<WorldMap>()
                    .init_resource::<SharedResourceCache>()
                    .init_resource::<TileSiteIndex>()
                    .insert_resource(ResourceItemVisualHandles {
                        icon_bone_small: default(),
                        icon_wood_small: default(),
                        icon_rock_small: default(),
                        icon_sand_small: default(),
                        icon_stasis_mud_small: default(),
                    })
                    .add_message::<ResourceReservationRequest>()
                    .add_message::<OnTaskAbandoned>();
                let area = TaskArea::from_points(Vec2::ZERO, Vec2::ZERO);
                let site = app.world_mut().spawn(Transform::default()).id();
                if floor {
                    app.world_mut().entity_mut(site).insert((
                        FloorConstructionSite::new(area, Vec2::ZERO, 0),
                        FloorConstructionCancelRequested,
                    ));
                    app.add_systems(Update, super::super::floor_construction::cancellation::floor_construction_cancellation_system);
                } else {
                    app.world_mut().entity_mut(site).insert((
                        WallConstructionSite::new(area, Vec2::ZERO, 0),
                        WallConstructionCancelRequested,
                    ));
                    app.add_systems(Update, super::super::wall_construction::cancellation::wall_construction_cancellation_system);
                }
                let item = app.world_mut().spawn(DeliveringTo(site)).id();
                let soul = app
                    .world_mut()
                    .spawn((
                        Transform::default(),
                        DamnedSoul::default(),
                        Path::default(),
                        Inventory::default(),
                        AssignedTask::Haul(HaulData {
                            item,
                            stockpile: site,
                            phase: HaulPhase::GoingToItem,
                        }),
                    ))
                    .id();
                if with_working_on {
                    app.world_mut().entity_mut(soul).insert(WorkingOn(item));
                }
                app.update();
                assert!(matches!(
                    app.world().get::<AssignedTask>(soul),
                    Some(AssignedTask::None)
                ));
                assert!(app.world().get::<WorkingOn>(soul).is_none());
                assert!(app.world().get::<DeliveringTo>(item).is_none());
                assert!(app.world().get_entity(site).is_err());
                let reservations: Vec<_> = app
                    .world_mut()
                    .resource_mut::<Messages<ResourceReservationRequest>>()
                    .drain()
                    .map(|message| message.op)
                    .collect();
                assert_eq!(
                    reservations,
                    vec![ResourceReservationOp::ReleaseSource {
                        source: item.into(),
                        amount: 1
                    }]
                );
                assert_eq!(
                    app.world_mut()
                        .resource_mut::<Messages<OnTaskAbandoned>>()
                        .drain()
                        .count(),
                    1
                );
                app.update();
                assert!(
                    app.world()
                        .resource::<Messages<ResourceReservationRequest>>()
                        .is_empty()
                );
                assert!(
                    app.world()
                        .resource::<Messages<OnTaskAbandoned>>()
                        .is_empty()
                );
            }
        }
    }
}
