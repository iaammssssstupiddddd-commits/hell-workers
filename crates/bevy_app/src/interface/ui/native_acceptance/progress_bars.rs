//! R12 fixtures use production task/mirror paths; observation never advances them.
use super::*;
use crate::entities::damned_soul::{DamnedSoul, SoulIdentity};
use bevy::ecs::system::RunSystemOnce;
use hw_core::{area::TaskArea, constants::*, relationships::WorkingOn, soul::Destination};
use hw_jobs::construction::{FloorConstructionPhase, FloorTileBlueprint, WallTileBlueprint};
use hw_jobs::{visual_sync::*, *};
use hw_visual::progress_bar::{GenericProgressBar, ProgressBarBackground, ProgressBarFill};
use hw_world::WorldMap;

#[derive(Resource)]
struct Fixture {
    source_grid: (i32, i32),
    blueprint_grid: (i32, i32),
    floor_grid: (i32, i32),
    wall_grid: (i32, i32),
}

fn free_grid(world: &World, point: Vec2) -> (i32, i32) {
    let map = world.resource::<WorldMap>();
    let anchor = WorldMap::world_to_grid(point);
    (-2..=2)
        .flat_map(|dy| (-2..=2).map(move |dx| (anchor.0 + dx, anchor.1 + dy)))
        .find(|&(x, y)| {
            (0..=1).all(|offset| {
                map.is_walkable(x + offset, y)
                    && map.building_entity((x + offset, y)).is_none()
                    && map.floor_entity((x + offset, y)).is_none()
                    && map.stockpile_entity((x + offset, y)).is_none()
            })
        })
        .expect("native bar fixture requires two free adjacent tiles")
}

