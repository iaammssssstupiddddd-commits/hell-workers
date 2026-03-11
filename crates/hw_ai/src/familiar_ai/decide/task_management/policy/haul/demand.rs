use bevy::prelude::*;
use hw_core::logistics::ResourceType;
use hw_logistics::floor_construction::floor_site_tile_demand_from_index;
use hw_logistics::provisional_wall::provisional_wall_mud_demand;
use hw_logistics::tile_index::TileSiteIndex;
use hw_logistics::wall_construction::wall_site_tile_demand_from_index;

use crate::familiar_ai::decide::task_management::{
    FamiliarTaskAssignmentQueries, IncomingDeliverySnapshot, ReservationShadow,
};

type TaskAssignmentQueries<'w, 's> = FamiliarTaskAssignmentQueries<'w, 's>;

pub struct DemandReadContext<'a, 'w, 's> {
    pub queries: &'a TaskAssignmentQueries<'w, 's>,
    pub shadow: &'a ReservationShadow,
    pub tile_site_index: &'a TileSiteIndex,
    pub incoming_snapshot: &'a IncomingDeliverySnapshot,
}

impl<'a, 'w, 's> DemandReadContext<'a, 'w, 's> {
    pub fn new(
        queries: &'a TaskAssignmentQueries<'w, 's>,
        shadow: &'a ReservationShadow,
        tile_site_index: &'a TileSiteIndex,
        incoming_snapshot: &'a IncomingDeliverySnapshot,
    ) -> Self {
        Self {
            queries,
            shadow,
            tile_site_index,
            incoming_snapshot,
        }
    }
}

pub fn compute_remaining_blueprint_amount(
    blueprint: Entity,
    resource_type: ResourceType,
    context: &DemandReadContext<'_, '_, '_>,
) -> u32 {
    let Ok((_, blueprint_comp, _)) = context.queries.storage.blueprints.get(blueprint) else {
        return 0;
    };

    if let Some(flexible) = &blueprint_comp.flexible_material_requirement
        && flexible.accepts(resource_type)
    {
        let incoming =
            count_matching_incoming_deliveries(blueprint, context, |incoming_resource_type| {
                flexible.accepted_types.contains(&incoming_resource_type)
            }) + flexible
                .accepted_types
                .iter()
                .map(|accepted_type| {
                    context
                        .shadow
                        .destination_reserved_resource(blueprint, *accepted_type)
                        as u32
                })
                .sum::<u32>();

        return flexible.remaining().saturating_sub(incoming);
    }

    let needed_material = blueprint_comp.remaining_material_amount(resource_type);
    if needed_material == 0 {
        return 0;
    }

    let incoming = count_exact_incoming_deliveries(blueprint, resource_type, context)
        + context
            .shadow
            .destination_reserved_resource(blueprint, resource_type) as u32;
    needed_material.saturating_sub(incoming)
}

pub fn compute_remaining_blueprint_wheelbarrow_amount(
    blueprint: Entity,
    resource_type: ResourceType,
    context: &DemandReadContext<'_, '_, '_>,
) -> u32 {
    compute_remaining_blueprint_amount(blueprint, resource_type, context)
}

pub fn compute_remaining_floor_bones(
    site_entity: Entity,
    context: &DemandReadContext<'_, '_, '_>,
) -> u32 {
    let tile_entities = context
        .tile_site_index
        .floor_tiles_by_site
        .get(&site_entity)
        .map(|tiles| tiles.as_slice())
        .unwrap_or(&[]);
    let base_demand = floor_site_tile_demand_from_index(
        tile_entities,
        &context.queries.storage.floor_tiles,
        ResourceType::Bone,
    );
    compute_remaining_from_incoming(site_entity, base_demand, ResourceType::Bone, context)
}

pub fn compute_remaining_floor_mud(
    site_entity: Entity,
    context: &DemandReadContext<'_, '_, '_>,
) -> u32 {
    let tile_entities = context
        .tile_site_index
        .floor_tiles_by_site
        .get(&site_entity)
        .map(|tiles| tiles.as_slice())
        .unwrap_or(&[]);
    let base_demand = floor_site_tile_demand_from_index(
        tile_entities,
        &context.queries.storage.floor_tiles,
        ResourceType::StasisMud,
    );
    compute_remaining_from_incoming(site_entity, base_demand, ResourceType::StasisMud, context)
}

