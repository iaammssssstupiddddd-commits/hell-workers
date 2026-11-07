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
    matches!(
        std::env::var("HW_NATIVE_UI_CASE").as_deref(),
        Ok("rest-area-planning"
            | "rest-area-refund"
            | "rest-area-completion"
            | "rest-area-usage"
            | "rest-area-exit")
    )
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
    let rest_area_usage = if matches!(
        std::env::var("HW_NATIVE_UI_CASE").as_deref(),
        Ok("rest-area-usage" | "rest-area-exit")
    ) {
        crate::assets::building_asset_set::snapshot_rest_observations(world)
            .unwrap_or_else(|failure| json!({"failure": failure}))
    } else {
        Value::Null
    };
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
                "delivered_materials": blueprint.delivered_materials.iter()
                    .map(|(kind, count)| (format!("{kind:?}"), *count))
                    .collect::<std::collections::BTreeMap<_, _>>(),
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
        "rest_area_usage": rest_area_usage,
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
    fn rest_exit_verifier_bounds_match_actual_slow_simulation_rules() {
        use hw_core::constants::{
            DREAM_ACCUMULATE_RATE_ESCAPING, DREAM_ACCUMULATE_RATE_GATHERING,
            DREAM_ACCUMULATE_RATE_IDLE, DREAM_ACCUMULATE_RATE_WORKING,
            REST_AREA_RECRUIT_COOLDOWN_SECS,
        };
        use hw_soul_ai::soul_ai::update::slow_simulation::{
            MAX_SLOW_SIMULATION_STEPS_PER_FRAME, SLOW_SIMULATION_STEP,
        };

        assert_eq!(REST_AREA_RECRUIT_COOLDOWN_SECS, 15.0);
        assert_eq!(
            [
                DREAM_ACCUMULATE_RATE_WORKING,
                DREAM_ACCUMULATE_RATE_IDLE,
                DREAM_ACCUMULATE_RATE_GATHERING,
                DREAM_ACCUMULATE_RATE_ESCAPING,
            ]
            .into_iter()
            .fold(0.0_f32, f32::max),
            0.5
        );
        assert_eq!(
            SLOW_SIMULATION_STEP.as_secs_f64() * f64::from(MAX_SLOW_SIMULATION_STEPS_PER_FRAME),
            0.5
        );
    }

    #[test]
    fn snapshot_reads_actual_delivered_materials_without_mutation() {
        let mut world = World::new();
        world.init_resource::<WorldMap>();
        world.insert_resource(Target((2, 3)));
        let mut blueprint =
            Blueprint::new(BuildingType::RestArea, vec![(2, 3), (3, 3), (2, 4), (3, 4)]);
        blueprint
            .delivered_materials
            .insert(hw_logistics::ResourceType::Wood, 2);
        let owner = world.spawn(blueprint).id();
        let before = world
            .get::<Blueprint>(owner)
            .unwrap()
            .delivered_materials
            .clone();
        let value = snapshot(&mut world);
        assert_eq!(value["blueprints"][0]["entity"], owner.to_bits());
        assert_eq!(
            value["blueprints"][0]["delivered_materials"],
            json!({"Wood": 2})
        );
        assert_eq!(
            world.get::<Blueprint>(owner).unwrap().delivered_materials,
            before
        );
        assert_eq!(world.get::<Blueprint>(owner).unwrap().progress, 0.0);
    }

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