pub(super) fn prepare(world: &mut World) {
    for mut policy in world
        .query::<&mut hw_core::familiar::FamiliarPolicy>()
        .iter_mut(world)
    {
        policy.set_all_allowed(false);
    }
    for entity in world
        .query_filtered::<Entity, With<crate::entities::familiar::Familiar>>()
        .iter(world)
        .collect::<Vec<_>>()
    {
        world.entity_mut(entity).insert(TaskArea::from_points(
            Vec2::splat(2500.0),
            Vec2::splat(2550.0),
        ));
    }
    let souls: Vec<_> = world
        .query_filtered::<Entity, With<DamnedSoul>>()
        .iter(world)
        .collect();
    let soul = souls[0];
    for entity in souls {
        world
            .entity_mut(entity)
            .insert(AssignedTask::None)
            .remove::<(ActiveTaskIdentity, WorkingOn)>();
    }
    let source_grid = free_grid(world, Vec2::new(-250.0, 120.0));
    let source_pos = WorldMap::grid_to_world(source_grid.0, source_grid.1);
    let source = world
        .spawn((
            Tree,
            TreeVariant(0),
            Designation {
                work_type: WorkType::Chop,
            },
            TaskSlots::new(1),
            Transform::from_translation(source_pos.extend(Z_MAP)),
            ObstaclePosition(source_grid.0, source_grid.1),
            ObstacleSourceKind::NaturalTerrainClearing,
        ))
        .id();
    world
        .resource_mut::<WorldMap>()
        .add_grid_obstacle(source_grid);
    let soul_pos = WorldMap::grid_to_world(source_grid.0 + 1, source_grid.1);
    world.get_mut::<Transform>(soul).unwrap().translation = soul_pos.extend(Z_CHARACTER);
    world.get_mut::<SoulIdentity>(soul).unwrap().name = "ProgressSoul".into();
    world.entity_mut(soul).insert((
        AssignedTask::Gather(GatherData {
            target: source,
            work_type: WorkType::Chop,
            phase: GatherPhase::Collecting { progress: 0.25 },
        }),
        ActiveTaskIdentity::new(source, source, WorkType::Chop),
        WorkingOn(source),
        Destination(soul_pos),
        hw_core::soul::Path::default(),
    ));
    let blueprint_grid = free_grid(world, Vec2::new(-90.0, 120.0));
    let blueprint_pos = WorldMap::grid_to_world(blueprint_grid.0, blueprint_grid.1);
    let mut blueprint = Blueprint::new(BuildingType::Wall, vec![blueprint_grid]);
    blueprint.delivered_materials = blueprint.required_materials.clone();
    blueprint.progress = 0.4;
    let blueprint_mirror = blueprint_visual_state(&blueprint);
    let blueprint_entity = world
        .spawn((
            blueprint,
            blueprint_mirror,
            Designation {
                work_type: WorkType::Build,
            },
            TaskSlots::new(1),
            Transform::from_translation(blueprint_pos.extend(Z_MAP)),
            Visibility::default(),
        ))
        .id();
    world.resource_mut::<WorldMap>().reserve_building_footprint(
        BuildingType::Wall,
        blueprint_entity,
        [blueprint_grid],
    );
    world.spawn((
        ObstaclePosition(blueprint_grid.0, blueprint_grid.1),
        ObstacleSourceKind::PlacementReservation,
        ChildOf(blueprint_entity),
    ));

    let floor_grid = free_grid(world, Vec2::new(80.0, 120.0));
    let floor_pos = WorldMap::grid_to_world(floor_grid.0, floor_grid.1);
    let mut floor = FloorConstructionSite::new(
        TaskArea::from_points(
            floor_pos - Vec2::splat(TILE_SIZE / 2.0),
            floor_pos + Vec2::splat(TILE_SIZE / 2.0),
        ),
        floor_pos,
        1,
    );
    floor.phase = FloorConstructionPhase::Curing;
    floor.tiles_reinforced = 1;
    floor.tiles_poured = 1;
    floor.curing_remaining_secs = FLOOR_CURING_DURATION_SECS * 0.75;
    let floor_mirror = floor_site_visual_state(&floor);
    let floor_entity = world
        .spawn((
            floor,
            floor_mirror,
            Transform::from_translation(floor_pos.extend(Z_MAP)),
            Visibility::default(),
        ))
        .id();
    let mut tile = FloorTileBlueprint::new(floor_entity, floor_grid);
    tile.state = FloorTileState::Complete;
    tile.bones_delivered = FLOOR_BONES_PER_TILE;
    tile.mud_delivered = 1;
    let mirror = floor_tile_visual_mirror(&tile);
    let tile_entity = world
        .spawn((
            tile,
            mirror,
            ObstaclePosition(floor_grid.0, floor_grid.1),
            ObstacleSourceKind::ConstructionProtection,
            Transform::from_translation(floor_pos.extend(Z_MAP)),
            Visibility::default(),
        ))
        .id();
    world.entity_mut(floor_entity).insert(
        crate::systems::jobs::floor_construction::CuringFootprint::from_tile_positions([(
            tile_entity,
            floor_grid,
        )]),
    );
    world
        .resource_mut::<WorldMap>()
        .add_grid_obstacle(floor_grid);

    let wall_grid = free_grid(world, Vec2::new(240.0, 120.0));
    let wall_pos = WorldMap::grid_to_world(wall_grid.0, wall_grid.1);
    let mut wall = WallConstructionSite::new(
        TaskArea::from_points(
            wall_pos - Vec2::splat(TILE_SIZE / 2.0),
            wall_pos + Vec2::new(TILE_SIZE * 1.5, TILE_SIZE / 2.0),
        ),
        wall_pos,
        2,
    );
    wall.tiles_framed = 1;
    let wall_mirror = wall_site_visual_state(&wall);
    let wall_entity = world
        .spawn((
            wall,
            wall_mirror,
            Transform::from_translation(wall_pos.extend(Z_MAP)),
            Visibility::default(),
        ))
        .id();
    for index in 0..2 {
        let grid = (wall_grid.0 + index, wall_grid.1);
        let position = WorldMap::grid_to_world(grid.0, grid.1);
        let mut tile = WallTileBlueprint::new(wall_entity, grid);
        tile.wood_delivered = 1;
        if index == 0 {
            let mut queue = bevy::ecs::world::CommandQueue::default();
            let wall = world.resource_scope(
                |world, handles: Mut<crate::plugins::startup::Building3dHandles>| {
                    crate::systems::jobs::wall_construction::spawn_wall_shell(
                        &mut Commands::new(&mut queue, world),
                        &handles,
                        grid,
                        true,
                    )
                },
            );
            queue.apply(world);
            world.resource_mut::<WorldMap>().reserve_building_footprint(
                BuildingType::Wall,
                wall,
                [grid],
            );
            world.spawn((
                ObstaclePosition(grid.0, grid.1),
                ObstacleSourceKind::BuildingFootprint,
                ChildOf(wall),
            ));
            tile.spawned_wall = Some(wall);
            tile.state = WallTileState::FramedProvisional;
        } else {
            tile.state = WallTileState::FramingReady;
            world.resource_mut::<WorldMap>().reserve_building_footprint(
                BuildingType::Wall,
                wall_entity,
                [grid],
            );
        }
        let mirror = wall_tile_visual_mirror(&tile);
        let tile = world
            .spawn((
                tile,
                mirror,
                Transform::from_translation(position.extend(Z_MAP)),
                Visibility::default(),
            ))
            .id();
        if index == 1 {
            world.entity_mut(tile).insert((
                Designation {
                    work_type: WorkType::FrameWallTile,
                },
                TaskSlots::new(1),
            ));
        }
    }
    world
        .run_system_once(sync_soul_task_visual_system)
        .expect("seed production Soul mirror");
    world.insert_resource(Fixture {
        source_grid,
        blueprint_grid,
        floor_grid,
        wall_grid,
    });
    for mut transform in world
        .query_filtered::<&mut Transform, With<hw_ui::camera::MainCamera>>()
        .iter_mut(world)
    {
        transform.translation.x = 0.0;
        transform.translation.y = 0.0;
    }
    world.resource_mut::<Time<Virtual>>().pause();
}

