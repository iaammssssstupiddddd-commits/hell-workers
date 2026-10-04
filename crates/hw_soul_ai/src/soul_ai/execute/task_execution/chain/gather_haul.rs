//! Admission of a Gather -> Haul segment using live claims plus this execution loop's delta.
use super::super::{
    context::{TaskExecutionContext, TaskHandlerControl},
    stockpile_policy::inbound_reservation_snapshot,
};
use bevy::prelude::*;
use hw_core::{constants::TILE_SIZE, relationships::WorkingOn};
use hw_jobs::construction::{FloorConstructionPhase, WallConstructionPhase};
use hw_jobs::{
    AssignedTask, HaulData, HaulPhase, HaulToBlueprintData, HaulToBpPhase, HaulToMixerData,
    HaulToMixerPhase, WorkType, lifecycle,
};
use hw_logistics::transport_request::{TransportRequestKind, TransportRequestState};
use hw_logistics::{
    ResourceType, StockpileContentsSnapshot, StockpileTransferPhase, evaluate_stockpile_policy,
    stockpile_owner_accepts_item,
};
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub(crate) struct ChainAdmissionShadow {
    sources: HashSet<Entity>,
    inbound: HashMap<(Entity, ResourceType), usize>,
}

impl ChainAdmissionShadow {
    fn matching(&self, destination: Entity, resource: ResourceType) -> usize {
        self.inbound
            .get(&(destination, resource))
            .copied()
            .unwrap_or(0)
    }
    fn total(&self, destination: Entity) -> usize {
        self.inbound
            .iter()
            .filter(|((owner, _), _)| *owner == destination)
            .map(|(_, count)| count)
            .sum()
    }
}

pub(in crate::soul_ai::execute::task_execution) struct PreparedGatherHaulSegment {
    item: Entity,
    destination: Entity,
    resource: ResourceType,
    source_pos: Vec2,
    task: AssignedTask,
    work_type: WorkType,
}

fn pending(entity: Entity, ctx: &TaskExecutionContext) -> bool {
    ctx.queries.deconstruction_pending.contains(entity)
        || ctx
            .queries
            .designation
            .belongs
            .get(entity)
            .is_ok_and(|owner| ctx.queries.deconstruction_pending.contains(owner.0))
}

fn incoming_for(
    destination: Entity,
    accepts: impl Fn(ResourceType) -> bool,
    ctx: &TaskExecutionContext,
) -> usize {
    ctx.queries
        .reservation
        .incoming_deliveries_query
        .get(destination)
        .ok()
        .map_or(0, |(_, incoming)| {
            incoming
                .iter()
                .filter(|item| {
                    ctx.queries
                        .reservation
                        .resources
                        .get(**item)
                        .is_ok_and(|item| accepts(item.0))
                })
                .count()
        })
}

fn reserved_for(destination: Entity, resource: ResourceType, ctx: &TaskExecutionContext) -> usize {
    incoming_for(destination, |incoming| incoming == resource, ctx)
        + ctx.chain_shadow.matching(destination, resource)
}

