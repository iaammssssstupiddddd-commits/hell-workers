use super::PlacementQueries;
use crate::assets::GameAssets;
use crate::systems::jobs::{Blueprint, Building, BuildingType};
use crate::world::map::{WorldMap, WorldMapRef};
use bevy::prelude::*;
use hw_core::constants::*;
use hw_core::visual_mirror::construction::BlueprintVisualState;
use hw_ui::selection::{
    BuildingPlacementContext, PlacementGeometry, PlacementTileRejection, PlacementValidation,
    TANK_NEARBY_BUCKET_STORAGE_TILES, validate_bucket_storage_placement,
    validate_building_placement,
};

use super::super::placement_geometry::{bucket_storage_geometry, live_building_geometry};

type PlaceBlueprintResult = Result<(Entity, Vec<(i32, i32)>, Vec2), PlacementTileRejection>;

fn is_replaceable_wall_at(
    world_map: &WorldMap,
    q_buildings: &Query<&Building>,
    grid: (i32, i32),
) -> bool {
    world_map.building_entity(grid).is_some_and(|entity| {
        q_buildings
            .get(entity)
            .is_ok_and(|building| building.kind == BuildingType::Wall && !building.is_provisional)
    })
}

fn is_wall_or_door_at(
    world_map: &WorldMap,
    q_buildings: &Query<&Building>,
    q_blueprints_by_entity: &Query<&Blueprint>,
    grid: (i32, i32),
) -> bool {
    let Some(entity) = world_map.building_entity(grid) else {
        return false;
    };
    if let Ok(building) = q_buildings.get(entity) {
        return matches!(building.kind, BuildingType::Wall | BuildingType::Door);
    }
    if let Ok(blueprint) = q_blueprints_by_entity.get(entity) {
        return matches!(blueprint.kind, BuildingType::Wall | BuildingType::Door);
    }
    false
}

fn validate_blueprint_geometry(
    world_map: &WorldMap,
    building_type: BuildingType,
    grid: (i32, i32),
    geometry: &PlacementGeometry,
    pq: &PlacementQueries<'_, '_, '_>,
) -> PlacementValidation {
    let read_world = WorldMapRef(world_map);
    let ctx = BuildingPlacementContext {
        world: &read_world,
        in_site: pq
            .q_sites
            .iter()
            .any(|site| site.contains(geometry.draw_pos)),
        in_yard: pq
            .q_yards
            .iter()
            .any(|yard| yard.contains(geometry.draw_pos)),
        is_wall_or_door_at: &|candidate| {
            is_wall_or_door_at(
                world_map,
                pq.q_buildings,
                pq.q_blueprints_by_entity,
                candidate,
            )
        },
        is_replaceable_wall_at: &|candidate| {
            is_replaceable_wall_at(world_map, pq.q_buildings, candidate)
        },
    };
    validate_building_placement(&ctx, building_type, grid, geometry)
}

/// Revalidates a BuildingPlace candidate without mutating WorldMap or spawning a Blueprint.
pub(super) fn validate_building_blueprint_placement(
    world_map: &WorldMap,
    building_type: BuildingType,
    grid: (i32, i32),
    pq: &PlacementQueries<'_, '_, '_>,
) -> PlacementValidation {
    let geometry = live_building_geometry(world_map, building_type, grid);
    validate_blueprint_geometry(world_map, building_type, grid, &geometry, pq)
}