pub(super) fn snapshot(world: &mut World) -> Value {
    let Some(fixture) = world.get_resource::<Fixture>() else {
        return Value::Null;
    };
    let mut owners = serde_json::Map::new();
    let mut owner_keys = std::collections::HashMap::new();
    for entity in world.iter_entities() {
        let key =
            if entity
                .get::<SoulIdentity>()
                .is_some_and(|identity| identity.name == "ProgressSoul")
            {
                Some("soul")
            } else if entity
                .get::<Blueprint>()
                .is_some_and(|bp| bp.occupied_grids == [fixture.blueprint_grid])
            {
                Some("blueprint")
            } else if entity.get::<FloorConstructionSite>().is_some_and(|site| {
                WorldMap::world_to_grid(site.material_center) == fixture.floor_grid
            }) {
                Some("floor")
            } else if entity.get::<WallConstructionSite>().is_some_and(|site| {
                WorldMap::world_to_grid(site.material_center) == fixture.wall_grid
            }) {
                Some("wall")
            } else {
                None
            };
        if let Some(key) = key {
            owner_keys.insert(entity.id(), key);
            owners.insert(
                key.into(),
                json!({"entity": entity.id().to_bits(),
                    "global": entity.get::<GlobalTransform>().map(|t| t.translation().to_array()),
                    "matrix": entity.get::<GlobalTransform>().map(|t| t.to_matrix().to_cols_array()),
                    "task": entity.get::<AssignedTask>().map(|task| format!("{task:?}")),
                }),
            );
        }
    }
    let camera = world
        .iter_entities()
        .find(|entity| entity.contains::<hw_ui::camera::MainCamera>());
    let mut bars = Vec::new();
    for entity in world
        .iter_entities()
        .filter(|entity| entity.contains::<GenericProgressBar>())
    {
        let parent = entity.get::<ChildOf>().map(ChildOf::parent);
        let kind = if entity.contains::<hw_visual::soul::SoulProgressBar>() {
            "soul"
        } else if entity.contains::<hw_visual::floor_construction::FloorCuringProgressBar>() {
            "floor"
        } else if entity.contains::<hw_visual::wall_construction::WallConstructionProgressBar>() {
            "wall"
        } else {
            "blueprint"
        };
        let projected = camera.as_ref().and_then(|camera| {
            camera
                .get::<Camera>()?
                .world_to_viewport(
                    camera.get::<GlobalTransform>()?,
                    entity.get::<GlobalTransform>()?.translation(),
                )
                .ok()
        });
        let bar = entity.get::<GenericProgressBar>().unwrap();
        bars.push(json!({"entity": entity.id().to_bits(), "kind": kind,
            "owner": parent.map(Entity::to_bits), "owner_key": parent.and_then(|parent| owner_keys.get(&parent)),
            "background": entity.contains::<ProgressBarBackground>(), "fill": entity.contains::<ProgressBarFill>(),
            "width": bar.config.width, "y_offset": bar.config.y_offset,
            "size": entity.get::<Sprite>().and_then(|sprite| sprite.custom_size).map(|size| size.to_array()),
            "color": entity.get::<Sprite>().map(|sprite| sprite.color.to_srgba().to_f32_array()),
            "local": entity.get::<Transform>().map(|t| t.translation.to_array()),
            "global": entity.get::<GlobalTransform>().map(|t| t.translation().to_array()),
            "point": projected.map(|p| p.to_array()),
            "visible": entity.get::<InheritedVisibility>().is_some_and(|visibility| visibility.get()),
        }));
    }
    let wall_target = world
        .iter_entities()
        .find(|entity| {
            entity
                .get::<WallTileBlueprint>()
                .is_some_and(|tile| tile.grid_pos == (fixture.wall_grid.0 + 1, fixture.wall_grid.1))
                && entity.contains::<Designation>()
        })
        .map(|entity| entity.id().to_bits());
    let source_present = world.iter_entities().any(|entity| {
        entity.contains::<Tree>()
            && entity.get::<Transform>().is_some_and(|transform| {
                WorldMap::world_to_grid(transform.translation.truncate()) == fixture.source_grid
            })
    });
    json!({"owners": owners, "bars": bars, "wall_target": wall_target, "source_present": source_present,
        "floor_complete": world.resource::<WorldMap>().floor_entity(fixture.floor_grid).is_some(),
    })
}
