use super::*;

/// Restores runtime obstacle provenance and derives the raw bitmap from durable
/// load state. `WorldMap.obstacles` is a cache, not a save-format authority.
pub(super) fn rehydrate_obstacle_runtime(world: &mut World) {
    let (natural_owners, natural_blockers) = restore_natural_obstacle_sources(world);
    let (curing_tiles, mut blockers) = restore_curing_floor_protection(world);
    blockers.extend(natural_blockers);
    despawn_incomplete_move_designations(world);
    discard_non_durable_obstacle_markers(world, &natural_owners, &curing_tiles);

    let map_sources = collect_world_map_obstacle_sources(world);
    blockers.extend(map_sources.blockers.iter().copied());
    apply_world_map_obstacle_sources(world, &map_sources, &blockers);
    spawn_building_obstacle_mirrors(world, &map_sources.building_mirrors);

    // Marker/source restoration precedes seeding so the first runtime removal
    // has an old position and provenance even when no Added event is visible.
    seed_obstacle_position_index(world);
}

fn restore_natural_obstacle_sources(world: &mut World) -> (HashSet<Entity>, HashSet<(i32, i32)>) {
    let natural_markers: Vec<(Entity, Option<(i32, i32)>)> = {
        let mut query = world
            .query_filtered::<(Entity, Option<&ObstaclePosition>), Or<(With<Tree>, With<Rock>)>>();
        query
            .iter(world)
            .map(|(entity, position)| (entity, position.map(|position| (position.0, position.1))))
            .collect()
    };

    let mut natural_owners = HashSet::new();
    let mut blockers = HashSet::new();
    for (entity, position) in natural_markers {
        if let Some(grid) = position {
            world
                .entity_mut(entity)
                .insert(ObstacleSourceKind::NaturalTerrainClearing);
            natural_owners.insert(entity);
            blockers.insert(grid);
        } else {
            warn!(
                "REHYDRATE: natural obstacle {entity:?} has no ObstaclePosition; skipping blocker recovery"
            );
        }
    }
    (natural_owners, blockers)
}

fn restore_curing_floor_protection(world: &mut World) -> (HashSet<Entity>, HashSet<(i32, i32)>) {
    let curing_sites: HashSet<Entity> = {
        let mut query = world.query::<(Entity, &FloorConstructionSite)>();
        query
            .iter(world)
            .filter(|(_, site)| site.phase == FloorConstructionPhase::Curing)
            .map(|(entity, _)| entity)
            .collect()
    };
    let floor_tiles: Vec<(Entity, Entity, (i32, i32))> = {
        let mut query = world.query::<(Entity, &FloorTileBlueprint)>();
        query
            .iter(world)
            .map(|(entity, tile)| (entity, tile.parent_site, tile.grid_pos))
            .collect()
    };

    let mut curing_tiles = HashSet::new();
    let mut blockers = HashSet::new();
    for (tile_entity, site_entity, grid) in floor_tiles {
        if curing_sites.contains(&site_entity) {
            world.entity_mut(tile_entity).insert((
                ObstaclePosition(grid.0, grid.1),
                ObstacleSourceKind::ConstructionProtection,
            ));
            curing_tiles.insert(tile_entity);
            blockers.insert(grid);
        } else {
            world
                .entity_mut(tile_entity)
                .remove::<(ObstaclePosition, ObstacleSourceKind)>();
        }
    }
    (curing_tiles, blockers)
}

fn despawn_incomplete_move_designations(world: &mut World) {
    let move_designations: Vec<Entity> = {
        let mut query = world.query::<(Entity, &Designation)>();
        query
            .iter(world)
            .filter(|(_, designation)| designation.work_type == WorkType::Move)
            .map(|(entity, _)| entity)
            .collect()
    };

    for entity in move_designations {
        world.despawn(entity);
    }
}