pub fn compute_remaining_wall_wood(
    site_entity: Entity,
    context: &DemandReadContext<'_, '_, '_>,
) -> u32 {
    let tile_entities = context
        .tile_site_index
        .wall_tiles_by_site
        .get(&site_entity)
        .map(|tiles| tiles.as_slice())
        .unwrap_or(&[]);
    let base_demand = wall_site_tile_demand_from_index(
        tile_entities,
        &context.queries.storage.wall_tiles,
        ResourceType::Wood,
    );
    compute_remaining_from_incoming(site_entity, base_demand, ResourceType::Wood, context)
}

pub fn compute_remaining_wall_mud(
    site_entity: Entity,
    context: &DemandReadContext<'_, '_, '_>,
) -> u32 {
    let tile_entities = context
        .tile_site_index
        .wall_tiles_by_site
        .get(&site_entity)
        .map(|tiles| tiles.as_slice())
        .unwrap_or(&[]);
    let base_demand = wall_site_tile_demand_from_index(
        tile_entities,
        &context.queries.storage.wall_tiles,
        ResourceType::StasisMud,
    );
    compute_remaining_from_incoming(site_entity, base_demand, ResourceType::StasisMud, context)
}

pub fn compute_remaining_stockpile_capacity(
    stockpile_entity: Entity,
    resource_type: ResourceType,
    context: &DemandReadContext<'_, '_, '_>,
) -> u32 {
    let Ok((_, _, stockpile, stored_items_opt)) =
        context.queries.storage.stockpiles.get(stockpile_entity)
    else {
        return 0;
    };
    if stockpile.resource_type.is_some() && stockpile.resource_type != Some(resource_type) {
        return 0;
    }

    let stored = stored_items_opt.map(|items| items.len()).unwrap_or(0);
    let incoming = context
        .incoming_snapshot
        .count_exact(stockpile_entity, resource_type) as usize;
    let shadow_incoming = context.shadow.destination_reserved_total(stockpile_entity);
    stockpile
        .capacity
        .saturating_sub(stored + incoming + shadow_incoming) as u32
}

pub fn compute_remaining_provisional_wall_mud(
    wall_entity: Entity,
    context: &DemandReadContext<'_, '_, '_>,
) -> u32 {
    let Ok((_, building, provisional_opt)) = context.queries.storage.buildings.get(wall_entity)
    else {
        return 0;
    };
    let base_demand = provisional_wall_mud_demand(&building, provisional_opt.as_deref()) as u32;
    compute_remaining_from_incoming(
        wall_entity,
        base_demand as usize,
        ResourceType::StasisMud,
        context,
    )
}

fn compute_remaining_from_incoming(
    anchor_entity: Entity,
    base_demand: usize,
    resource_type: ResourceType,
    context: &DemandReadContext<'_, '_, '_>,
) -> u32 {
    let incoming = count_exact_incoming_deliveries(anchor_entity, resource_type, context)
        + context
            .shadow
            .destination_reserved_resource(anchor_entity, resource_type) as u32;

    base_demand.saturating_sub(incoming as usize) as u32
}

fn count_exact_incoming_deliveries(
    target: Entity,
    resource_type: ResourceType,
    context: &DemandReadContext<'_, '_, '_>,
) -> u32 {
    count_matching_incoming_deliveries(target, context, |incoming_resource_type| {
        incoming_resource_type == resource_type
    })
}

fn count_matching_incoming_deliveries(
    target: Entity,
    context: &DemandReadContext<'_, '_, '_>,
    mut predicate: impl FnMut(ResourceType) -> bool,
) -> u32 {
    context
        .incoming_snapshot
        .iter_counts(target)
        .filter(|(resource_type, _)| predicate(*resource_type))
        .map(|(_, count)| count)
        .sum()
}
