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
