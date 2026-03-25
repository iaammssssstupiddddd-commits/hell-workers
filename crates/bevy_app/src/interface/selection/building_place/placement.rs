use crate::assets::GameAssets;
use crate::systems::jobs::{Blueprint, Building, BuildingType};
use crate::world::map::{RIVER_Y_MIN, WorldMap, WorldMapRef};
use bevy::prelude::*;
use hw_core::constants::*;
use hw_core::visual_mirror::construction::BlueprintVisualState;
use hw_ui::selection::{
    BuildingPlacementContext, TANK_NEARBY_BUCKET_STORAGE_TILES, bucket_storage_geometry,
    building_geometry, validate_bucket_storage_placement, validate_building_placement,
};
use super::PlacementQueries;

type PlaceBlueprintResult = Option<(Entity, Vec<(i32, i32)>, Vec2)>;

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

/// Attempts to spawn a Blueprint entity for the given building type at the given grid position.
/// Returns `Some((entity, occupied_grids, spawn_pos))` on success, `None` if placement is blocked.
pub(super) fn place_building_blueprint(
    commands: &mut Commands,
    world_map: &mut WorldMap,
    game_assets: &GameAssets,
    building_type: BuildingType,
    grid: (i32, i32),
    pq: &PlacementQueries<'_, '_, '_>,
) -> PlaceBlueprintResult {
    let geometry = building_geometry(building_type, grid, RIVER_Y_MIN);
    let replace_wall_entity = {
        let read_world = WorldMapRef(world_map);
        let ctx = BuildingPlacementContext {
            world: &read_world,
            in_site: pq.q_sites.iter().any(|site| site.contains(geometry.draw_pos)),
            in_yard: pq.q_yards.iter().any(|yard| yard.contains(geometry.draw_pos)),
            is_wall_or_door_at: &|candidate| {
                is_wall_or_door_at(world_map, pq.q_buildings, pq.q_blueprints_by_entity, candidate)
            },
            is_replaceable_wall_at: &|candidate| {
                is_replaceable_wall_at(world_map, pq.q_buildings, candidate)
            },
        };
        let validation = validate_building_placement(&ctx, building_type, grid, &geometry);
        if !validation.can_place {
            return None;
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

    Some((entity, geometry.occupied_grids, geometry.draw_pos))
}

/// Attempts to place the BucketStorage companion for a Tank blueprint.
/// Returns true if placement succeeded.
pub(super) fn try_place_bucket_storage_companion(
    commands: &mut Commands,
    world_map: &mut WorldMap,
    parent_blueprint: Entity,
    parent_occupied_grids: &[(i32, i32)],
    anchor_grid: (i32, i32),
) -> bool {
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
        return false;
    }

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
    }
    true
}