fn request_destination(
    kind: TransportRequestKind,
    destination: Entity,
    resource: ResourceType,
    ctx: &TaskExecutionContext,
) -> Option<(u8, Vec2)> {
    if pending(destination, ctx) {
        return None;
    }
    let (priority, transform, remaining) = match kind {
        TransportRequestKind::DeliverToWallConstruction => {
            let (transform, site, _) = ctx.queries.storage.wall_sites.get(destination).ok()?;
            if !matches!(
                (site.phase, resource),
                (WallConstructionPhase::Framing, ResourceType::Wood)
                    | (WallConstructionPhase::Coating, ResourceType::StasisMud)
            ) {
                return None;
            }
            let demand = hw_logistics::wall_construction::wall_site_tile_demand(
                ctx.queries
                    .storage
                    .wall_tiles
                    .iter()
                    .map(|(_, tile, _)| tile),
                destination,
                resource,
            );
            (
                0,
                transform,
                demand.saturating_sub(reserved_for(destination, resource, ctx)),
            )
        }
        TransportRequestKind::DeliverToFloorConstruction => {
            let (transform, site, _) = ctx.queries.storage.floor_sites.get(destination).ok()?;
            if !matches!(
                (site.phase, resource),
                (FloorConstructionPhase::Reinforcing, ResourceType::Bone)
                    | (FloorConstructionPhase::Pouring, ResourceType::StasisMud)
            ) {
                return None;
            }
            let demand = hw_logistics::floor_construction::floor_site_tile_demand(
                ctx.queries
                    .storage
                    .floor_tiles
                    .iter()
                    .map(|(_, tile, _)| tile),
                destination,
                resource,
            );
            (
                0,
                transform,
                demand.saturating_sub(reserved_for(destination, resource, ctx)),
            )
        }
        TransportRequestKind::DeliverToBlueprint => {
            let (transform, blueprint, _) = ctx.queries.storage.blueprints.get(destination).ok()?;
            let reserved = if let Some(flexible) = &blueprint.flexible_material_requirement {
                if !flexible.accepts(resource) {
                    return None;
                }
                incoming_for(destination, |incoming| flexible.accepts(incoming), ctx)
                    + flexible
                        .accepted_types
                        .iter()
                        .map(|resource| ctx.chain_shadow.matching(destination, *resource))
                        .sum::<usize>()
            } else {
                reserved_for(destination, resource, ctx)
            };
            (
                1,
                transform,
                (blueprint.remaining_material_amount(resource) as usize).saturating_sub(reserved),
            )
        }
        TransportRequestKind::DeliverToMixerSolid => {
            let (transform, storage, _) = ctx.queries.storage.mixers.get(destination).ok()?;
            let stored = match resource {
                ResourceType::Sand => storage.sand,
                ResourceType::Rock => storage.rock,
                _ => return None,
            };
            let reserved = ctx
                .queries
                .reservation
                .resource_cache
                .get_mixer_destination_reservation(destination, resource)
                + ctx.chain_shadow.matching(destination, resource);
            (
                2,
                transform,
                (hw_core::constants::MUD_MIXER_CAPACITY.saturating_sub(stored) as usize)
                    .saturating_sub(reserved),
            )
        }
        _ => return None,
    };
    (remaining > 0).then_some((priority, transform.translation.truncate()))
}

fn prepare_destination(
    item: Entity,
    resource: ResourceType,
    source_pos: Vec2,
    soul_pos: Vec2,
    ctx: &TaskExecutionContext,
) -> Option<PreparedGatherHaulSegment> {
    let request = ctx
        .queries
        .transport_request_status
        .iter()
        .filter(|(request, demand, state, lease, _)| {
            request.resource_type == resource
                && **state == TransportRequestState::Pending
                && lease.is_none()
                && demand.remaining() > 0
        })
        .filter_map(|(request, _, _, _, _)| {
            request_destination(request.kind, request.anchor, resource, ctx)
                .map(|(priority, pos)| (request, priority, pos.distance_squared(soul_pos)))
        })
        .min_by(|(_, ap, ad), (_, bp, bd)| ap.cmp(bp).then_with(|| ad.total_cmp(bd)));
    let (destination, task, work_type) = if let Some((request, _, _)) = request {
        let destination = request.anchor;
        match request.kind {
            TransportRequestKind::DeliverToBlueprint => (
                destination,
                AssignedTask::HaulToBlueprint(HaulToBlueprintData {
                    item,
                    blueprint: destination,
                    phase: HaulToBpPhase::GoingToItem,
                }),
                WorkType::Haul,
            ),
            TransportRequestKind::DeliverToMixerSolid => (
                destination,
                AssignedTask::HaulToMixer(HaulToMixerData {
                    item,
                    mixer: destination,
                    resource_type: resource,
                    phase: HaulToMixerPhase::GoingToItem,
                }),
                WorkType::HaulToMixer,
            ),
            _ => (
                destination,
                AssignedTask::Haul(HaulData {
                    item,
                    stockpile: destination,
                    phase: HaulPhase::GoingToItem,
                }),
                WorkType::Haul,
            ),
        }
    } else {
        let item_owner = ctx
            .queries
            .designation
            .belongs
            .get(item)
            .ok()
            .map(|owner| owner.0);
        let empty_owned = HashSet::new();
        let (destination, _, _, _) = ctx
            .queries
            .storage
            .stockpiles
            .iter()
            .filter(|(destination, _, stockpile, stored)| {
                if ctx.queries.storage.bucket_storages.contains(*destination)
                    || pending(*destination, ctx)
                {
                    return false;
                }
                let owner = ctx
                    .queries
                    .designation
                    .belongs
                    .get(*destination)
                    .ok()
                    .map(|owner| owner.0);
                if !stockpile_owner_accepts_item(item_owner, owner) {
                    return false;
                }
                let Ok(policy) = ctx.queries.stockpile_policies.get(*destination) else {
                    return false;
                };
                let reservations = inbound_reservation_snapshot(
                    *destination,
                    resource,
                    &empty_owned,
                    &ctx.queries.reservation.incoming_deliveries_query,
                    &ctx.queries.reservation.resources,
                )
                .with_cycle_counts(
                    ctx.chain_shadow.total(*destination),
                    ctx.chain_shadow.matching(*destination, resource),
                );
                evaluate_stockpile_policy(
                    StockpileContentsSnapshot {
                        policy: *policy,
                        capacity: stockpile.capacity,
                        stored_amount: stored.map_or(0, |items| items.len()),
                        stored_resource: stockpile.resource_type,
                    }
                    .policy_input(
                        StockpileTransferPhase::NewInbound,
                        resource,
                        1,
                        reservations,
                    ),
                )
                .allowed_amount
                    == 1
            })
            .min_by(|(_, a, _, _), (_, b, _, _)| {
                a.translation
                    .truncate()
                    .distance_squared(soul_pos)
                    .total_cmp(&b.translation.truncate().distance_squared(soul_pos))
            })?;
        (
            destination,
            AssignedTask::Haul(HaulData {
                item,
                stockpile: destination,
                phase: HaulPhase::GoingToItem,
            }),
            WorkType::Haul,
        )
    };
    Some(PreparedGatherHaulSegment {
        item,
        destination,
        resource,
        source_pos,
        task,
        work_type,
    })
}

