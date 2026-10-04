//! Passive RestArea placement observations in the ordinary generated world.
//! Preparation changes only camera position and pause; no building, material,
//! task, occupancy, or Dream state is seeded by this profiling-only case.
use super::*;
use crate::interface::selection::placement_geometry::live_building_geometry;
use crate::world::map::WorldMapRef;
use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};
use hw_jobs::{Blueprint, Building, BuildingType};
use hw_ui::selection::{BuildingPlacementContext, validate_building_placement};
use hw_world::WorldMap;
use hw_world::zones::Yard;

#[derive(Resource)]
struct Target((i32, i32));

pub(super) fn enabled() -> bool {
    std::env::var("HW_NATIVE_UI_CASE").as_deref() == Ok("rest-area-planning")
}

fn legal_anchor(map: &WorldMap, yards: &[Yard]) -> Option<(i32, i32)> {
    let read = WorldMapRef(map);
    (1..MAP_WIDTH - 2)
        .flat_map(|x| (1..MAP_HEIGHT - 2).map(move |y| (x, y)))
        .filter(|&anchor| {
            let geometry = live_building_geometry(map, BuildingType::RestArea, anchor);
            let context = BuildingPlacementContext {
                world: &read,
                in_site: false,
                in_yard: yards.iter().any(|yard| yard.contains(geometry.draw_pos)),
                is_wall_or_door_at: &|_| false,
                is_replaceable_wall_at: &|_| false,
            };
            validate_building_placement(&context, BuildingType::RestArea, anchor, &geometry)
                .can_place
        })
        .min_by_key(|&(x, y)| (x - MAP_WIDTH / 2).abs() + (y - MAP_HEIGHT / 2).abs())
}

pub(super) fn prepare(world: &mut World) {
    let yards: Vec<_> = world.query::<&Yard>().iter(world).cloned().collect();
    let anchor = legal_anchor(world.resource::<WorldMap>(), &yards)
        .expect("generated world must contain a legal RestArea anchor inside an existing Yard");
    let center =
        live_building_geometry(world.resource::<WorldMap>(), BuildingType::RestArea, anchor)
            .draw_pos;
    for mut transform in world
        .query_filtered::<&mut Transform, With<hw_ui::camera::MainCamera>>()
        .iter_mut(world)
    {
        transform.translation.x = center.x;
        transform.translation.y = center.y;
    }
    world.insert_resource(Target(anchor));
    world.resource_mut::<Time<Virtual>>().pause();
}

pub(super) fn snapshot(world: &mut World) -> Value {
    let Some(target) = world.get_resource::<Target>() else {
        return Value::Null;
    };
    let anchor = target.0;
    let yards: Vec<_> = world.query::<&Yard>().iter(world).cloned().collect();
    let point = WorldMap::grid_to_world(anchor.0, anchor.1);
    let projected = world
        .query_filtered::<(&Camera, &GlobalTransform), With<hw_ui::camera::MainCamera>>()
        .iter(world)
        .find_map(|(camera, transform)| {
            camera.world_to_viewport(transform, point.extend(0.0)).ok()
        });
    let blueprints: Vec<_> = world
        .query::<(Entity, &Blueprint)>()
        .iter(world)
        .filter(|(_, blueprint)| blueprint.kind == BuildingType::RestArea)
        .map(|(entity, blueprint)| {
            json!({
                "entity": entity.to_bits(), "footprint": blueprint.occupied_grids,
                "progress": blueprint.progress, "materials_complete": blueprint.materials_complete(),
            })
        })
        .collect();
    let buildings: Vec<_> = world
        .query::<(Entity, &Building)>()
        .iter(world)
        .filter(|(_, building)| building.kind == BuildingType::RestArea)
        .map(|(entity, _)| entity.to_bits())
        .collect();
    let map = world.resource::<WorldMap>();
    let geometry = live_building_geometry(map, BuildingType::RestArea, anchor);
    let cells: Vec<_> = geometry
        .occupied_grids
        .iter()
        .map(|&(x, y)| {
            json!({
                "grid": [x, y], "walkable": map.is_walkable(x, y),
                "owner": map.building_entity((x, y)).map(Entity::to_bits),
            })
        })
        .collect();
    json!({
        "anchor": anchor, "footprint": geometry.occupied_grids,
        "draw_pos": geometry.draw_pos.to_array(),
        "point": projected.map(|point| point.to_array()),
        "legal": legal_anchor_at(map, &yards, anchor),
        "cells": cells, "blueprints": blueprints, "buildings": buildings,
    })
}

fn legal_anchor_at(map: &WorldMap, yards: &[Yard], anchor: (i32, i32)) -> bool {
    let geometry = live_building_geometry(map, BuildingType::RestArea, anchor);
    let read = WorldMapRef(map);
    let context = BuildingPlacementContext {
        world: &read,
        in_site: false,
        in_yard: yards.iter().any(|yard| yard.contains(geometry.draw_pos)),
        is_wall_or_door_at: &|_| false,
        is_replaceable_wall_at: &|_| false,
    };
    validate_building_placement(&context, BuildingType::RestArea, anchor, &geometry).can_place
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_world_has_yard_eligible_rest_area_without_mutation() {
        let layout = hw_world::generate_world_layout(20260914);
        let mut map = WorldMap::default();
        map.tiles.clone_from(&layout.terrain_tiles);
        for &(x, y) in layout
            .initial_tree_positions
            .iter()
            .chain(&layout.initial_rock_positions)
        {
            map.add_obstacle(x, y);
        }
        let bounds = layout.anchors.yard;
        let yard = Yard {
            min: WorldMap::grid_to_world(bounds.min_x, bounds.min_y),
            max: WorldMap::grid_to_world(bounds.max_x, bounds.max_y),
        };
        let before = map.tiles.clone();
        let anchor = legal_anchor(&map, std::slice::from_ref(&yard))
            .expect("ordinary world must have a buildable RestArea yard position");
        assert!(legal_anchor_at(&map, &[yard], anchor));
        assert_eq!(map.tiles, before);
    }
}