/// Attempts to spawn a Blueprint entity for the given building type at the given grid position.
/// Returns the spawned entity and geometry on success, or the typed rejection when blocked.
pub(super) fn place_building_blueprint(
    commands: &mut Commands,
    world_map: &mut WorldMap,
    game_assets: &GameAssets,
    building_type: BuildingType,
    grid: (i32, i32),
    pq: &PlacementQueries<'_, '_, '_>,
) -> PlaceBlueprintResult {
    let geometry = live_building_geometry(world_map, building_type, grid);
    let replace_wall_entity = {
        let validation = validate_blueprint_geometry(world_map, building_type, grid, &geometry, pq);
        if !validation.can_place {
            return Err(validation
                .rejection(grid)
                .expect("rejected placement must carry a reason"));
        }

        (building_type == BuildingType::Door)
            .then(|| world_map.building_entity(grid))
            .flatten()
            .filter(|_| is_replaceable_wall_at(world_map, pq.q_buildings, grid))
    };

    if let Some(entity) = replace_wall_entity {
        world_map.clear_building_occupancy(grid);
        commands.entity(entity).despawn();
    }

    let texture = match building_type {
        BuildingType::Wall => game_assets.wall_isolated.clone(),
        BuildingType::Door => game_assets.door_closed.clone(),
        BuildingType::Floor => {
            unreachable!("Floor should be placed via Drag-and-drop area selection")
        }
        BuildingType::Tank => game_assets.tank_empty.clone(),
        BuildingType::MudMixer => game_assets.mud_mixer.clone(),
        BuildingType::RestArea => game_assets.rest_area.clone(),
        BuildingType::Bridge => game_assets.bridge.clone(),
        BuildingType::SandPile => game_assets.sand_pile.clone(),
        BuildingType::BonePile => game_assets.bone_pile.clone(),
        BuildingType::WheelbarrowParking => game_assets.wheelbarrow_parking.clone(),
        BuildingType::SoulSpa => game_assets.bone_pile.clone(), // placeholder — SoulSpa uses own spawn
        BuildingType::OutdoorLamp => game_assets.bone_pile.clone(),
    };

    let entity = commands
        .spawn((
            Blueprint::new(building_type, geometry.occupied_grids.clone()),
            BlueprintVisualState::default(),
            crate::systems::jobs::Designation {
                work_type: crate::systems::jobs::WorkType::Build,
            },
            crate::systems::jobs::TaskSlots::new(1),
            Sprite {
                image: texture,
                color: Color::srgba(1.0, 1.0, 1.0, 0.5),
                custom_size: Some(geometry.size),
                ..default()
            },
            Transform::from_xyz(geometry.draw_pos.x, geometry.draw_pos.y, Z_AURA),
            Name::new(format!("Blueprint ({:?})", building_type)),
        ))
        .id();

    world_map.reserve_building_footprint(
        building_type,
        entity,
        geometry.occupied_grids.iter().copied(),
    );

    Ok((entity, geometry.occupied_grids, geometry.draw_pos))
}

/// Attempts to place the BucketStorage companion for a Tank blueprint.
/// Returns the one-entity-per-tile storage roots on success, or the typed rejection from the
/// shared validator.
pub(crate) fn try_place_bucket_storage_companion(
    commands: &mut Commands,
    world_map: &mut WorldMap,
    parent_blueprint: Entity,
    parent_occupied_grids: &[(i32, i32)],
    anchor_grid: (i32, i32),
) -> Result<Vec<Entity>, PlacementTileRejection> {
    let geometry = bucket_storage_geometry(anchor_grid);
    let read_world = WorldMapRef(world_map);
    let validation = validate_bucket_storage_placement(
        &read_world,
        &geometry,
        parent_occupied_grids,
        true,
        TANK_NEARBY_BUCKET_STORAGE_TILES,
    );
    if !validation.can_place {
        return Err(validation
            .rejection(anchor_grid)
            .expect("rejected companion placement must carry a reason"));
    }

    let mut storage_entities = Vec::with_capacity(geometry.occupied_grids.len());
    for (gx, gy) in geometry.occupied_grids {
        let pos = WorldMap::grid_to_world(gx, gy);
        let storage_entity = commands
            .spawn((
                crate::systems::logistics::Stockpile {
                    capacity: 10,
                    resource_type: None,
                },
                crate::systems::logistics::BucketStorage,
                crate::systems::logistics::PendingBelongsToBlueprint(parent_blueprint),
                Sprite {
                    color: Color::srgba(1.0, 1.0, 0.0, 0.2),
                    custom_size: Some(Vec2::splat(TILE_SIZE)),
                    ..default()
                },
                Transform::from_xyz(pos.x, pos.y, Z_MAP + 0.01),
                Name::new("Pending Tank Bucket Storage"),
            ))
            .id();
        world_map.register_stockpile_tile((gx, gy), storage_entity);
        storage_entities.push(storage_entity);
    }
    Ok(storage_entities)
}

