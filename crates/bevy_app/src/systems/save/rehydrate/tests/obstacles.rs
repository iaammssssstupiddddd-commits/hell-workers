use super::construction::floor_site;
use super::*;

fn building_mirror_count(world: &mut World, owner: Entity, grid: (i32, i32)) -> usize {
    let mut query = world.query::<(&ObstaclePosition, &ObstacleSourceKind, &ChildOf)>();
    query
        .iter(world)
        .filter(|(position, source, parent)| {
            parent.parent() == owner
                && **source == ObstacleSourceKind::BuildingFootprint
                && (position.0, position.1) == grid
        })
        .count()
}

#[test]
fn rebuilds_durable_sources_and_stays_idempotent() {
    let mut world = World::new();
    world.insert_resource(WorldMap::default());

    let tree = world
        .spawn((Tree, TreeVariant(0), ObstaclePosition(3, 4)))
        .id();
    let rock = world.spawn((Rock, ObstaclePosition(5, 6))).id();

    let tank = world
        .spawn(Building {
            kind: BuildingType::Tank,
            is_provisional: false,
        })
        .id();
    let blueprint = world
        .spawn(Blueprint::new(BuildingType::Tank, vec![(9, 10)]))
        .id();
    let bridge = world
        .spawn(Building {
            kind: BuildingType::Bridge,
            is_provisional: false,
        })
        .id();
    let spa = world
        .spawn((
            SoulSpaSite::default(),
            Building {
                kind: BuildingType::SoulSpa,
                is_provisional: false,
            },
        ))
        .id();

    let open_door = world
        .spawn((
            Building {
                kind: BuildingType::Door,
                is_provisional: false,
            },
            Door {
                state: DoorState::Locked,
            },
        ))
        .id();
    let closed_door = world
        .spawn((
            Building {
                kind: BuildingType::Door,
                is_provisional: false,
            },
            Door {
                state: DoorState::Open,
            },
        ))
        .id();
    let locked_door = world
        .spawn((
            Building {
                kind: BuildingType::Door,
                is_provisional: false,
            },
            Door {
                state: DoorState::Open,
            },
        ))
        .id();

    let curing_site = world.spawn(floor_site(FloorConstructionPhase::Curing)).id();
    let curing_tile = world
        .spawn(FloorTileBlueprint::new(curing_site, (18, 19)))
        .id();
    let unfinished_site = world
        .spawn(floor_site(FloorConstructionPhase::Reinforcing))
        .id();
    let unfinished_tile = world
        .spawn((
            FloorTileBlueprint::new(unfinished_site, (20, 21)),
            ObstaclePosition(20, 21),
            ObstacleSourceKind::ConstructionProtection,
        ))
        .id();
    let move_designation = world
        .spawn(Designation {
            work_type: WorkType::Move,
        })
        .id();

    {
        let mut map = world.resource_mut::<WorldMap>();
        map.set_building((7, 8), tank);
        map.set_building((9, 10), blueprint);
        map.set_building((11, 12), bridge);
        map.set_building((13, 14), spa);

        let bridge_idx = map.pos_to_idx(11, 12).unwrap();
        map.set_terrain_at_idx(bridge_idx, TerrainType::River);
        let stale_bridge_idx = map.pos_to_idx(22, 23).unwrap();
        map.set_terrain_at_idx(stale_bridge_idx, TerrainType::River);
        map.bridged_tiles.insert((22, 23));

        for (grid, door, state) in [
            ((14, 15), open_door, DoorState::Open),
            ((15, 16), closed_door, DoorState::Closed),
            ((16, 17), locked_door, DoorState::Locked),
        ] {
            map.set_building(grid, door);
            map.doors.insert(grid, door);
            map.door_states.insert(grid, state);
        }

        map.add_grid_obstacle((11, 12));
        map.add_grid_obstacle((20, 21));
        map.add_grid_obstacle((22, 23));
    }

    rehydrate_obstacle_runtime(&mut world);

    assert_eq!(
        world.get::<ObstacleSourceKind>(tree),
        Some(&ObstacleSourceKind::NaturalTerrainClearing)
    );
    assert_eq!(
        world.get::<ObstacleSourceKind>(rock),
        Some(&ObstacleSourceKind::NaturalTerrainClearing)
    );
    assert_eq!(building_mirror_count(&mut world, tank, (7, 8)), 1);
    assert_eq!(building_mirror_count(&mut world, blueprint, (9, 10)), 0);
    assert_eq!(building_mirror_count(&mut world, bridge, (11, 12)), 0);

    assert_eq!(
        world.get::<ObstacleSourceKind>(curing_tile),
        Some(&ObstacleSourceKind::ConstructionProtection)
    );
    assert!(world.get::<ObstaclePosition>(curing_tile).is_some());
    assert!(world.get::<ObstaclePosition>(unfinished_tile).is_none());
    assert!(world.get_entity(move_designation).is_err());

    {
        let map = world.resource::<WorldMap>();
        assert!(!map.is_walkable(3, 4));
        assert!(!map.is_walkable(5, 6));
        assert!(!map.is_walkable(7, 8));
        assert!(!map.is_walkable(9, 10));
        assert!(map.is_walkable(11, 12));
        assert!(map.bridged_tiles.contains(&(11, 12)));
        assert!(map.is_walkable(13, 14));
        assert!(map.is_walkable(14, 15));
        assert!(map.is_walkable(15, 16));
        assert!(!map.is_walkable(16, 17));
        assert!(!map.is_walkable(18, 19));
        assert!(map.is_walkable(20, 21));
        assert!(!map.is_walkable(22, 23));
        assert!(!map.bridged_tiles.contains(&(22, 23)));

        let open_idx = map.pos_to_idx(14, 15).unwrap();
        let closed_idx = map.pos_to_idx(15, 16).unwrap();
        let locked_idx = map.pos_to_idx(16, 17).unwrap();
        assert!(!map.obstacles[open_idx]);
        assert!(map.obstacles[closed_idx]);
        assert!(map.obstacles[locked_idx]);
    }
    assert_eq!(world.get::<Door>(open_door).unwrap().state, DoorState::Open);
    assert_eq!(
        world.get::<Door>(closed_door).unwrap().state,
        DoorState::Closed
    );
    assert_eq!(
        world.get::<Door>(locked_door).unwrap().state,
        DoorState::Locked
    );

    rehydrate_obstacle_runtime(&mut world);
    assert_eq!(building_mirror_count(&mut world, tank, (7, 8)), 1);
}

