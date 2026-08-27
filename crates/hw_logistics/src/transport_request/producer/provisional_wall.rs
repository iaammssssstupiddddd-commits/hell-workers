//! Provisional wall upgrade transport producer

use bevy::prelude::*;

use hw_core::constants::TILE_SIZE;
use hw_core::relationships::TaskWorkers;
use hw_jobs::construction::WallTileBlueprint;
use hw_jobs::{
    Building, BuildingType, Designation, Priority, ProvisionalWall, TaskSlots, WorkType,
};
use hw_spatial::{ResourceSpatialGrid, SpatialGridOps};

use crate::transport_request::producer::active_unit_cache::{
    CachedActiveFamiliars, CachedActiveYards,
};
use crate::transport_request::{
    TransportDemand, TransportPolicy, TransportPriority, TransportRequest, TransportRequestKind,
    TransportRequestState,
};
use crate::types::{ResourceItem, ResourceType};

const PROVISIONAL_WALL_PRIORITY: u32 = 5;

type WallDesignationQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Building,
        &'static ProvisionalWall,
        Option<&'static Designation>,
        Option<&'static TaskWorkers>,
    ),
>;

fn to_u32_saturating(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

pub fn provisional_wall_auto_haul_system(
    mut commands: Commands,
    familiars_cache: Res<CachedActiveFamiliars>,
    yards_cache: Res<CachedActiveYards>,
    q_walls: Query<(
        Entity,
        &Transform,
        &Building,
        &ProvisionalWall,
        Option<&TaskWorkers>,
    )>,
    q_requests: Query<(
        Entity,
        &TransportRequest,
        Option<&TaskWorkers>,
        super::upsert::ExistingRequestRuntime<'static>,
    )>,
    q_wall_tiles: Query<&WallTileBlueprint>,
) {
    let site_managed_walls: std::collections::HashSet<Entity> = q_wall_tiles
        .iter()
        .filter_map(|tile| tile.spawned_wall)
        .collect();

    let mut in_flight = std::collections::HashMap::<Entity, usize>::new();
    for (_, req, workers_opt, _) in q_requests.iter() {
        if req.kind != TransportRequestKind::DeliverToProvisionalWall {
            continue;
        }
        let workers = workers_opt.map(|w| w.len()).unwrap_or(0);
        if workers > 0 {
            *in_flight.entry(req.anchor).or_insert(0) += workers;
        }
    }

    let active_familiars = &familiars_cache.data;
    let active_yards = &yards_cache.data;
    let all_owners = super::collect_all_area_owners(active_familiars, active_yards);

    let mut desired_requests = std::collections::HashMap::<Entity, (Entity, Vec2, u32)>::new();
    for (wall_entity, wall_transform, building, provisional, workers_opt) in q_walls.iter() {
        if site_managed_walls.contains(&wall_entity) {
            continue;
        }
        if building.kind != BuildingType::Wall
            || !building.is_provisional
            || provisional.mud_delivered
        {
            continue;
        }

        if workers_opt.map(|w| w.len()).unwrap_or(0) > 0 {
            continue;
        }

        let wall_pos = wall_transform.translation.truncate();
        let Some((fam_entity, _)) = super::find_owner(wall_pos, &all_owners) else {
            continue;
        };

        let inflight = *in_flight.get(&wall_entity).unwrap_or(&0);
        if inflight >= 1 {
            continue;
        }

        desired_requests.insert(wall_entity, (fam_entity, wall_pos, 1));
    }

    let mut seen_existing = std::collections::HashSet::<Entity>::new();
    for (request_entity, request, workers_opt, current) in q_requests.iter() {
        if request.kind != TransportRequestKind::DeliverToProvisionalWall {
            continue;
        }

        let key = request.anchor;
        let workers = workers_opt.map(|w| w.len()).unwrap_or(0);
        if !super::upsert::process_duplicate_key(
            &mut commands,
            request_entity,
            workers,
            &mut seen_existing,
            key,
        ) {
            continue;
        }

        let inflight = to_u32_saturating(workers);
        if let Some((issued_by, wall_pos, slots)) = desired_requests.get(&key) {
            super::upsert::update_request_runtime_if_needed(
                &mut commands,
                request_entity,
                request,
                current,
                super::upsert::SemanticRequestSpec {
                    key: (key, ResourceType::StasisMud),
                    site_pos: *wall_pos,
                    issued_by: *issued_by,
                    desired_slots: *slots,
                    inflight,
                    priority: PROVISIONAL_WALL_PRIORITY,
                    transport_priority: TransportPriority::Low,
                    kind: TransportRequestKind::DeliverToProvisionalWall,
                    work_type: WorkType::Haul,
                    state: super::upsert::request_state_for_workers(workers),
                },
            );
            continue;
        }

        super::upsert::disable_request_if_needed(
            &mut commands,
            request_entity,
            current,
            Some(inflight),
        );
    }

    for (wall_entity, (issued_by, wall_pos, slots)) in desired_requests {
        if seen_existing.contains(&wall_entity) {
            continue;
        }

        commands.spawn((
            Name::new("TransportRequest::DeliverToProvisionalWall"),
            Transform::from_xyz(wall_pos.x, wall_pos.y, 0.0),
            Visibility::Hidden,
            Designation {
                work_type: WorkType::Haul,
            },
            hw_core::relationships::ManagedBy(issued_by),
            TaskSlots::new(slots),
            Priority(PROVISIONAL_WALL_PRIORITY),
            TransportRequest {
                kind: TransportRequestKind::DeliverToProvisionalWall,
                anchor: wall_entity,
                resource_type: ResourceType::StasisMud,
                issued_by,
                priority: TransportPriority::Low,
                stockpile_group: vec![],
            },
            TransportDemand {
                desired_slots: slots,
                inflight: 0,
            },
            TransportRequestState::Pending,
            TransportPolicy::default(),
        ));
    }
}

pub fn provisional_wall_material_delivery_sync_system(
    mut commands: Commands,
    mut q_walls: Query<(Entity, &Transform, &Building, &mut ProvisionalWall)>,
    q_resources: Query<(
        Entity,
        &Transform,
        &Visibility,
        &ResourceItem,
        Option<&hw_core::relationships::StoredIn>,
    )>,
    q_wall_tiles: Query<&WallTileBlueprint>,
    resource_grid: Res<ResourceSpatialGrid>,
    mut nearby_resources: Local<Vec<Entity>>,
    mut consumption_shadow: ResMut<super::ConstructionMaterialConsumptionShadow>,
) {
    let site_managed_walls: std::collections::HashSet<Entity> = q_wall_tiles
        .iter()
        .filter_map(|tile| tile.spawned_wall)
        .collect();

    let pickup_radius_sq = (TILE_SIZE * 1.5) * (TILE_SIZE * 1.5);

    for (wall_entity, wall_transform, building, mut provisional) in q_walls.iter_mut() {
        if site_managed_walls.contains(&wall_entity) {
            continue;
        }
        if building.kind != BuildingType::Wall
            || !building.is_provisional
            || provisional.mud_delivered
        {
            continue;
        }

        let wall_pos = wall_transform.translation.truncate();
        resource_grid.get_nearby_in_radius_into(wall_pos, TILE_SIZE * 1.5, &mut nearby_resources);
        let nearest_mud = nearby_resources
            .iter()
            .copied()
            .filter(|entity| !consumption_shadow.consumed.contains(entity))
            .filter_map(|entity| {
                let Ok((_, transform, visibility, item, stored_in_opt)) = q_resources.get(entity)
                else {
                    return None;
                };
                (*visibility != Visibility::Hidden
                    && stored_in_opt.is_none()
                    && item.0 == ResourceType::StasisMud)
                    .then_some((
                        entity,
                        transform.translation.truncate().distance_squared(wall_pos),
                    ))
            })
            .filter(|(_, distance_squared)| *distance_squared <= pickup_radius_sq)
            .min_by(|(entity_a, distance_a), (entity_b, distance_b)| {
                distance_a
                    .total_cmp(distance_b)
                    .then_with(|| entity_a.to_bits().cmp(&entity_b.to_bits()))
            })
            .map(|(entity, _)| entity);

        if let Some(mud_entity) = nearest_mud {
            consumption_shadow.consumed.insert(mud_entity);
            commands.entity(mud_entity).try_despawn();
            provisional.mud_delivered = true;
            debug!(
                "PROVISIONAL_WALL: Wall {:?} received StasisMud and is ready to coat",
                wall_entity
            );
        }
    }
}

pub fn provisional_wall_designation_system(
    mut commands: Commands,
    q_walls: WallDesignationQuery,
    q_wall_tiles: Query<&WallTileBlueprint>,
) {
    let site_managed_walls: std::collections::HashSet<Entity> = q_wall_tiles
        .iter()
        .filter_map(|tile| tile.spawned_wall)
        .collect();

    for (wall_entity, building, provisional, designation_opt, workers_opt) in q_walls.iter() {
        if site_managed_walls.contains(&wall_entity) {
            continue;
        }
        if building.kind != BuildingType::Wall {
            continue;
        }

        let should_designate = building.is_provisional && provisional.mud_delivered;
        let has_workers = workers_opt.map(|workers| workers.len()).unwrap_or(0) > 0;
        let is_coat_designation = designation_opt
            .map(|designation| designation.work_type == WorkType::CoatWall)
            .unwrap_or(false);

        if should_designate {
            if !is_coat_designation {
                commands.entity(wall_entity).insert((
                    Designation {
                        work_type: WorkType::CoatWall,
                    },
                    TaskSlots::new(1),
                    Priority(PROVISIONAL_WALL_PRIORITY),
                ));
            }
            continue;
        }

        if is_coat_designation && !has_workers {
            commands
                .entity(wall_entity)
                .remove::<Designation>()
                .remove::<TaskSlots>()
                .remove::<Priority>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_distance_delivery_uses_entity_bits_tie_break() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<ResourceSpatialGrid>()
            .init_resource::<super::super::ConstructionMaterialConsumptionShadow>()
            .add_systems(Update, provisional_wall_material_delivery_sync_system);
        let center = Vec2::new(64.0, 64.0);
        let wall = app
            .world_mut()
            .spawn((
                Building {
                    kind: BuildingType::Wall,
                    is_provisional: true,
                },
                ProvisionalWall::default(),
                Transform::from_translation(center.extend(0.0)),
            ))
            .id();
        let first = app
            .world_mut()
            .spawn((
                ResourceItem(ResourceType::StasisMud),
                Transform::from_translation((center + Vec2::X).extend(0.0)),
                Visibility::Visible,
            ))
            .id();
        let second = app
            .world_mut()
            .spawn((
                ResourceItem(ResourceType::StasisMud),
                Transform::from_translation((center - Vec2::X).extend(0.0)),
                Visibility::Visible,
            ))
            .id();
        {
            let mut grid = app.world_mut().resource_mut::<ResourceSpatialGrid>();
            grid.insert(first, center + Vec2::X);
            grid.insert(second, center - Vec2::X);
        }
        let (expected, remaining) = if first.to_bits() < second.to_bits() {
            (first, second)
        } else {
            (second, first)
        };

        app.update();

        assert!(app.world().get_entity(expected).is_err());
        assert!(app.world().get_entity(remaining).is_ok());
        assert!(
            app.world()
                .entity(wall)
                .get::<ProvisionalWall>()
                .unwrap()
                .mud_delivered
        );
    }
}