#[cfg(test)]
mod bridge_tests {
    use super::super::BuildingStateQueries;
    use super::*;

    #[derive(Resource)]
    struct Attempt {
        grid: (i32, i32),
        result: Option<PlaceBlueprintResult>,
    }

    fn commit(
        mut commands: Commands,
        mut map: ResMut<WorldMap>,
        assets: Res<GameAssets>,
        queries: BuildingStateQueries,
        mut attempt: ResMut<Attempt>,
    ) {
        let pq = PlacementQueries {
            q_buildings: &queries.q_buildings,
            q_blueprints_by_entity: &queries.q_blueprints_by_entity,
            q_sites: &queries.q_sites,
            q_yards: &queries.q_yards,
        };
        attempt.result = Some(place_building_blueprint(
            &mut commands,
            &mut map,
            &assets,
            BuildingType::Bridge,
            attempt.grid,
            &pq,
        ));
    }

    #[test]
    fn bridge_production_commit_reserves_generated_crossing_and_rejects_overlap() {
        use hw_ui::selection::resolve_bridge_crossing;
        let layout = hw_world::generate_world_layout(20260920);
        let mut map = WorldMap::default();
        map.tiles.clone_from(&layout.terrain_tiles);
        for &(x, y) in layout
            .initial_tree_positions
            .iter()
            .chain(&layout.initial_rock_positions)
        {
            map.add_obstacle(x, y);
        }
        let crossing = (0..MAP_WIDTH - 1)
            .find_map(|x| resolve_bridge_crossing(&WorldMapRef(&map), (x, 0)).ok())
            .unwrap();
        let initial: Vec<_> = crossing
            .occupied_grids
            .iter()
            .map(|&(x, y)| map.is_walkable(x, y))
            .collect();
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Image>()
            .init_asset::<Font>()
            .init_asset::<Gltf>()
            .init_asset::<WorldAsset>();
        let server = app.world().resource::<AssetServer>().clone();
        let assets =
            crate::plugins::startup::create_game_assets(&server, &mut Assets::<Image>::default());
        app.insert_resource(assets)
            .insert_resource(map)
            .insert_resource(Attempt {
                grid: (crossing.anchor.0, 0),
                result: None,
            })
            .add_systems(Update, commit);
        // Zone inputs only; logical terrain and blockers remain generated.
        app.world_mut().spawn(hw_world::zones::Site {
            min: WorldMap::grid_to_world(0, 0),
            max: WorldMap::grid_to_world(MAP_WIDTH - 1, MAP_HEIGHT - 1),
        });
        app.world_mut().spawn(hw_world::zones::Yard {
            min: WorldMap::grid_to_world(0, 0),
            max: WorldMap::grid_to_world(MAP_WIDTH - 1, MAP_HEIGHT - 1),
        });
        app.update();
        let (owner, grids, position) = app
            .world_mut()
            .resource_mut::<Attempt>()
            .result
            .take()
            .unwrap()
            .unwrap();
        assert_eq!(grids, crossing.occupied_grids);
        assert_eq!(app.world().get::<Blueprint>(owner).unwrap().progress, 0.0);
        assert_eq!(
            app.world()
                .get::<Transform>(owner)
                .unwrap()
                .translation
                .truncate(),
            position
        );
        let map = app.world().resource::<WorldMap>();
        for (&grid, &walkable) in grids.iter().zip(&initial) {
            assert_eq!(map.building_entity(grid), Some(owner));
            assert!(!map.bridged_tiles.contains(&grid));
            assert_eq!(map.is_walkable(grid.0, grid.1), walkable);
        }
        app.world_mut().resource_mut::<Attempt>().grid.1 = MAP_HEIGHT - 1;
        app.update();
        assert!(
            app.world()
                .resource::<Attempt>()
                .result
                .as_ref()
                .unwrap()
                .is_err()
        );
        let mut blueprints = app.world_mut().query::<&Blueprint>();
        assert_eq!(blueprints.iter(app.world()).count(), 1);
        assert_eq!(
            app.world().resource::<WorldMap>().tiles,
            layout.terrain_tiles
        );
    }
}