#[test]
fn rehydrate_restores_missing_door_cache_and_bumps_topology_once() {
    let mut world = World::new();
    world.insert_resource(WorldMap::default());

    let grid = (24, 25);
    let door = world
        .spawn((
            Building {
                kind: BuildingType::Door,
                is_provisional: false,
            },
            Door {
                state: DoorState::Closed,
            },
        ))
        .id();

    let version_before_rehydrate = {
        let mut map = world.resource_mut::<WorldMap>();
        map.set_building(grid, door);
        map.add_grid_obstacle(grid);
        assert!(!map.is_walkable(grid.0, grid.1));
        map.obstacle_version
    };

    rehydrate_obstacle_runtime(&mut world);

    {
        let map = world.resource::<WorldMap>();
        assert_eq!(map.door_entity(grid.0, grid.1), Some(door));
        assert_eq!(map.door_state(grid.0, grid.1), Some(DoorState::Closed));
        assert!(map.has_raw_obstacle(grid.0, grid.1));
        assert!(map.is_walkable(grid.0, grid.1));
        assert_eq!(map.obstacle_version, version_before_rehydrate + 1);
    }

    rehydrate_obstacle_runtime(&mut world);
    assert_eq!(
        world.resource::<WorldMap>().obstacle_version,
        version_before_rehydrate + 1
    );
}
