pub mod blueprint;
pub mod bucket;
pub mod consolidation;
pub mod floor_construction;
pub mod mixer;
pub mod mixer_helpers;
pub mod provisional_wall;
pub mod stockpile_group;
pub mod tank_water_request;
pub mod task_area;
pub mod upsert;
pub mod wall_construction;
pub mod wheelbarrow;

use bevy::math::Vec2;
use bevy::prelude::{Commands, Entity, Query, Transform, Visibility};
use hw_world::zones::{AreaBounds, Yard};
use std::collections::HashMap;

use crate::transport_request::producer::upsert::{SpawnRequestSpec, UpsertRequestSpec};
use crate::transport_request::{TransportRequest, TransportRequestKind};
use crate::types::{ResourceItem, ResourceType};
use hw_spatial::{ResourceSpatialGrid, SpatialGridOps};

pub fn to_u32_saturating(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

pub fn collect_all_area_owners(
    familiars: &[(Entity, AreaBounds)],
    yards: &[(Entity, Yard)],
) -> Vec<(Entity, AreaBounds)> {
    let mut all = familiars.to_vec();
    for (yard_entity, yard) in yards {
        all.push((*yard_entity, yard.bounds()));
    }
    all
}

pub fn find_owner(pos: Vec2, owners: &[(Entity, AreaBounds)]) -> Option<(Entity, &AreaBounds)> {
    owners
        .iter()
        .filter(|(_, area)| area.contains(pos))
        .min_by(|(_, area1), (_, area2)| {
            let d1 = area1.center().distance_squared(pos);
            let d2 = area2.center().distance_squared(pos);
            d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(entity, area)| (*entity, area))
}

pub fn find_owner_yard(pos: Vec2, yards: &[(Entity, Yard)]) -> Option<(Entity, &Yard)> {
    yards
        .iter()
        .filter(|(_, yard)| yard.contains(pos))
        .min_by(|(_, a), (_, b)| {
            let da = a.min.distance_squared(pos) + a.max.distance_squared(pos);
            let db = b.min.distance_squared(pos) + b.max.distance_squared(pos);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(entity, yard)| (*entity, yard))
}

pub fn find_owner_for_position<'a>(
    pos: Vec2,
    owners: &'a [(Entity, AreaBounds)],
    yards: &'a [(Entity, Yard)],
) -> Option<(Entity, &'a AreaBounds)> {
    if let Some((_yard_entity, yard)) = find_owner_yard(pos, yards) {
        let yard_center = (yard.min + yard.max) * 0.5;
        if let Some(result) = owners
            .iter()
            .filter(|(_, area)| area.contains(yard_center))
            .min_by(|(_, area_a), (_, area_b)| {
                let da = area_a.center().distance_squared(yard_center);
                let db = area_b.center().distance_squared(yard_center);
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(entity, area)| (*entity, area))
        {
            return Some(result);
        }
    }

    find_owner(pos, owners)
}

/// `collect_nearby_resource_entities` の検索条件をまとめた構造体。
pub struct NearbyResourceSpec {
    pub center: Vec2,
    pub pickup_radius: f32,
    pub target_resource: ResourceType,
}

pub fn collect_nearby_resource_entities(
    spec: NearbyResourceSpec,
    resource_grid: &ResourceSpatialGrid,
    q_resources: &Query<(
        Entity,
        &Transform,
        &Visibility,
        &ResourceItem,
        Option<&hw_core::relationships::StoredIn>,
    )>,
    scratch: &mut Vec<Entity>,
    resources_scanned: &mut u32,
) -> Vec<Entity> {
    let pickup_radius_sq = spec.pickup_radius * spec.pickup_radius;
    let mut nearby_resources = Vec::new();
    resource_grid.get_nearby_in_radius_into(spec.center, spec.pickup_radius, scratch);
    for entity in scratch.iter().copied() {
        let Ok((_, transform, visibility, resource_item, stored_in_opt)) = q_resources.get(entity)
        else {
            continue;
        };
        *resources_scanned = resources_scanned.saturating_add(1);
        if *visibility != Visibility::Hidden
            && stored_in_opt.is_none()
            && resource_item.0 == spec.target_resource
            && transform
                .translation
                .truncate()
                .distance_squared(spec.center)
                <= pickup_radius_sq
        {
            nearby_resources.push(entity);
        }
    }
    nearby_resources
}

pub fn group_tiles_by_site<T: bevy::prelude::Component>(
    q_tiles: &Query<(Entity, &T)>,
    mut parent_site_of: impl FnMut(&T) -> Entity,
    tiles_scanned: &mut u32,
) -> HashMap<Entity, Vec<Entity>> {
    let mut tiles_by_site = HashMap::<Entity, Vec<Entity>>::new();
    for (tile_entity, tile) in q_tiles.iter() {
        *tiles_scanned = tiles_scanned.saturating_add(1);
        tiles_by_site
            .entry(parent_site_of(tile))
            .or_default()
            .push(tile_entity);
    }
    tiles_by_site
}

/// `consume_waiting_tile_resources` の非ジェネリック引数をまとめた構造体。
pub struct TileConsumeSpec<'a> {
    pub site_tiles: &'a [Entity],
    pub required_amount: u32,
}

pub fn consume_waiting_tile_resources<
    T: bevy::prelude::Component<Mutability = bevy::ecs::component::Mutable>,
>(
    commands: &mut Commands,
    spec: TileConsumeSpec<'_>,
    q_tiles: &mut Query<&mut T>,
    nearby_resources: &mut Vec<Entity>,
    mut is_waiting: impl FnMut(&T) -> bool,
    mut delivered_mut: impl FnMut(&mut T) -> &mut u32,
    mut mark_ready: impl FnMut(&mut T),
) -> u32 {
    let mut consumed = 0u32;
    for tile_entity in spec.site_tiles.iter().copied() {
        let Ok(mut tile) = q_tiles.get_mut(tile_entity) else {
            continue;
        };
        if !is_waiting(&tile) {
            continue;
        }

        let reached_required = {
            let delivered = delivered_mut(&mut tile);
            while *delivered < spec.required_amount {
                let Some(resource_entity) = nearby_resources.pop() else {
                    break;
                };
                commands.entity(resource_entity).try_despawn();
                *delivered += 1;
                consumed += 1;
            }
            *delivered >= spec.required_amount
        };

        if reached_required {
            mark_ready(&mut tile);
        }
        if nearby_resources.is_empty() {
            break;
        }
    }
    consumed
}

/// `sync_construction_delivery` のサイト固有データ＋検索補助バッファをまとめた構造体。
pub struct ConstructionDeliverySpec<'a> {
    pub site_entity: Entity,
    pub site_pos: Vec2,
    pub target_resource: ResourceType,
    pub required_amount: u32,
    pub pickup_radius: f32,
    pub resource_grid: &'a ResourceSpatialGrid,
    pub scratch: &'a mut Vec<Entity>,
    pub resources_scanned: &'a mut u32,
    pub tiles_by_site: &'a HashMap<Entity, Vec<Entity>>,
}

pub fn sync_construction_delivery<
    TTile: bevy::prelude::Component<Mutability = bevy::ecs::component::Mutable>,
>(
    commands: &mut Commands,
    spec: ConstructionDeliverySpec<'_>,
    q_resources: &Query<(
        Entity,
        &Transform,
        &Visibility,
        &ResourceItem,
        Option<&hw_core::relationships::StoredIn>,
    )>,
    q_tiles: &mut Query<&mut TTile>,
    is_waiting: impl FnMut(&TTile) -> bool,
    delivered_mut: impl FnMut(&mut TTile) -> &mut u32,
    mark_ready: impl FnMut(&mut TTile),
) -> u32 {
    let mut nearby_resources = collect_nearby_resource_entities(
        NearbyResourceSpec {
            center: spec.site_pos,
            pickup_radius: spec.pickup_radius,
            target_resource: spec.target_resource,
        },
        spec.resource_grid,
        q_resources,
        spec.scratch,
        spec.resources_scanned,
    );

    if nearby_resources.is_empty() {
        return 0;
    }

    let Some(site_tiles) = spec.tiles_by_site.get(&spec.site_entity) else {
        return 0;
    };

    consume_waiting_tile_resources(
        commands,
        TileConsumeSpec {
            site_tiles,
            required_amount: spec.required_amount,
        },
        q_tiles,
        &mut nearby_resources,
        is_waiting,
        delivered_mut,
        mark_ready,
    )
}

/// `sync_construction_requests` のリクエスト種別定義をまとめた構造体。
pub struct RequestSyncSpec {
    pub expected_kind: TransportRequestKind,
    pub request_name: &'static str,
    pub request_kind: TransportRequestKind,
}

pub fn sync_construction_requests<TTarget: bevy::prelude::Component>(
    commands: &mut Commands,
    q_requests: &Query<(
        Entity,
        &TTarget,
        &TransportRequest,
        Option<&hw_core::relationships::TaskWorkers>,
    )>,
    desired_requests: &HashMap<(Entity, ResourceType), (Entity, u32, Vec2)>,
    spec: RequestSyncSpec,
    target_entity: impl Fn(&TTarget) -> Entity,
    build_target: impl Fn(Entity) -> TTarget,
    priority_for: impl Fn(ResourceType) -> u32,
) -> std::collections::HashSet<(Entity, ResourceType)> {
    let mut seen_existing_keys = std::collections::HashSet::<(Entity, ResourceType)>::new();

    for (request_entity, target, request, workers_opt) in q_requests.iter() {
        if request.kind != spec.expected_kind {
            continue;
        }

        let key = (target_entity(target), request.resource_type);
        let workers = workers_opt.map(|w| w.len()).unwrap_or(0);
        if !upsert::process_duplicate_key(
            commands,
            request_entity,
            workers,
            &mut seen_existing_keys,
            key,
        ) {
            continue;
        }

        let inflight = to_u32_saturating(workers);
        if let Some((issued_by, slots, site_pos)) = desired_requests.get(&key) {
            upsert::upsert_transport_request(
                commands,
                request_entity,
                UpsertRequestSpec {
                    key,
                    site_pos: *site_pos,
                    issued_by: *issued_by,
                    desired_slots: *slots,
                    inflight,
                    priority: priority_for(key.1),
                    target: build_target(key.0),
                    kind: spec.request_kind,
                    work_type: hw_jobs::WorkType::Haul,
                },
            );
            continue;
        }

        upsert::disable_request_with_demand(commands, request_entity, inflight);
    }

    for (key, (issued_by, slots, site_pos)) in desired_requests.iter() {
        if seen_existing_keys.contains(key) {
            continue;
        }

        upsert::spawn_transport_request(
            commands,
            SpawnRequestSpec {
                name: spec.request_name,
                key: *key,
                site_pos: *site_pos,
                issued_by: *issued_by,
                desired_slots: *slots,
                priority: priority_for(key.1),
                target: build_target(key.0),
                kind: spec.request_kind,
                work_type: hw_jobs::WorkType::Haul,
            },
        );
    }

    seen_existing_keys
}