fn discard_non_durable_obstacle_markers(
    world: &mut World,
    natural_owners: &HashSet<Entity>,
    curing_tiles: &HashSet<Entity>,
) {
    let source_markers: Vec<(Entity, ObstacleSourceKind)> = {
        let mut query = world.query::<(Entity, &ObstacleSourceKind)>();
        query
            .iter(world)
            .map(|(entity, source)| (entity, *source))
            .collect()
    };

    for (entity, source) in source_markers {
        let keep = match source {
            ObstacleSourceKind::NaturalTerrainClearing => natural_owners.contains(&entity),
            ObstacleSourceKind::ConstructionProtection => curing_tiles.contains(&entity),
            ObstacleSourceKind::BuildingFootprint | ObstacleSourceKind::PlacementReservation => {
                false
            }
        };
        if keep {
            continue;
        }

        match source {
            ObstacleSourceKind::BuildingFootprint | ObstacleSourceKind::PlacementReservation => {
                world.despawn(entity);
            }
            ObstacleSourceKind::NaturalTerrainClearing
            | ObstacleSourceKind::ConstructionProtection => {
                world
                    .entity_mut(entity)
                    .remove::<(ObstaclePosition, ObstacleSourceKind)>();
            }
        }
    }

    // No other persisted entity is an obstacle source. Dropping stale marker
    // data prevents a pre-M4 save from reviving an incomplete reservation.
    let unclassified_markers: Vec<Entity> = {
        let mut query =
            world.query_filtered::<Entity, (With<ObstaclePosition>, Without<ObstacleSourceKind>)>();
        query.iter(world).collect()
    };
    for entity in unclassified_markers {
        world.entity_mut(entity).remove::<ObstaclePosition>();
    }
}

struct WorldMapObstacleSources {
    blockers: HashSet<(i32, i32)>,
    building_mirrors: Vec<(Entity, (i32, i32))>,
    stale_building_entries: Vec<(i32, i32)>,
    doors: HashMap<(i32, i32), (Entity, DoorState)>,
    bridged_tiles: HashSet<(i32, i32)>,
}

fn collect_world_map_obstacle_sources(world: &World) -> WorldMapObstacleSources {
    let map_entries: Vec<((i32, i32), Entity)> = world
        .resource::<WorldMap>()
        .building_entries()
        .map(|(&grid, &entity)| (grid, entity))
        .collect();
    let saved_door_states = world.resource::<WorldMap>().door_states.clone();

    let mut sources = WorldMapObstacleSources {
        blockers: HashSet::new(),
        building_mirrors: Vec::new(),
        stale_building_entries: Vec::new(),
        doors: HashMap::new(),
        bridged_tiles: HashSet::new(),
    };

    for (grid, owner) in map_entries {
        if let Some(building) = world.get::<Building>(owner) {
            if building.kind == BuildingType::Bridge {
                sources.bridged_tiles.insert(grid);
            }
            if building.kind.blocks_movement() {
                sources.blockers.insert(grid);
                sources.building_mirrors.push((owner, grid));
            }
            if building.kind == BuildingType::Door {
                let state = saved_door_states
                    .get(&grid)
                    .copied()
                    .or_else(|| world.get::<Door>(owner).map(|door| door.state))
                    .unwrap_or(DoorState::Closed);
                sources.doors.insert(grid, (owner, state));
            }
            continue;
        }

        if let Some(blueprint) = world.get::<Blueprint>(owner) {
            if blueprint.kind != BuildingType::Bridge {
                sources.blockers.insert(grid);
            }
            continue;
        }

        if world.get::<WallConstructionSite>(owner).is_some() {
            sources.blockers.insert(grid);
            continue;
        }

        warn!("REHYDRATE: dropping stale WorldMap building entry at {grid:?} for {owner:?}");
        sources.stale_building_entries.push(grid);
    }
    sources
}

fn apply_world_map_obstacle_sources(
    world: &mut World,
    sources: &WorldMapObstacleSources,
    blockers: &HashSet<(i32, i32)>,
) {
    for &(owner, state) in sources.doors.values() {
        if let Some(mut door) = world.get_mut::<Door>(owner) {
            door.state = state;
        }
    }

    let mut world_map = world.resource_mut::<WorldMap>();
    for grid in &sources.stale_building_entries {
        world_map.clear_building(*grid);
    }
    world_map.replace_navigation_caches(blockers, &sources.doors, &sources.bridged_tiles);
}

fn spawn_building_obstacle_mirrors(world: &mut World, mirrors: &[(Entity, (i32, i32))]) {
    for &(owner, (x, y)) in mirrors {
        world.spawn((
            ChildOf(owner),
            ObstaclePosition(x, y),
            ObstacleSourceKind::BuildingFootprint,
            Name::new("Building Obstacle"),
        ));
    }
}
