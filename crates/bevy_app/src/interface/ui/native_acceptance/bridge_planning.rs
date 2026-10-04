//! Ordinary generated-world planning observations; no terrain, actor, material,
//! blueprint or task outcomes are seeded. Preparation changes only view/pause.
use super::*;
use crate::world::map::WorldMapRef;
use hw_core::constants::MAP_WIDTH;
use hw_jobs::{Blueprint, Building, BuildingType};
use hw_ui::selection::{BridgeCrossing, resolve_bridge_crossing};
use hw_world::WorldMap;
use hw_world::zones::Site;

#[derive(Resource)]
struct Target(BridgeCrossing);

pub(super) fn enabled() -> bool {
    std::env::var("HW_NATIVE_UI_CASE").as_deref() == Ok("bridge-planning")
}

pub(super) fn prepare(world: &mut World) {
    let sites: Vec<_> = world.query::<&Site>().iter(world).cloned().collect();
    let map = WorldMapRef(world.resource::<WorldMap>());
    let crossing = (0..MAP_WIDTH - 1)
        .filter_map(|x| resolve_bridge_crossing(&map, (x, 0)).ok())
        .filter(|crossing| crossing_in_site(crossing, &sites))
        .min_by_key(|crossing| (crossing.anchor.0 - MAP_WIDTH / 2).abs())
        .expect("generated world must contain a legal Bridge crossing inside an existing Site");
    let center = WorldMap::grid_to_world(crossing.anchor.0, crossing.anchor.1);
    for mut transform in world
        .query_filtered::<&mut Transform, With<hw_ui::camera::MainCamera>>()
        .iter_mut(world)
    {
        transform.translation.x = center.x;
        transform.translation.y = center.y;
    }
    world.insert_resource(Target(crossing));
    world.resource_mut::<Time<Virtual>>().pause();
}

fn crossing_in_site(crossing: &BridgeCrossing, sites: &[Site]) -> bool {
    // Match the production placement owner: Site membership uses draw_pos,
    // not the clicked tile or a terrain-only crossing predicate.
    let draw_pos = crate::interface::selection::placement_geometry::building_spawn_pos(
        BuildingType::Bridge,
        crossing.anchor,
        crossing.anchor.1,
    );
    sites.iter().any(|site| site.contains(draw_pos))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_default_world_has_no_site_eligible_crossing_and_must_not_be_admitted() {
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
        let bounds = layout.anchors.site;
        let site = Site {
            min: WorldMap::grid_to_world(bounds.min_x, bounds.min_y),
            max: WorldMap::grid_to_world(bounds.max_x, bounds.max_y),
        };
        let candidates: Vec<_> = (0..MAP_WIDTH - 1)
            .filter_map(|x| resolve_bridge_crossing(&WorldMapRef(&map), (x, 0)).ok())
            .filter(|crossing| crossing_in_site(crossing, std::slice::from_ref(&site)))
            .collect();
        // This is a refusal regression, NOT a successful Bridge acceptance.
        // The unchanged default world's river and construction Site do not
        // overlap at a valid production draw position. Do not seed a new Site
        // or weaken placement to turn this into passing lifecycle evidence.
        assert!(candidates.is_empty());
    }

    #[test]
    fn bridge_candidate_requires_existing_site_at_production_draw_position() {
        let crossing = BridgeCrossing {
            anchor: (50, 69),
            occupied_grids: vec![],
            banks: [(0, 0); 4],
        };
        let anchor = WorldMap::grid_to_world(50, 69);
        assert!(!crossing_in_site(&crossing, &[]));
        let anchor_only = Site {
            min: anchor,
            max: anchor,
        };
        assert!(!crossing_in_site(&crossing, &[anchor_only]));
        let center = crate::interface::selection::placement_geometry::building_spawn_pos(
            BuildingType::Bridge,
            crossing.anchor,
            crossing.anchor.1,
        );
        assert!(crossing_in_site(
            &crossing,
            &[Site {
                min: center,
                max: center
            }]
        ));
    }
}

pub(super) fn snapshot(world: &mut World) -> Value {
    let Some(target) = world.get_resource::<Target>() else {
        return Value::Null;
    };
    let crossing = target.0.clone();
    let point = WorldMap::grid_to_world(crossing.anchor.0, crossing.anchor.1);
    let projected = world
        .query_filtered::<(&Camera, &GlobalTransform), With<hw_ui::camera::MainCamera>>()
        .iter(world)
        .find_map(|(camera, transform)| {
            camera.world_to_viewport(transform, point.extend(0.0)).ok()
        });
    let blueprints: Vec<_> = world
        .query::<(Entity, &Blueprint)>()
        .iter(world)
        .filter(|(_, bp)| bp.kind == BuildingType::Bridge)
        .map(|(entity, bp)| {
            json!({
                "entity": entity.to_bits(), "footprint": bp.occupied_grids,
                "progress": bp.progress, "materials_complete": bp.materials_complete(),
            })
        })
        .collect();
    let buildings: Vec<_> = world
        .query::<(Entity, &Building)>()
        .iter(world)
        .filter(|(_, building)| building.kind == BuildingType::Bridge)
        .map(|(entity, _)| entity.to_bits())
        .collect();
    let map = world.resource::<WorldMap>();
    let cells: Vec<_> = crossing
        .occupied_grids
        .iter()
        .map(|&(x, y)| {
            json!({
                "grid": [x, y], "river": map.is_river_tile(x, y),
                "walkable": map.is_walkable(x, y), "bridged": map.bridged_tiles.contains(&(x, y)),
                "owner": map.building_entity((x, y)).map(Entity::to_bits),
            })
        })
        .collect();
    json!({"anchor": crossing.anchor, "footprint": crossing.occupied_grids,
        "banks": crossing.banks, "point": projected.map(|point| point.to_array()),
        "legal": resolve_bridge_crossing(&WorldMapRef(map), crossing.anchor).is_ok(),
        "cells": cells, "blueprints": blueprints, "buildings": buildings})
}
