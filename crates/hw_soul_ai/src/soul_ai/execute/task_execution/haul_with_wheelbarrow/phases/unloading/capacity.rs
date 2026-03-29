use crate::soul_ai::execute::task_execution::context::TaskExecutionContext;
use hw_logistics::ResourceType;
use hw_logistics::{
    count_nearby_ground_resources as count_nearby_ground_items, floor_site_tile_demand,
    provisional_wall_mud_demand, wall_site_tile_demand,
};

pub(super) fn floor_site_remaining(
    ctx: &TaskExecutionContext,
    site_entity: bevy::prelude::Entity,
    resource_type: ResourceType,
) -> usize {
    let Ok((_, site, _)) = ctx.queries.storage.floor_sites.get(site_entity) else {
        return 0;
    };

    let needed = floor_site_tile_demand(
        ctx.queries.storage.floor_tiles.iter().map(|(_, t, _)| t),
        site_entity,
        resource_type,
    );
    let nearby = count_nearby_ground_items(
        ctx.queries.resource_items.iter(),
        site.material_center,
        (hw_core::constants::TILE_SIZE * 2.0).powi(2),
        resource_type,
        None,
    );
    needed.saturating_sub(nearby)
}

pub(super) fn wall_site_remaining(
    ctx: &TaskExecutionContext,
    site_entity: bevy::prelude::Entity,
    resource_type: ResourceType,
) -> usize {
    let Ok((_, site, _)) = ctx.queries.storage.wall_sites.get(site_entity) else {
        return 0;
    };

    let needed = wall_site_tile_demand(
        ctx.queries.storage.wall_tiles.iter().map(|(_, t, _)| t),
        site_entity,
        resource_type,
    );
    let nearby = count_nearby_ground_items(
        ctx.queries.resource_items.iter(),
        site.material_center,
        (hw_core::constants::TILE_SIZE * 2.0).powi(2),
        resource_type,
        None,
    );
    needed.saturating_sub(nearby)
}

pub(super) fn provisional_wall_remaining(
    ctx: &TaskExecutionContext,
    wall_entity: bevy::prelude::Entity,
    resource_type: ResourceType,
) -> usize {
    let Ok((wall_transform, building, provisional_opt)) =
        ctx.queries.storage.buildings.get(wall_entity)
    else {
        return 0;
    };
    if resource_type != ResourceType::StasisMud
        || provisional_wall_mud_demand(building, provisional_opt) == 0
    {
        return 0;
    }

    1usize.saturating_sub(count_nearby_ground_items(
        ctx.queries.resource_items.iter(),
        wall_transform.translation.truncate(),
        (hw_core::constants::TILE_SIZE * 1.5).powi(2),
        ResourceType::StasisMud,
        None,
    ))
}