/// Prepare a fully admissible source/destination pair without changing ECS or reservations.
pub(in crate::soul_ai::execute::task_execution) fn prepare_gather_haul_segment(
    resource: ResourceType,
    soul_pos: Vec2,
    ctx: &TaskExecutionContext,
) -> Option<PreparedGatherHaulSegment> {
    ctx.queries
        .resource_items
        .iter()
        .filter(|(entity, transform, visibility, item, stored, loaded)| {
            **visibility != Visibility::Hidden
                && item.0 == resource
                && stored.is_none()
                && loaded.is_none()
                && transform.translation.truncate().distance_squared(soul_pos)
                    <= (TILE_SIZE * 4.0).powi(2)
                && !ctx.queries.chain_unavailable_sources.contains(*entity)
                && !ctx.chain_shadow.sources.contains(entity)
                && ctx
                    .queries
                    .reservation
                    .resource_cache
                    .get_source_reservation(*entity)
                    == 0
                && !pending(*entity, ctx)
        })
        .filter_map(|(item, transform, _, _, _, _)| {
            prepare_destination(
                item,
                resource,
                transform.translation.truncate(),
                soul_pos,
                ctx,
            )
        })
        .min_by(|a, b| {
            a.source_pos
                .distance_squared(soul_pos)
                .total_cmp(&b.source_pos.distance_squared(soul_pos))
        })
}

/// All rejection happens during preparation. Commit the loop-local claim before deferred writes.
pub(in crate::soul_ai::execute::task_execution) fn commit_gather_haul_segment(
    prepared: PreparedGatherHaulSegment,
    ctx: &mut TaskExecutionContext,
    commands: &mut Commands,
) -> TaskHandlerControl {
    ctx.chain_shadow.sources.insert(prepared.item);
    *ctx.chain_shadow
        .inbound
        .entry((prepared.destination, prepared.resource))
        .or_default() += 1;
    for op in lifecycle::collect_active_reservation_ops(&prepared.task, |_, fallback| fallback) {
        ctx.queue_reservation(op);
    }
    crate::soul_ai::execute::task_assignment_apply::attach_delivering_to_relationship(
        commands,
        &prepared.task,
    );
    commands
        .entity(ctx.soul_entity)
        .insert(WorkingOn(prepared.item));
    ctx.transition_task_identity(prepared.item, prepared.work_type);
    *ctx.task = prepared.task;
    ctx.dest.0 = prepared.source_pos;
    ctx.path.waypoints.clear();
    ctx.path.current_index = 0;
    ctx.path.planned_destination = None;
    TaskHandlerControl::Continue
}
